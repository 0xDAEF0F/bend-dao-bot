use crate::{
    benddao_state::{Loan, Status},
    constants::bend_dao::{LEND_POOL, LEND_POOL_LOAN, NFT_ORACLE, RESERVE_ORACLE},
    LendPool, LendPoolLoan, LoanData, NFTOracle, ReserveOracle,
};
use anyhow::Result;
use ethers::{
    providers::{Http, Provider},
    types::{Address, U256},
};
use futures::future::join_all;
use std::sync::Arc;
use tokio::task::JoinHandle;

#[derive(Debug)]
pub struct DataSource {
    pub provider: Arc<Provider<Http>>,
    pub lend_pool: LendPool<Provider<Http>>,
    pub lend_pool_loan: LendPoolLoan<Provider<Http>>,
    pub nft_oracle: NFTOracle<Provider<Http>>,
    pub reserve_oracle: ReserveOracle<Provider<Http>>,
}

impl DataSource {
    pub fn try_new(url: &str) -> Result<DataSource> {
        let provider = Provider::<Http>::try_from(url)?;
        let provider = Arc::new(provider);

        let address = LEND_POOL.parse::<Address>()?;
        let lend_pool = LendPool::new(address, provider.clone());

        let address = LEND_POOL_LOAN.parse::<Address>()?;
        let lend_pool_loan = LendPoolLoan::new(address, provider.clone());

        let address = NFT_ORACLE.parse::<Address>()?;
        let nft_oracle = NFTOracle::new(address, provider.clone());

        let address = RESERVE_ORACLE.parse::<Address>()?;
        let reserve_oracle = ReserveOracle::new(address, provider.clone());

        Ok(DataSource {
            provider,
            lend_pool,
            lend_pool_loan,
            nft_oracle,
            reserve_oracle,
        })
    }

    pub async fn get_loans(
        &'static self,
        start_loan_id: u64,
        last_loan_id: u64,
    ) -> Result<Vec<Loan>> {
        let mut handles = Vec::new();
        let mut loans: Vec<Loan> = Vec::new();

        for loan_id in start_loan_id..last_loan_id {
            let loan_id = U256::from_little_endian(&loan_id.to_le_bytes());
            let future: JoinHandle<Result<Option<Loan>>> =
                tokio::spawn(async move { self.get_updated_loan(loan_id).await });
            handles.push(future);
        }

        let result = join_all(handles).await;

        for res in result {
            let loan = res??;
            if let Some(loan) = loan {
                loans.push(loan)
            }
        }

        Ok(loans)
    }
    pub async fn get_updated_loan(&self, loan_id: U256) -> Result<Option<Loan>> {
        let loan_data: LoanData = self.lend_pool_loan.get_loan(loan_id).await?;

        // repaid or defaulted
        if loan_data.state == 4 || loan_data.state == 5 {
            return Ok(None);
        }

        let status = match loan_data.state {
            1 => Status::Created,
            2 => Status::Active,
            3 => Status::Auction,
            _ => panic!("invalid state"),
        };

        let (_, _, _, total_debt, _, health_factor) = self
            .lend_pool
            .get_nft_debt_data(loan_data.reserve_asset, loan_data.nft_token_id)
            .await?;

        let loan = Loan {
            health_factor,
            status,
            total_debt,
            loan_id: loan_data.loan_id,
            nft_collection: loan_data.nft_asset,
            nft_token_id: loan_data.nft_token_id,
            reserve_asset: loan_data.reserve_asset,
        };

        Ok(Some(loan))
    }
}
