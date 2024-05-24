pub mod auction;
pub mod balances;
pub mod loan;
pub mod status;

use self::{auction::Auction, status::Status};
use crate::{
    constants::math::{ONE_DAY, ONE_MINUTE},
    global_provider::GlobalProvider,
    prices_client::PricesClient,
    slack_bot::SlackBot,
    utils::{calculate_bidding_amount, get_repaid_defaulted_loans, save_repaid_defaulted_loans},
    ConfigVars,
};
use anyhow::{anyhow, bail, Result};
use ethers::{
    providers::{Middleware, Provider, Ws},
    types::{U256, U64},
    utils::format_ether,
};
use log::{error, info, warn};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};
use tokio::{
    time::{Duration, Instant},
    try_join,
};

pub struct BendDao {
    monitored_loans: HashSet<U256>,
    pub our_pending_auctions: HashMap<U256, Instant>, // loan_id -> Instant
    global_provider: GlobalProvider,
    prices_client: PricesClient,
    pub slack_bot: SlackBot,
}

impl BendDao {
    pub async fn try_new(config_vars: ConfigVars) -> Result<BendDao> {
        Ok(BendDao {
            monitored_loans: HashSet::new(),
            our_pending_auctions: HashMap::new(),
            global_provider: GlobalProvider::try_new(config_vars.clone()).await?,
            prices_client: PricesClient::new(config_vars.clone()),
            slack_bot: SlackBot::new(config_vars),
        })
    }

    pub fn get_provider(&self) -> Arc<Provider<Ws>> {
        self.global_provider.provider.clone()
    }

