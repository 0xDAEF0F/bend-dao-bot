use crate::{
    constants::{LEND_POOL, LEND_POOL_LOAN},
    data_source::DataSource,
    LendPool, LendPoolLoan, LoanData,
};
use anyhow::Result;
use ethers::{
    providers::{Http, Provider},
    types::{Address, U256},
};
use futures::future::join_all;
use std::sync::Arc;
use tokio::{spawn, task::JoinHandle};

#[derive(Debug)]
pub struct BendDao {
    pub loans: Vec<Loan>,
    provider: Arc<Provider<Http>>,
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
    pub fn try_new(url: &str) -> Result<Self> {
        let provider = Provider::<Http>::try_from(url)?;

        Ok(BendDao {
            loans: Vec::new(),
            provider: Arc::new(provider),
        })
    }

    pub async fn build_all_loans(&mut self) -> Result<()> {
        let lend_pool: Address = LEND_POOL.parse()?;
        let lend_pool = LendPool::new(lend_pool, self.provider.clone());

        let lend_pool_loan: Address = LEND_POOL_LOAN.parse()?;
        let lend_pool_loan = LendPoolLoan::new(lend_pool_loan, self.provider.clone());

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
            let future: JoinHandle<Result<(_, _, _)>> = spawn(async move {
                let loan: LoanData = lend_pool_loan.get_loan(loan_id.into()).await?;
                let (_, _, _, total_debt, _, health_factor) = lend_pool
                    .get_nft_debt_data(loan.nft_asset, loan.nft_token_id)
                    .await?;

                Ok((loan, total_debt, health_factor))
            });
            handles.push(future);
        }

        for loan in join_all(handles).await {
            let (loan, total_debt, health_factor) = loan??;
            // is active
            if loan.state == 2 {
                self.loans.push(Loan {
                    loan_id: loan.loan_id,
                    health_factor,
                    total_debt,
                    reserve_asset: loan.reserve_asset,
                    nft_collection: loan.nft_asset,
                    nft_token_id: loan.nft_token_id,
                });
            }
        }

        // sort all loans by health factor in ascending order
        self.loans.sort_by_key(|loan| loan.health_factor);

        Ok(())
    }
}
