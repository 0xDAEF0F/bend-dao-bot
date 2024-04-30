pub mod balances;
pub mod loan;

use crate::{
    benddao::balances::Balances,
    benddao::loan::{Loan, ReserveAsset, Status},
    constants::bend_dao::LEND_POOL,
    global_provider::GlobalProvider,
    prices_client::PricesClient,
    ConfigVars,
};
use anyhow::Result;
use ethers::{
    providers::{Middleware, Provider, Ws},
    signers::Signer,
    types::{Address, U256},
    utils::format_ether,
};
use log::{debug, error, info, warn};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

pub struct BendDao {
    loans: HashMap<U256, Loan>,
    balances: Balances,
    monitored_loans: HashSet<U256>,
    global_provider: GlobalProvider,
    prices_client: PricesClient,
}

impl BendDao {
    pub async fn try_new(config_vars: ConfigVars) -> Result<BendDao> {
        Ok(BendDao {
            monitored_loans: HashSet::new(),
            loans: HashMap::new(),
            global_provider: GlobalProvider::try_new(config_vars.clone()).await?,
            prices_client: PricesClient::new(config_vars),
            balances: Balances::default(),
        })
    }

    pub fn get_provider(&self) -> Arc<Provider<Ws>> {
        self.global_provider.provider.clone()
    }

