#![cfg(test)]

use anyhow::Result;
use bend_dao_collector::benddao_state::BendDao;
use bend_dao_collector::constants::{USDT_ADDRESS, WETH_ADDRESS};
use bend_dao_collector::{
    constants::{BEND_INCEPTION_BLOCK, LEND_POOL},
    LendPool, LendPoolEvents,
};
use bend_dao_collector::{BorrowFilter, ReserveData};
use dotenv::dotenv;
use ethers::providers::Middleware;
use ethers::types::{U256, U64};
use ethers::{
    contract::LogMeta,
    providers::{Http, Provider},
    types::Address,
};
use std::cmp;
use std::collections::HashSet;
use std::sync::Arc;

const TO_BLOCK: u64 = 14695332;
const BORROW_INDEX_T1: &str = "1021631895953930649510339652";
const INTEREST_RATE_T1: &str = "150380200444504240174026135";

#[tokio::test]
async fn match_latest_reserve_data() -> Result<()> {
    dotenv()?;

    let url = dotenv::var("MAINNET_RPC_URL")?;

    let provider = Provider::<Http>::try_from(url)?;
    let provider = Arc::new(provider);

    let lend_pool = LendPool::new(LEND_POOL.parse::<Address>()?, provider);

    let evts = lend_pool
        .events()
        .from_block(BEND_INCEPTION_BLOCK)
        .to_block(TO_BLOCK)
        .query_with_meta()
        .await;

    println!("{:#?}", evts);

    // let mut bend_dao_state = BendDao::new();

    // for (evt, meta) in evts {
    //     match evt {
    //         LendPoolEvents::ReserveDataUpdatedFilter(rd) => {
    //             bend_dao_state.update_reserve_data(rd);
    //             bend_dao_state.update_block(meta);
    //         }
    //         _ => {}
    //     }
    // }

    // println!("{:#?}", bend_dao_state);

    Ok(())
}

#[tokio::test]
async fn burner() -> Result<()> {
    dotenv()?;

    let url = dotenv::var("MAINNET_RPC_URL")?;
    let provider = Arc::new(Provider::<Http>::try_from(url)?);

    let provider_c = provider.clone();
    let lend_pool = LendPool::new(LEND_POOL.parse::<Address>()?, provider_c);

    let mut loan_ids: HashSet<U256> = HashSet::new();

    let mut start_block = BEND_INCEPTION_BLOCK;
    let last_block_number = provider.get_block_number().await?.as_u64();

    while start_block <= last_block_number {
        let end_block = cmp::min(start_block + 999_999, last_block_number);

        let res: Vec<BorrowFilter> = lend_pool
            .borrow_filter()
            .from_block(start_block)
            .to_block(end_block)
            .query()
            .await?;

        for borrow in res {
            loan_ids.insert(borrow.loan_id);
        }

        start_block += 1_000_000;
    }

    println!("{:#?}", loan_ids);

    // lend_pool.get_nft_debt_data(nft_asset, nft_token_id)

    Ok(())
}
