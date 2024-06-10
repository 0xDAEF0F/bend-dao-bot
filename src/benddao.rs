pub mod loan;
pub mod status;

use self::status::Status;
use crate::{
    constants::*,
    global_provider::GlobalProvider,
    prices_client::PricesClient,
    types::*,
    utils::{calculate_bidding_amount, get_repaid_defaulted_loans, save_repaid_defaulted_loans},
    Config,
};
use anyhow::{anyhow, bail, Result};
use ethers::{
    providers::{Middleware, Provider, Ws},
    types::{spoof::State, H160, U256, U64},
};
use ethers_flashbots::BundleRequest;
use loan::{Loan, ReserveAsset};
use log::{error, info, warn};
use messenger_rs::slack_hook::SlackClient;
use std::{
    collections::{self, BTreeSet, HashMap},
    sync::Arc,
};
use tokio::time::{Duration, Instant};

#[allow(dead_code)]
pub struct BendDao {
    monitored_loans: Vec<U256>, // sorted by `health_factor` in ascending order
    pending_auctions: PendingAuctions,
    global_provider: GlobalProvider,
    prices_client: PricesClient,
    pub slack_bot: SlackClient,
}

impl BendDao {
    pub async fn try_new(config_vars: Config) -> Result<BendDao> {
        Ok(BendDao {
            monitored_loans: vec![],
            pending_auctions: PendingAuctions::default(),
            global_provider: GlobalProvider::try_new(config_vars.clone()).await?,
            prices_client: PricesClient::new(config_vars.clone()),
            slack_bot: SlackClient::new(config_vars.clone().slack_url),
        })
    }

    pub fn get_provider(&self) -> Arc<Provider<Ws>> {
        self.global_provider.provider.clone()
    }

    // temp fn
    // TODO: CHANGE !!!
    pub fn get_global_provider(&self) -> GlobalProvider {
        self.global_provider.clone()
    }

    // THIS WILL GET DEPRECATED. IT IS DOING TOO MUCH.
    pub async fn update_loan_in_system(&mut self, loan_id: U256) -> Result<()> {
        let loan = match self.global_provider.get_updated_loan(loan_id).await? {
            None => return Ok(()),
            Some(l) => l,
        };

        if !loan.nft_asset.is_allowed_in_production() {
            return Ok(());
        }

        match loan.status {
            Status::RepaidDefaulted => {
                // would be nice to update the data store, too but it's not that important.
                // we can do that in the next synchronization of `build_all_loans`
                self.monitored_loans
                    .get_mut(&H160::from(loan.nft_asset))
                    .unwrap()
                    .retain(|x| x != &loan_id);
                return Ok(());
            }
            Status::Auction(auction) => {
                // remove from the system. if the loan is redeemed it will be added back
                self.monitored_loans
                    .get_mut(&H160::from(loan.nft_asset))
                    .unwrap()
                    .retain(|x| x != &loan_id);

                if !auction.is_ours(&self.global_provider.local_wallet) {
                    self.pending_auctions.remove(&loan_id);
                }
                let msg = format!("auction happening - {}", loan);
                let _ = self.slack_bot.send_message(&msg).await;
                return Ok(());
            }
            Status::Active => {
                // TODO: send a notification to our slack to signal that they redeemed us
                // if its in our pending auctions we should remove it
                self.pending_auctions.remove(&loan_id);
            }
            Status::Created => {
                info!("Status::Created is not handled");
            }
        }

        Ok(())
    }

    pub async fn initiate_auctions_if_any(&mut self, modded_state: Option<State>) -> Result<()> {
        let iter = self.monitored_loans.iter().map(|x| x.as_u64());
        let monitored_loans = self
            .global_provider
            .get_loans_from_iter(iter, modded_state)
            .await?;

        let mut balances = self.global_provider.get_balances().await?;

        let loans_ready_to_auction =
            BendDao::package_loans_ready_to_auction(monitored_loans, &mut balances);

        if loans_ready_to_auction.is_empty() {
            return Ok(());
        }

        let bundle = self
            .global_provider
            .create_auction_bundle(BundleRequest::new(), loans_ready_to_auction)
            .await?;

        match self.global_provider.send_bundle(bundle).await {
            Ok(()) => {
                info!("started an auction bundle");
                let _ = self
                    .slack_bot
                    .send_message("started an auction bundle")
                    .await;
            }
            Err(e) => {
                error!("{e}");
                let _ = self.slack_bot.send_message("failed to start bundle").await;
            }
        }

        self.notify_and_log_monitored_loans().await?;

        Ok(())
    }

    fn package_loans_ready_to_auction(
        loans: Vec<Loan>,
        balances: &mut Balances,
    ) -> Vec<AuctionBid> {
        let mut loans_for_auction = vec![];

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
                        balances.usdt -= bid_amount;
                    }
                }
                ReserveAsset::Weth => {
                    if balances.weth < bid_amount {
                        continue;
                    } else {
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

    pub fn get_next_liquidation(&self) -> Option<(U256, Instant)> {
        self.pending_auctions
            .iter()
            .min_by(|a, b| a.1.cmp(b.1))
            .map(|(&loan_id, &instant)| (loan_id, instant))
    }

    /// 1] has the auction ended?
    /// 2] do we have enough eth to call liquidate?
    /// 3] did we actually win the auction?
    /// 4] is the bid we pushed enough to pay the total debt?
    pub async fn try_liquidate(&mut self, loan_id: U256) -> Result<()> {
        let loan = self
            .global_provider
            .get_updated_loan(loan_id)
            .await?
            .ok_or_else(|| anyhow!("benddao.rs - 265"))?;

        let auction: Auction;
        if let Status::Auction(auction_) = loan.status {
            auction = auction_;
        } else {
            bail!("{} is not in auction", loan)
        }

        if !auction.is_ours(&self.global_provider.local_wallet) {
            bail!("auction is not ours")
        }

        let has_auction_ended = self
            .global_provider
            .has_auction_ended(loan.nft_asset, loan.nft_token_id)
            .await?;

        if !has_auction_ended {
            bail!("auction has not ended yet")
        }

        let balances = self.global_provider.get_balances().await?;

        if balances.eth < U256::exp10(16) {
            bail!("not enough ETH balance to liquidate")
        }

        if auction.current_bid < loan.total_debt {
            bail!("can't liquidate because best_bid < total_debt")
        }

        self.global_provider.liquidate_loan(&loan).await?;

        self.pending_auctions.remove(&loan.loan_id);

        Ok(())
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
                // TODO: Insert it to the current auctions
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

    pub async fn create_db_cache() -> Result<()> {
        Ok(())
    }
}
