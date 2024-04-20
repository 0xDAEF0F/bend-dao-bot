#![allow(unused)]
#![cfg(test)]

use anyhow::Result;
use bend_dao_collector::{
    constants::{LEND_POOL, LEND_POOL_LOAN, NFT_ORACLE, RESERVE_ORACLE},
    data_source::DataSource,
    lend_pool, LendPool, LendPoolLoan, NFTOracle, ReserveOracle,
};
use ethers::{
    providers::{Http, JsonRpcClient, Provider, Ws},
    types::{Address, U256},
};
use std::sync::Arc;

async fn get_nft_twap_price(
    clients: &DataSource,
    nft_address: Address,
    reserve_address: Address,
) -> Result<U256> {
    let nft_unit_price: U256 = clients.nft_oracle.get_asset_price(nft_address).await?;
    let reserve_unit_price: U256 = clients
        .reserve_oracle
        .get_asset_price(reserve_address)
        .await?;

    todo!()
}

#[test]
fn test_a() {}
