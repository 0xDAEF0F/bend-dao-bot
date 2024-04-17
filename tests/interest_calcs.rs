#![cfg(test)]

use anyhow::Result;
use bend_dao_collector::math::{calculate_compounded_interest, ray_mul};
use ethers::types::U256;

// example of a benddao loan ocurrence
pub const LOAN_ID: u64 = 12196;
pub const NFT_TOKEN_ID: u64 = 5477;
pub const BORROWER_ADDRESS: &str = "0x7FD8E5a1e5D7A7E272266Dd4CAD0072D319573D2";

// interest rates don't change from t0-t1 because i choose t1 to be a block before
// interest rate change
pub const BORROW_RATE_T0: &str = "105768883926509761075108855";
pub const BORROW_RATE_T1: &str = "105768883926509761075108855";

// scaled debt does not change unless there the user takes some action
pub const USER_SCALED_DEBT_T0: u64 = 34230761162;
pub const USER_SCALED_DEBT_T1: u64 = 34230761162;

// borrow index *DOES* change with time
pub const BORROW_INDEX_T0: &str = "1089155710661941255265101813";
pub const BORROW_INDEX_T1: &str = "1089160401033479259295493309";

pub const BLOCK_T0: u64 = 18940159;
pub const BLOCK_T1: u64 = 18940264;

pub const TIMESTAMP_T0: u64 = 1704446555;
pub const TIMESTAMP_T1: u64 = 1704447839;

pub const DEBT_T0: u64 = 37282629000;
pub const DEBT_T1: u64 = 37282789555;

#[tokio::test]
// dependencies: last borrow index, last borrow rate
// the borrow rate remains the same if there is no withdrawal/deposit of reserves
async fn calculate_total_debt() -> Result<()> {
    let borrow_rate = U256::from_dec_str(BORROW_RATE_T0)?;
    let compound_interest = calculate_compounded_interest(
        borrow_rate,
        U256::from(TIMESTAMP_T0),
        U256::from(TIMESTAMP_T1),
    );
    let borrow_index_t0 = U256::from_dec_str(BORROW_INDEX_T0)?;

    let borrow_index_t1 = ray_mul(compound_interest, borrow_index_t0).unwrap();
    assert_eq!(borrow_index_t1, U256::from_dec_str(BORROW_INDEX_T1)?);

    let debt_t1 = ray_mul(borrow_index_t1, U256::from(USER_SCALED_DEBT_T0)).unwrap();

    assert_eq!(debt_t1, U256::from(DEBT_T1));

    Ok(())
}

#[tokio::test]
async fn scaled_debt_is_what_user_borrows_divided_by_borrow_index() -> Result<()> {
    let ray = U256::exp10(27);

    let debt_t0: U256 = U256::from(DEBT_T0);
    let borrow_index = U256::from_dec_str(BORROW_INDEX_T0)?;

    let scaled_debt_at_t0 = debt_t0 * ray / borrow_index;

    assert_eq!(scaled_debt_at_t0, U256::from(USER_SCALED_DEBT_T0));

    Ok(())
}

#[tokio::test]
async fn total_debt_is_scaled_debt_times_borrow_index() -> Result<()> {
    // scaled by a ray (supposedly)
    let user_scaled_debt: U256 = U256::from(USER_SCALED_DEBT_T0);

    // scaled by a ray (this one is always changing upwards with time. interest accumulates here)
    let borrow_index = U256::from_dec_str(BORROW_INDEX_T1)?;

    let debt_at_t1 = ray_mul(user_scaled_debt, borrow_index).unwrap();

    assert_eq!(debt_at_t1, U256::from(DEBT_T1));

    Ok(())
}
