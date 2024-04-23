use crate::{
    constants::bend_dao::{LEND_POOL, LEND_POOL_LOAN},
    data_source::DataSource,
    LendPool, LendPoolLoan, LoanData,
};
use anyhow::Result;
use ethers::{
    providers::Provider,
    types::{Address, U256},
};
use futures::future::join_all;
use std::sync::Arc;
use tokio::{spawn, task::JoinHandle};

#[derive(Debug)]
pub struct BendDao {
    all_loans: Vec<Loan>,
    data_source: DataSource,
}

#[derive(Debug)]
pub struct Loan {
    pub loan_id: U256,
    pub nft_token_id: U256,
    pub health_factor: U256,
    pub total_debt: U256,
    pub reserve_asset: Address,
    pub nft_collection: Address,
}

impl BendDao {
    pub fn new() -> BendDao {
        BendDao {
            all_loans: Vec::new(),
            data_source: DataSource::try_new("").unwrap(),
        }
    }

    pub async fn handle_repay_loan(&mut self, loan_id: U256) -> Result<()> {
        let loan = self.data_source.get_updated_loan(loan_id).await?;
        todo!();
    }

    pub async fn handle_auction(&mut self, loan_id: U256) -> Result<()> {
        let loan = self.data_source.get_updated_loan(loan_id).await?;
        todo!();
    }

    pub async fn handle_borrow(&mut self, loan_id: U256) -> Result<()> {
        let loan = self.data_source.get_updated_loan(loan_id).await?;
        todo!();
    }

    pub async fn handle_redeem(&mut self, loan_id: U256) -> Result<()> {
        let loan = self.data_source.get_updated_loan(loan_id).await?;
        todo!();
    }

    pub async fn handle_liquidation(&mut self, loan_id: U256) -> Result<()> {
        todo!();
    }

    pub async fn handle_new_block(&mut self) -> Result<()> {
        todo!();
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

                if loan.state != 2 {
                    return Ok(None);
                }

                let (_, _, _, total_debt, _, health_factor) = lend_pool
                    .get_nft_debt_data(loan.nft_asset, loan.nft_token_id)
                    .await?;

                let loan = Loan {
                    loan_id: loan.loan_id,
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
                self.all_loans.push(loan);
            }
        }

        // sort all loans by health factor in ascending order
        self.all_loans.sort_by_key(|loan| loan.health_factor);

        Ok(())
    }
}
