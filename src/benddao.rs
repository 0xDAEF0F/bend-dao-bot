pub mod loan;

use crate::{
    benddao::loan::{Loan, ReserveAsset, Status},
    chain_provider::ChainProvider,
    prices_client::PricesClient,
};
use anyhow::Result;
use ethers::types::U256;
use log::{debug, info, warn};
use std::collections::{BTreeSet, HashMap, HashSet};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

pub struct BendDao {
    loans: HashMap<U256, Loan>,
    monitored_loans: HashSet<U256>,
    chain_provider: ChainProvider,
    prices_client: PricesClient,
}

impl BendDao {
    pub fn try_new() -> Result<BendDao> {
        let url = dotenv::var("MAINNET_RPC_URL")?;
        Ok(BendDao {
            monitored_loans: HashSet::new(),
            loans: HashMap::new(),
            chain_provider: ChainProvider::try_new(&url)?,
            prices_client: PricesClient::default(),
        })
    }

    pub async fn update_loan_in_system(&mut self, loan_id: U256) -> Result<()> {
        let loan = match self.chain_provider.get_updated_loan(loan_id).await? {
            None => return Ok(()),
            Some(l) => l,
        };

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
        info!("checking on monitored_loans");

        let mut pending_loan_ids_to_remove = Vec::new();

        for loan_id in self.monitored_loans.iter() {
            let updated_loan = self
                .chain_provider
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
                    "loan_id: {} - health_factor: {:.2} - status: healthy",
                    loan_id.as_u64(),
                    updated_loan.health_factor()
                );
                continue;
            }

            info!("loan_id: {} - status: auctionable", loan_id);

            let best_bid = self
                .prices_client
                .get_best_nft_bid(updated_loan.nft_asset)
                .await?; // WEI

            let total_debt_eth = match updated_loan.reserve_asset {
                ReserveAsset::Usdt => {
                    let usdt_eth_price = self.prices_client.get_usdt_eth_price().await?;
                    updated_loan.total_debt * usdt_eth_price / U256::exp10(6)
                }
                ReserveAsset::Weth => updated_loan.total_debt,
            };

            if best_bid < total_debt_eth {
                let debt_human_readable = total_debt_eth.as_u128() as f64 / 1e18;
                let best_bid_human_readable = best_bid.as_u128() as f64 / 1e18;
                info!(
                    "loan_id: {} unprofitable - total_debt: {:.2} > best_bid: {:.2}",
                    loan_id, debt_human_readable, best_bid_human_readable
                );
                continue;
            }

            let potential_profit = (best_bid - total_debt_eth).as_u128() as f64 / 1e18;
            info!(
                "loan_id: {} - potential profit: {:.2}",
                loan_id, potential_profit
            );
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
            .chain_provider
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
        let all_loans = self.chain_provider.get_loans_from_iter(iter).await?;

        for loan in all_loans {
            if loan.status == Status::RepaidDefaulted {
                repaid_defaulted_loans_set.insert(loan.loan_id.as_u64());
            }

            if loan.status != Status::Auction {
                if loan.should_monitor() {
                    self.monitored_loans.insert(loan.loan_id);
                }
                self.loans.insert(loan.loan_id, loan);
            }
        }

        save_repaid_defaulted_loans(&repaid_defaulted_loans_set).await?;

        info!("all loans have been built");
        debug!("{:?}", &self.loans);

        Ok(())
    }

    pub async fn refresh_all_loans(&mut self) -> Result<()> {
        let iter = self.loans.keys().map(|k| k.as_u64());
        let loans = self.chain_provider.get_loans_from_iter(iter).await?;

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