    pub async fn update_balances(&mut self) -> Result<()> {
        let local_wallet_address = self.global_provider.local_wallet.address();

        let eth = self
            .global_provider
            .provider
            .get_balance(local_wallet_address, None)
            .await?;

        let weth = self
            .global_provider
            .weth
            .balance_of(local_wallet_address)
            .await?;

        let lend_pool_address: Address = LEND_POOL.parse()?;
        let approval_amount = self
            .global_provider
            .weth
            .allowance(local_wallet_address, lend_pool_address)
            .await?;

        let balances = Balances {
            eth,
            weth,
            is_lend_pool_approved: approval_amount == U256::MAX,
        };

        debug!("{:?}", balances);

        self.balances = balances;

        Ok(())
    }

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
                self.loans.remove(&loan_id);
                self.monitored_loans.remove(&loan_id);
                return Ok(());
            }
            Status::Auction => {
                // remove from the system. if the loan is redeemed it will be added back
                self.loans.remove(&loan_id);
                self.monitored_loans.remove(&loan_id);
                return Ok(());
            }
            _ => {}
        }

        match loan.should_monitor() {
            true => self.monitored_loans.insert(loan_id),
            false => self.monitored_loans.remove(&loan_id),
        };

        self.loans.insert(loan_id, loan);

        Ok(())
    }

    pub async fn handle_new_block(&mut self) -> Result<()> {
        info!("refreshing monitored loans");

        let mut pending_loan_ids_to_remove = Vec::new();

        for loan_id in self.monitored_loans.iter() {
            let updated_loan = self
                .global_provider
                .get_updated_loan(*loan_id)
                .await?
                .expect("loan in monitored_loans pool shouldn't be `None`");

            if updated_loan.status == Status::Auction {
                warn!("loan_id: {loan_id} in monitored_loans transitioned to `Status::Auction`");
                continue;
            }

            if !updated_loan.should_monitor() {
                pending_loan_ids_to_remove.push(updated_loan.loan_id);
            }

            if !updated_loan.is_auctionable() {
                info!(
                    "{:?} #{} | health_factor: {:.4} | status: HEALTHY",
                    updated_loan.nft_asset,
                    updated_loan.nft_token_id,
                    updated_loan.health_factor()
                );
                continue;
            }

            info!(
                "{:?} #{} | status: AUCTIONABLE",
                updated_loan.nft_asset, updated_loan.nft_token_id
            );

            let best_bid = self
                .prices_client
                .get_best_nft_bid(updated_loan.nft_asset)
                .await?; // WEI

            let total_debt_eth = updated_loan.get_total_debt_eth(&self.prices_client).await?;

            let bidding_amount = calculate_bidding_amount(total_debt_eth);

            if best_bid < bidding_amount {
                let debt_human_readable = total_debt_eth.as_u128() as f64 / 1e18;
                let best_bid_human_readable = best_bid.as_u128() as f64 / 1e18;
                info!(
                    "{:?} #{} unprofitable | total_debt: {:.2} > best_bid: {:.2}",
                    updated_loan.nft_asset,
                    updated_loan.nft_token_id,
                    debt_human_readable,
                    best_bid_human_readable
                );
                continue;
            }

            let potential_profit = format_ether(best_bid - bidding_amount);
            let potential_profit = potential_profit
                .parse::<f64>()
                .expect("unable to convert ETH to f64");
            info!(
                "{:?} #{} - potential profit: {:.4} ETH",
                updated_loan.nft_asset, updated_loan.nft_token_id, potential_profit
            );

            if updated_loan.reserve_asset == ReserveAsset::Usdt {
                warn!(
                    "USDT currently not supported. Missing {} ETH opportunity",
                    potential_profit
                );
                continue;
            }

            let balances = &self.balances;

            if !balances.is_lend_pool_approved {
                warn!("lend pool not approved to handle WETH. please approve it.");
                continue;
            }

            if bidding_amount > balances.weth {
                warn!("not enough WETH to participate in auction");
                continue;
            }

            if !balances.has_enough_gas_to_call_auction() {
                warn!("not enough ETH to pay for the auction gas costs");
                continue;
            }

            match self
                .global_provider
                .start_auction(&updated_loan, bidding_amount)
                .await
            {
                Ok(()) => info!("started auction successfully"),
                Err(e) => {
                    error!("failed to start auction");
                    error!("{e}");
                }
            }
        }

        for loan_id in pending_loan_ids_to_remove {
            self.monitored_loans.remove(&loan_id);
        }

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

        let start_loan_id: u64 = dotenv::var("ENVIRONMENT")
            .map(|env| {
                if env.to_lowercase() == "production" {
                    1
                } else {
                    end_loan_id - 2
                }
            })
            .unwrap_or(end_loan_id - 2);

        let iter = (start_loan_id..end_loan_id).filter(|x| !repaid_defaulted_loans_set.contains(x));

        info!("querying information for {} loans", iter.clone().count());

        let all_loans = self.global_provider.get_loans_from_iter(iter).await?;

        for loan in all_loans {
            // collections not allowed to trade in production
            if !loan.nft_asset.is_allowed_in_production() {
                continue;
            }

            if loan.status == Status::RepaidDefaulted {
                repaid_defaulted_loans_set.insert(loan.loan_id.as_u64());
                continue;
            }

            if loan.status == Status::Active {
                if loan.should_monitor() {
                    self.monitored_loans.insert(loan.loan_id);
                }
                self.loans.insert(loan.loan_id, loan);
            }
        }

        save_repaid_defaulted_loans(&repaid_defaulted_loans_set).await?;

        info!("a total of {} loans have been indexed", self.loans.len());
        info!(
            "a total of {} loans are set for monitoring",
            self.monitored_loans.len()
        );
        debug!("{:?}", &self.loans);

        Ok(())
    }

    pub async fn refresh_all_loans(&mut self) -> Result<()> {
        let iter = self.loans.keys().map(|k| k.as_u64());
        let loans = self.global_provider.get_loans_from_iter(iter).await?;

        for loan in loans {
            if loan.status == Status::RepaidDefaulted || loan.status == Status::Auction {
                self.loans.remove(&loan.loan_id);
                self.monitored_loans.remove(&loan.loan_id);
            }

            match loan.should_monitor() {
                true => self.monitored_loans.insert(loan.loan_id),
                false => self.monitored_loans.remove(&loan.loan_id),
            };

            self.loans.insert(loan.loan_id, loan);
        }

        Ok(())
    }
}

async fn get_repaid_defaulted_loans() -> Result<BTreeSet<u64>> {
    // if the file does not exist it will return Err
    let mut file = File::open("data/repaid-defaulted.json").await?;
    let mut json_string = String::new();

    file.read_to_string(&mut json_string).await?;

    let data: Vec<u64> = serde_json::from_str(&json_string)?;

    Ok(BTreeSet::from_iter(data))
}

async fn save_repaid_defaulted_loans(loans: &BTreeSet<u64>) -> Result<()> {
    // will create the file or delete it's contents if it exists already
    let mut file = File::create("data/repaid-defaulted.json").await?;

    // if BTreeSet is empty it will just write `[]` to the json file
    let data = serde_json::to_string(loans)?;

    file.write_all(data.as_bytes()).await?;

    Ok(())
}

// TODO: refine the calculation
// 40% / 365 days = 0.11% (so we take into account the interest in the next 24 hours until we liquidate)
// total_debt + 0.11% * total_debt
pub fn calculate_bidding_amount(total_debt: U256) -> U256 {
    total_debt + (total_debt * U256::from(11) / U256::from(10_000))
}
