use crate::{
    benddao::{
        auction::Auction,
        loan::{Loan, NftAsset, ReserveAsset},
        status::Status,
    },
    LendPool, LendPoolLoan, LoanData,
};
use anyhow::Result;
use ethers::{
    providers::{JsonRpcClient, Provider},
    types::U256,
};
use log::debug;
use std::collections::BTreeSet;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

pub async fn get_repaid_defaulted_loans() -> Result<BTreeSet<u64>> {
    // if the file does not exist it will return Err
    let mut file = File::open("data/repaid-defaulted.json").await?;
    let mut json_string = String::new();

    file.read_to_string(&mut json_string).await?;

    let data: Vec<u64> = serde_json::from_str(&json_string)?;

    Ok(BTreeSet::from_iter(data))
}

pub async fn save_repaid_defaulted_loans(loans: &BTreeSet<u64>) -> Result<()> {
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

// builds a loan based on the struct `Loan`. does not care if the
// `NftAsset` is not supported in production
pub async fn get_loan_data<U>(
    loan_id: U256,
    lend_pool: LendPool<Provider<U>>,
    lend_pool_loan: LendPoolLoan<Provider<U>>,
) -> Result<Option<Loan>>
where
    U: JsonRpcClient + 'static,
{
    let loan_data: LoanData = lend_pool_loan.get_loan(loan_id).await?;

    let status = match loan_data.state {
        0 => return Ok(None),
        1 => Status::Created,
        2 => Status::Active,
        3 => Status::Auction(Auction {
            highest_bidder: loan_data.bidder_address,
            best_bid: loan_data.bid_price,
            bid_start_timestamp: loan_data.bid_start_timestamp,
        }),
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
