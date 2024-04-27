use crate::{
    benddao::loan::{Loan, NftAsset, ReserveAsset, Status},
    constants::bend_dao::{LEND_POOL, LEND_POOL_LOAN, NFT_ORACLE, RESERVE_ORACLE},
    LendPool, LendPoolLoan, LoanData, NFTOracle, ReserveOracle,
};
use anyhow::Result;
use ethers::{
    providers::{Http, Provider},
    types::{Address, U256},
};
use futures::future::join_all;
use log::debug;
use std::sync::Arc;
use tokio::task::JoinHandle;

pub struct ChainProvider {
    pub provider: Arc<Provider<Http>>,
    pub lend_pool: LendPool<Provider<Http>>,
    pub lend_pool_loan: LendPoolLoan<Provider<Http>>,
    pub nft_oracle: NFTOracle<Provider<Http>>,
    pub reserve_oracle: ReserveOracle<Provider<Http>>,
}

impl ChainProvider {
    pub fn try_new(url: &str) -> Result<ChainProvider> {
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

        Ok(ChainProvider {
            provider,
            lend_pool,
            lend_pool_loan,
            nft_oracle,
            reserve_oracle,
        })
    }

    pub async fn get_loans_from_iter(&self, range: impl Iterator<Item = u64>) -> Result<Vec<Loan>> {
        let mut handles = Vec::new();
        let mut loans: Vec<Loan> = Vec::new();

        for loan_id in range {
            let loan_id = U256::from_little_endian(&loan_id.to_le_bytes());
            let lend_pool = self.lend_pool.clone();
            let lend_pool_loan = self.lend_pool_loan.clone();
            let future: JoinHandle<Result<Option<Loan>>> =
                tokio::spawn(
                    async move { get_loan_data(loan_id, lend_pool, lend_pool_loan).await },
                );
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
        get_loan_data(loan_id, self.lend_pool.clone(), self.lend_pool_loan.clone()).await
    }
}

async fn get_loan_data(
    loan_id: U256,
    lend_pool: LendPool<Provider<Http>>,
    lend_pool_loan: LendPoolLoan<Provider<Http>>,
) -> Result<Option<Loan>> {
    let loan_data: LoanData = lend_pool_loan.get_loan(loan_id).await?;

    let status = match loan_data.state {
        0 => return Ok(None),
        1 => Status::Created,
        2 => Status::Active,
        3 => Status::Auction,
        4 | 5 => Status::RepaidDefaulted,
        _ => panic!("invalid state"),
    };

    let reserve_asset = match ReserveAsset::try_from(loan_data.reserve_asset) {
        Ok(reserve_asset) => reserve_asset,
        Err(e) => {
            debug!("{e}");
            return Ok(None);
        }
    };

    let nft_asset = match NftAsset::try_from(loan_data.nft_asset) {
        Ok(nft_asset) => nft_asset,
        Err(e) => {
            debug!("{e}");
            return Ok(None);
        }
    };

    let (_, _, _, total_debt, _, health_factor) = lend_pool
        .get_nft_debt_data(loan_data.nft_asset, loan_data.nft_token_id)
        .await?;

    let loan = Loan {
        health_factor,
        status,
        total_debt,
        reserve_asset,
        nft_asset,
        loan_id: loan_data.loan_id,
        nft_token_id: loan_data.nft_token_id,
    };

    Ok(Some(loan))
}