    // assumes that whenever we triggered the auction we already put that in `our_pending_auctions`
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
                self.monitored_loans.remove(&loan_id);
                return Ok(());
            }
            Status::Auction(auction) => {
                let msg = format!("auction happening - {}", loan);
                let _ = self.slack_bot.send_msg(&msg).await;
                // remove from the system. if the loan is redeemed it will be added back
                self.monitored_loans.remove(&loan_id);
                if !auction.is_ours(&self.global_provider.local_wallet) {
                    self.our_pending_auctions.remove(&loan_id);
                }
                return Ok(());
            }
            Status::Active => {
                // TODO: send a notification to our slack to signal that they redeemed us
                // if its in our pending auctions we should remove it
                self.our_pending_auctions.remove(&loan_id);
            }
            Status::Created => {
                info!("Status::Created is not handled");
            }
        }

        match loan.should_monitor() {
            true => self.monitored_loans.insert(loan_id),
            false => self.monitored_loans.remove(&loan_id),
        };

        Ok(())
    }

    pub async fn handle_new_block(&mut self, block_number: Option<U64>) -> Result<()> {
        let range = self.monitored_loans.iter().map(|loan_id| loan_id.as_u64());
        let mut monitored_loans = self.global_provider.get_loans_from_iter(range).await?;
        monitored_loans.sort_by_key(|k| k.health_factor);

        for updated_loan in monitored_loans {
            if let Status::Auction(_auction) = updated_loan.status {
                warn!("transitioned to `Status::Auction` and was not handled by event listener");
                continue;
            }

            if !updated_loan.should_monitor() {
                let msg = format!(
                    "removing {:?} #{} from monitored loans. HF: {:.2}",
                    updated_loan.nft_asset,
                    updated_loan.nft_token_id,
                    updated_loan.health_factor()
                );
                info!("{msg}");
                let _ = self.slack_bot.send_msg(&msg).await;
                self.monitored_loans.remove(&updated_loan.loan_id);
            }

            if !updated_loan.is_auctionable() {
                info!(
                    "{:?} #{} not auctionable. skipping...",
                    updated_loan.nft_asset, updated_loan.nft_token_id
                );
                continue;
            }

            let msg = format!(
                "{:?} #{} HF below 1. Checking profitability",
                updated_loan.nft_asset, updated_loan.nft_token_id
            );
            info!("{msg}");
            let _ = self.slack_bot.send_msg(&msg).await;

            // IS PROFITABLE
            let (best_bid_eth, total_debt_eth) = try_join!(
                self.prices_client.get_best_nft_bid(updated_loan.nft_asset),
                updated_loan.get_total_debt_eth(&self.prices_client)
            )?;

            let bidding_amount = calculate_bidding_amount(total_debt_eth);

            if best_bid_eth < bidding_amount {
                let debt_human_readable = total_debt_eth.as_u128() as f64 / 1e18;
                let best_bid_human_readable = best_bid_eth.as_u128() as f64 / 1e18;
                info!(
                    "{}",
                    format!(
                        ">> unprofitable | total debt: {:.2} > best bid: {:.2}",
                        debt_human_readable, best_bid_human_readable
                    )
                );
                continue;
            }

            let potential_profit = format_ether(best_bid_eth - bidding_amount);
            let potential_profit = potential_profit
                .parse::<f64>()
                .expect("unable to convert ETH to f64");
            info!(
                "{}",
                format!(">> potential profit: {:.4} ETH", potential_profit)
            );
            let slack_msg = format!(
                "potential profit of ~ {:.4} ETH for {:?} #{}\nproceeding to start auction...",
                potential_profit, updated_loan.nft_asset, updated_loan.nft_token_id
            );
            let _ = self.slack_bot.send_msg(&slack_msg).await;

            // IS PROFITABLE [END]

            let balances = self.global_provider.get_balances().await?;

            if let (false, log) = balances.can_initiate_auction_with_log(&updated_loan) {
                warn!("{log}");
                let _ = self.slack_bot.send_msg(&log).await;
                continue;
            }

            match self
                .global_provider
                .start_auction(
                    &updated_loan,
                    calculate_bidding_amount(updated_loan.total_debt),
                )
                .await
            {
                Ok(()) => {
                    let msg = format!(
                        "@here started auction successfully for {:?} #{}",
                        updated_loan.nft_asset, updated_loan.nft_token_id
                    );
                    info!("{msg}");
                    let _ = self.slack_bot.send_msg(&msg).await;
                    let cushion_time = ONE_MINUTE * 5;
                    let instant = Instant::now() + Duration::from_secs(ONE_DAY + cushion_time);
                    self.our_pending_auctions
                        .insert(updated_loan.loan_id, instant);
                }
                Err(e) => {
                    let msg = format!(
                        "@here failed to start auction for {:?} #{}",
                        updated_loan.nft_asset, updated_loan.nft_token_id
                    );
                    let _ = self.slack_bot.send_msg(&msg).await;
                    error!("{msg}");
                    error!("{e}");
                }
            }
        }

        // send slack message and log every 300 blocks the monitored loans
        if block_number.is_some_and(|x| x.as_u64() % 300 == 0) {
            let block_number = block_number.unwrap();
            self.notify_and_log_monitored_loans(block_number).await?;
        }

        Ok(())
    }

    pub fn get_next_liquidation(&self) -> Option<(U256, Instant)> {
        self.our_pending_auctions
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

        if !balances.has_enough_gas_to_auction_or_liquidate() {
            bail!("not enough ETH balance to liquidate")
        }

        if auction.best_bid < loan.total_debt {
            bail!("can't liquidate because best_bid < total_debt")
        }

        self.global_provider.liquidate_loan(&loan).await?;

        self.our_pending_auctions.remove(&loan.loan_id);

        Ok(())
    }

    pub async fn build_all_loans(&mut self) -> Result<()> {
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

        let all_loans = self.global_provider.get_loans_from_iter(iter).await?;
        let mut display_monitored_loans = String::from("");

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
                if auction.is_ours(&self.global_provider.local_wallet) {
                    let instant = auction.get_bid_end();
                    self.our_pending_auctions.insert(loan.loan_id, instant);
                    continue;
                }
            }

            if loan.should_monitor() {
                self.monitored_loans.insert(loan.loan_id);
                display_monitored_loans.push_str(&format!("{}\n", loan))
            }
        }

        save_repaid_defaulted_loans(&repaid_defaulted_loans_set).await?;

        let block_number = self.global_provider.provider.get_block_number().await?;
        self.notify_and_log_monitored_loans(block_number).await?;

        Ok(())
    }

    pub async fn notify_and_log_monitored_loans(&self, block_number: U64) -> Result<()> {
        let mut msg = format!("~~~ Block: *#{}* ~~~\n", block_number);

        let range = self.monitored_loans.iter().map(|loan_id| loan_id.as_u64());
        let mut loans = self.global_provider.get_loans_from_iter(range).await?;
        loans.sort_by_key(|x| x.health_factor);

        for loan in loans.into_iter().take(5) {
            msg.push_str(&format!(
                "{:?} #{} | HF: *{:.5}*\n",
                loan.nft_asset,
                loan.nft_token_id,
                loan.health_factor()
            ));
        }

        let _ = self.slack_bot.send_msg(&msg).await;
        info!("{msg}");

        Ok(())
    }
}
