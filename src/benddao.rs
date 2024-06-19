pub mod loan;
pub mod status;

use self::status::Status;
use crate::{
    constants::OUR_EOA_ADDRESS,
    global_provider::GlobalProvider,
    prices_client::PricesClient,
    types::*,
    utils::{calculate_bidding_amount, get_repaid_defaulted_loans, save_repaid_defaulted_loans},
    AuctionFilter, Config, LiquidateFilter, RedeemFilter,
};
use anyhow::Result;
use ethers::{
    providers::{Middleware, Provider, Ws},
    types::{spoof::State, Transaction, U256},
};
use ethers_flashbots::BundleRequest;
use loan::{Loan, NftAsset, ReserveAsset};
use log::{info, warn};
use messenger_rs::slack_hook::SlackClient;
use std::{collections::BTreeSet, sync::Arc};
use tokio::sync::RwLock;

#[allow(dead_code)]
pub struct BendDao {
    monitored_loans: Vec<U256>, // sorted by `health_factor` in ascending order
    pub pending_auctions: PendingAuctions,
    global_provider: GlobalProvider,
    prices_client: Arc<RwLock<PricesClient>>,
    pub slack_bot: SlackClient,
}

impl BendDao {
    pub async fn try_new(
        config_vars: Config,
        prices_client: Arc<RwLock<PricesClient>>,
        slack_bot: SlackClient,
    ) -> Result<BendDao> {
        Ok(BendDao {
            monitored_loans: vec![],
            pending_auctions: PendingAuctions::default(),
            global_provider: GlobalProvider::try_new(config_vars.clone()).await?,
            prices_client,
            slack_bot,
        })
    }

    pub fn get_provider(&self) -> Arc<Provider<Ws>> {
        self.global_provider.provider.clone()
    }

    pub fn get_global_provider(&self) -> GlobalProvider {
        self.global_provider.clone()
    }

    pub async fn react_to_auction(&mut self, evt: AuctionFilter) {
        let bid_end_timestamp = self
            .global_provider
            .get_auction_end_timestamp(evt.nft_asset, evt.nft_token_id)
            .await;

        let auction = Auction {
            current_bid: evt.bid_price,
            current_bidder: evt.on_behalf_of,
            nft_asset: evt.nft_asset.try_into().unwrap(),
            nft_token_id: evt.nft_token_id,
            bid_end_timestamp,
            reserve_asset: evt.reserve.try_into().unwrap(),
        };

        let msg = match self.pending_auctions.add_update_auction(auction) {
            true => format!(
                "New bid for {:?} #{} by {} | Auction time remaining: {} seconds",
                auction.nft_asset,
                evt.nft_token_id,
                {
                    if evt.on_behalf_of != OUR_EOA_ADDRESS.into() {
                        evt.on_behalf_of.to_string() + " (not us)"
                    } else {
                        "us".to_string()
                    }
                },
                chrono::TimeDelta::seconds(
                    bid_end_timestamp.as_u64() as i64 - chrono::Local::now().timestamp()
                )
                .num_seconds()
            ),
            false => format!(
                "New auction initiated for {:?} #{} by {}",
                auction.nft_asset,
                auction.nft_token_id,
                {
                    if evt.on_behalf_of != OUR_EOA_ADDRESS.into() {
                        evt.on_behalf_of.to_string() + " (not us)"
                    } else {
                        "us".to_string()
                    }
                }
            ),
        };

        info!("{msg}");
        self.slack_bot.send_message(msg).await.ok();
    }

    pub async fn react_to_redeem(&mut self, evt: RedeemFilter) {
        let nft_asset = NftAsset::try_from(evt.nft_asset).unwrap();
        self.pending_auctions
            .remove_auction(nft_asset, evt.nft_token_id);

        let nft_asset = NftAsset::try_from(evt.nft_asset).unwrap();
        let msg = format!("Redeem happened on {:?} #{}", nft_asset, evt.nft_token_id);
        info!("{msg}");
        self.slack_bot.send_message(&msg).await.ok();
    }

    pub async fn react_to_liquidation(&mut self, evt: LiquidateFilter) {
        let nft_asset = NftAsset::try_from(evt.nft_asset).unwrap();
        self.pending_auctions
            .remove_auction(nft_asset, evt.nft_token_id);

        let msg = format!(
            "liquidation happened for {:?} #{}",
            nft_asset, evt.nft_token_id
        );
        warn!("{msg}");
        self.slack_bot.send_message(&msg).await.ok();
    }

    pub async fn initiate_auctions_if_any(
        &mut self,
        nft_oracle_tx: Transaction,
        modded_state: Option<State>,
    ) -> Result<Option<BundleRequest>> {
        let iter = self.monitored_loans.iter().map(|x| x.as_u64());
        let monitored_loans = self
            .global_provider
            .get_loans_from_iter(iter, modded_state)
            .await?;

        let mut balances = self.global_provider.get_balances().await?;

        let loans_ready_to_auction = self
            .package_loans_ready_to_auction(monitored_loans, &mut balances)
            .await;

        if loans_ready_to_auction.is_empty() {
            return Ok(None);
        }

        let mut bundle = BundleRequest::new();
        bundle.add_transaction(nft_oracle_tx);

        Ok(Some(
            self.global_provider
                .create_auction_bundle(bundle, loans_ready_to_auction, false)
                .await?,
        ))
    }

