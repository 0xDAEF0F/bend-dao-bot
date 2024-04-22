use crate::{
    constants::{LEND_POOL, LEND_POOL_LOAN, NFT_ORACLE, RESERVE_ORACLE},
    LendPool, LendPoolLoan, NFTOracle, ReserveOracle,
};
use anyhow::Result;
use ethers::{
    providers::{Http, Provider},
    types::{Address, U256},
};
use std::sync::Arc;

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

    pub async fn get_nft_health_factor(&self, nft_addr: Address, token_id: U256) -> Result<U256> {
        let (_, _, _, _total_debt, _, health_factor) =
            self.lend_pool.get_nft_debt_data(nft_addr, token_id).await?;

        Ok(health_factor)
    }
}
