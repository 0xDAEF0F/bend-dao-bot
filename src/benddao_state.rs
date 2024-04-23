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
    pub fn new() -> BendDao {
        BendDao {
            monitored_loans: HashSet::new(),
            loans: HashMap::new(),
            data_source: DataSource::try_new("").unwrap(),
        }
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

    pub async fn build_all_loans<P>(&mut self, provider: Arc<Provider<P>>) -> Result<()>
    where
        P: ethers::providers::JsonRpcClient + 'static,
    {
        let lend_pool: Address = LEND_POOL.parse()?;
        let lend_pool = LendPool::new(lend_pool, provider.clone());

        let lend_pool_loan: Address = LEND_POOL_LOAN.parse()?;
        let lend_pool_loan = LendPoolLoan::new(lend_pool_loan, provider.clone());

        let last_loan_id: u64 = lend_pool_loan.get_current_loan_id().await?.as_u64();
        let start_loan_id: u64 = dotenv::var("ENVIRONMENT")
            .map(|env| {
                if env.to_lowercase() == "production" {
                    1
                } else {
                    last_loan_id - 10
                }
            })
            .unwrap_or(last_loan_id - 10);

        let mut handles = Vec::new();

        for loan_id in start_loan_id..last_loan_id {
            let lend_pool = lend_pool.clone();
            let lend_pool_loan = lend_pool_loan.clone();
            let future: JoinHandle<Result<Option<Loan>>> = spawn(async move {
                let loan: LoanData = lend_pool_loan.get_loan(loan_id.into()).await?;

                // repaid or defaulted (not interested in these loans)
                if loan.state == 4 || loan.state == 5 {
                    return Ok(None);
                }

                let status = match loan.state {
                    1 => Status::Created,
                    2 => Status::Active,
                    3 => Status::Auction,
                    _ => panic!("invalid state"),
                };

                let (_, _, _, total_debt, _, health_factor) = lend_pool
                    .get_nft_debt_data(loan.nft_asset, loan.nft_token_id)
                    .await?;

                let loan = Loan {
                    loan_id: loan.loan_id,
                    status,
                    health_factor,
                    total_debt,
                    reserve_asset: loan.reserve_asset,
                    nft_collection: loan.nft_asset,
                    nft_token_id: loan.nft_token_id,
                };

                Ok(Some(loan))
            });
            handles.push(future);
        }

        for loan in join_all(handles).await {
            if let Some(loan) = loan?? {
                self.loans.insert(loan.loan_id, loan);
            }
        }

        Ok(())
    }
}