    async fn package_loans_ready_to_auction(
        &self,
        loans: Vec<Loan>,
        balances: &mut Balances,
    ) -> Vec<AuctionBid> {
        let mut loans_for_auction = vec![];
        let (prices, eth_usd) = {
            let prices_client = &self.prices_client.read().await;
            (
                &prices_client.prices.clone(),
                prices_client.get_eth_usd_price(),
            )
        };

        for loan in loans {
            if loan.status != Status::Active || !loan.is_auctionable() {
                info!("loan is not auctionable");
                continue;
            }

            if !balances.is_usdt_lend_pool_approved || !balances.is_weth_lend_pool_approved {
                warn!("dont have approved usdt/weth");
                continue;
            }

            if !balances.eth < U256::exp10(16) {
                warn!("not enough eth for txn");
                continue;
            } else {
                balances.eth -= U256::exp10(16);
            }

            let bid_amount = calculate_bidding_amount(loan.total_debt);
            match loan.reserve_asset {
                ReserveAsset::Usdt => {
                    if balances.usdt < bid_amount {
                        continue;
                    } else {
                        let price = prices.get(&loan.nft_asset).unwrap() * U256::exp10(6) / eth_usd;
                        if bid_amount > price {
                            continue;
                        }
                        balances.usdt -= bid_amount;
                    }
                }
                ReserveAsset::Weth => {
                    if balances.weth < bid_amount {
                        continue;
                    } else {
                        let price = *prices.get(&loan.nft_asset).unwrap();
                        if bid_amount > price {
                            continue;
                        }
                        balances.weth -= bid_amount;
                    }
                }
            }

            let auction_bid = AuctionBid {
                bid_price: bid_amount,
                nft_asset: loan.nft_asset.into(),
                nft_token_id: loan.nft_token_id,
            };

            loans_for_auction.push(auction_bid)
        }

        loans_for_auction
    }

    pub async fn refresh_monitored_loans(&mut self) -> Result<()> {
        let mut repaid_defaulted_loans_set = get_repaid_defaulted_loans()
            .await
            .unwrap_or_else(|_| BTreeSet::new());

        // this loan has not yet existed so not inclusive range
        let end_loan_id: u64 = self
            .global_provider
            .lend_pool_loan
            .get_current_loan_id()
            .await?
            .as_u64();

        let iter = (1..end_loan_id).filter(|x| !repaid_defaulted_loans_set.contains(x));

        info!("querying information for {} loans", iter.clone().count());

        let all_loans = self.global_provider.get_loans_from_iter(iter, None).await?;
        let mut loans_to_monitor = vec![];

        for loan in all_loans {
            // collections not allowed to trade in production
            if !loan.nft_asset.is_allowed_in_production() {
                continue;
            }

            if loan.status == Status::RepaidDefaulted {
                repaid_defaulted_loans_set.insert(loan.loan_id.as_u64());
                continue;
            }

            if let Status::Auction(auction) = loan.status {
                self.pending_auctions.add_update_auction(auction);
            }

            if loan.should_monitor() {
                loans_to_monitor.push((loan.loan_id, loan.health_factor));
            }
        }

        loans_to_monitor.sort_by(|a, b| a.1.cmp(&b.1));

        self.monitored_loans = loans_to_monitor
            .into_iter()
            .map(|(loan_id, _hf)| loan_id)
            .collect();

        save_repaid_defaulted_loans(&repaid_defaulted_loans_set).await?;

        self.notify_and_log_monitored_loans().await?;

        Ok(())
    }

    /// Notifies to slack and logs monitored loans every 600 blocks ~ 2 Hrs
    pub async fn notify_and_log_monitored_loans(&self) -> Result<()> {
        let block_number = self.global_provider.provider.get_block_number().await?;

        if block_number.as_u64() % 600 != 0 {
            return Ok(());
        }

        let mut msg = format!("~~~ Block: *#{}* ~~~\n", block_number);

        let range = self.monitored_loans.iter().map(|loan_id| loan_id.as_u64());
        let mut loans = self
            .global_provider
            .get_loans_from_iter(range, None)
            .await?;
        loans.sort_by_key(|x| x.health_factor);

        for loan in loans.into_iter().take(5) {
            msg.push_str(&format!(
                "{:?} #{} | HF: *{:.5}*\n",
                loan.nft_asset,
                loan.nft_token_id,
                loan.health_factor()
            ));
        }

        let _ = self.slack_bot.send_message(&msg).await;
        info!("{msg}");

        Ok(())
    }

    /// Bids first auction
    pub async fn verify_and_package_outbids(
        &mut self,
        auctions: &Vec<Auction>,
    ) -> Result<Vec<BundleRequest>> {
        let mut bundles = Vec::new();

        let (prices, eth_usd_price) = {
            let prices_client = self.prices_client.read().await;

            (&prices_client.prices.clone(), prices_client.eth_usd_price)
        };

        for auction in auctions {
            let mut nft_best_bid_price = *prices.get(&auction.nft_asset).unwrap();

            if auction.reserve_asset == ReserveAsset::Usdt {
                nft_best_bid_price = nft_best_bid_price * U256::exp10(6) / eth_usd_price;
            }

            let outbid = auction.current_bid * 101 / 100;

            if nft_best_bid_price > outbid {
                // not sending as one bundle bc we may get a revert chain
                // if one bid get frontrun, all bids will revert
                bundles.push(self.send_bid(auction, outbid).await?)
            } else {
                info!(
                    "bid on {:?} #{} was not profitable for {} as price is: {}",
                    auction.nft_asset, auction.nft_token_id, outbid, nft_best_bid_price
                );
            }
        }

        Ok(bundles)
    }

    async fn send_bid(&self, auction: &Auction, bid: U256) -> Result<BundleRequest> {
        let bundle = BundleRequest::new()
            .set_max_timestamp(auction.bid_end_timestamp.as_u64())
            // 14 is arbitrary
            // can change in future
            .set_min_timestamp(auction.bid_end_timestamp.as_u64() - 14);
        self.global_provider
            .create_auction_bundle(bundle, vec![AuctionBid::new(auction, bid)], true)
            .await
    }
}
