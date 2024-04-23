use crate::{
    benddao_state::Loan,
    constants::bend_dao::{LEND_POOL, LEND_POOL_LOAN, NFT_ORACLE, RESERVE_ORACLE},
    LendPool, LendPoolLoan, LoanData, NFTOracle, ReserveOracle,
};
use anyhow::Result;
use ethers::{
    providers::{Http, Provider},
    types::{Address, U256},
};
use std::sync::Arc;

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

    pub async fn get_updated_loan(&self, loan_id: U256) -> Result<Loan> {
        let loan_data: LoanData = self.lend_pool_loan.get_loan(loan_id).await?;

        let (_, _, _, total_debt, _, health_factor) = self
            .lend_pool
            .get_nft_debt_data(loan_data.reserve_asset, loan_data.nft_token_id)
            .await?;

        let loan = Loan {
            health_factor,
            total_debt,
            loan_id: loan_data.loan_id,
            nft_collection: loan_data.nft_asset,
            nft_token_id: loan_data.nft_token_id,
            reserve_asset: loan_data.reserve_asset,
        };

        Ok(loan)
    }
}
