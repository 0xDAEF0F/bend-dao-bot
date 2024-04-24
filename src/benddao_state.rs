use crate::{
    constants::bend_dao::{HEALTH_FACTOR_THRESHOLD_TO_MONITOR, LEND_POOL, LEND_POOL_LOAN},
    data_source::DataSource,
    LendPool, LendPoolLoan, LoanData,
};
use anyhow::Result;
use ethers::{
    providers::Provider,
    types::{Address, U256},
};
use futures::future::join_all;
use log::info;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::{spawn, task::JoinHandle};

#[derive(Debug, PartialEq)]
pub enum Status {
    Created, // not sure about this state
    Active,
    Auction,
}

#[derive(Debug)]
pub struct BendDao {
    loans: HashMap<U256, Loan>,
    monitored_loans: HashSet<U256>,
    data_source: DataSource,
}

#[derive(Debug)]
pub struct Loan {
    pub loan_id: U256,
    pub status: Status,
    pub nft_token_id: U256,
    pub health_factor: U256,
    pub total_debt: U256,
    pub reserve_asset: Address,
    pub nft_collection: Address,
}

impl BendDao {
    pub fn try_new() -> Result<BendDao> {
        let url = dotenv::var("MAINNET_RPC_URL")?;
        Ok(BendDao {
            monitored_loans: HashSet::new(),
            loans: HashMap::new(),
            data_source: DataSource::try_new(&url)?,
        })
    }

    pub async fn handle_repay_loan(&mut self, loan_id: U256) -> Result<()> {
        match self.data_source.get_updated_loan(loan_id).await? {
            Some(loan) => {
                // still active (partial repay)
                self.handle_monitoring(&loan);
                self.loans.insert(loan_id, loan);
            }
            None => {
                // fully repaid
                self.loans.remove(&loan_id);
                self.monitored_loans.remove(&loan_id);
            }
        }
        Ok(())
    }

    pub async fn handle_auction(&mut self, loan_id: U256) -> Result<()> {
        let loan = self
            .data_source
            .get_updated_loan(loan_id)
            .await?
            .expect("loan should exist");

        self.handle_monitoring(&loan);

        self.loans.insert(loan_id, loan);

        Ok(())
    }

    pub async fn handle_borrow(&mut self, loan_id: U256) -> Result<()> {
        let loan = self
            .data_source
            .get_updated_loan(loan_id)
            .await?
            .expect("loan should exist");

        self.handle_monitoring(&loan);

        self.loans.insert(loan_id, loan);

        Ok(())
    }

    pub async fn handle_redeem(&mut self, loan_id: U256) -> Result<()> {
        let loan = self
            .data_source
            .get_updated_loan(loan_id)
            .await?
            .expect("loan should still be active");

        self.handle_monitoring(&loan);
        self.loans.insert(loan_id, loan);

        Ok(())
    }

    // take it off the system
    pub fn handle_liquidation(&mut self, loan_id: U256) {
        self.loans.remove(&loan_id);
        self.monitored_loans.remove(&loan_id);
    }

    fn handle_monitoring(&mut self, loan: &Loan) {
        // nothing to do here just take it off the monitoring list and return
        if loan.status == Status::Auction {
            self.monitored_loans.remove(&loan.loan_id);
            return;
        }

        if loan.health_factor < U256::from_dec_str(HEALTH_FACTOR_THRESHOLD_TO_MONITOR).unwrap() {
            self.monitored_loans.insert(loan.loan_id);
        } else {
            self.monitored_loans.remove(&loan.loan_id);
        }
    }

    pub async fn handle_new_block(&mut self) -> Result<()> {
        for loan_id in self.monitored_loans.iter() {
            let updated_loan = self.data_source.get_updated_loan(*loan_id).await?.unwrap();

            if updated_loan.health_factor >= U256::exp10(18) {
                continue;
            }

            // determine the profitability
        }

        Ok(())
    }

    pub async fn build_all_loans(&'static mut self) -> Result<()> {
        let last_loan_id: u64 = self
            .data_source
            .lend_pool_loan
            .get_current_loan_id()
            .await?
            .as_u64();
        let start_loan_id: u64 = dotenv::var("ENVIRONMENT")
            .map(|env| {
                if env.to_lowercase() == "production" {
                    1
                } else {
                    last_loan_id - 200
                }
            })
            .unwrap_or(last_loan_id - 200);

        let all_loans = self
            .data_source
            .get_loans(start_loan_id, last_loan_id)
            .await?;

        for loan in all_loans {
            self.loans.insert(loan.loan_id, loan);
        }

        info!("all loans have been built");

        Ok(())
    }
}
