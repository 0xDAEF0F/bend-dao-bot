#![cfg(test)]

use anyhow::Result;
use bend_dao_collector::benddao::loan::{NftAsset, ReserveAsset};
use bend_dao_collector::benddao::BendDao;
use bend_dao_collector::prices_client::PricesClient;
use bend_dao_collector::types::Auction;
use bend_dao_collector::{constants::*, prices_client, Config};
use ethers::types::H160;
use ethers::utils::parse_ether;
use ethers::{
    types::{Address, U256},
    utils::Anvil,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

#[tokio::test]
async fn test_bid_bundle_creation() -> Result<()> {
    // env_logger::init();

    let anvil = Anvil::default()
        .fork("https://sepolia.infura.io/v3/875080fe51934e0b9d5736139fc3e4e7")
        // .fork_block_number(20101046u64)
        .mnemonic("abstract vacuum mammal awkward pudding scene penalty purchase dinner depart evoke puzzle")
        .spawn();

    let config = Config {
        mainnet_rpc_url_ws: anvil.ws_endpoint(),
        mnemonic: "abstract vacuum mammal awkward pudding scene penalty purchase dinner depart evoke puzzle".to_string(),
        alchemy_api_key: "S1llhLoNFxJdv4K85HALN0xYqNXaa7d0".to_string(),
        reservoir_api_key: "f1bc813b-97f8-5808-83de-1238af13d6f9".to_string(),
        coinmarketcap_api_key: "6373b364-088c-4d1e-a9d3-c6020d33e3ac".to_string(),
        slack_url: "https://hooks.slack.com/services/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".to_string(),
        env: None,
    };

    let mut prices_client = PricesClient::new(config.clone());

    prices_client.refresh_prices().await?;

    let prices_client = Arc::new(RwLock::new(prices_client));

    let mut state = BendDao::try_new(config, prices_client).await?;

    let auctions = vec![Auction {
        nft_asset: NftAsset::CryptoPunks,
        nft_token_id: U256::from(12),
        current_bid: parse_ether(1u8).unwrap(),
        current_bidder: H160::default(),
        bid_end_timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs()
            .into(),
        reserve_asset: ReserveAsset::Weth,
    }];

    let bundles = state.verify_and_package_outbids(&auctions).await?;

    dbg!(&bundles);

    /* {
        "chainId": "0x0",
        "nonce": "0x0",
        "maxPriorityFeePerGas": "0xa",
        "maxFeePerGas": "0xa",
        "gasLimit": "0x0",
        "to": "0x70b97a0da65c15dfb0ffa02aee6fa36e507c2762",
        "value": "0x0",
        "data": "0xa4c0166b000000000000000000000000b7f7f6c52f2e2fdb1963eab30438024864c313f6000000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000e043da6172500000000000000000000000000003cdb3d9e1b74692bb1e3bb5fc81938151ca64b02",
        {
            "function": "auction(address,uint256,uint256,address)",
            "params": [
                "0xb7F7F6C52F2e2fdb1963Eab30438024864c313F6",
                "12",
                "1010000000000000000",
                "0x3cDB3d9e1B74692Bb1E3bb5fc81938151cA64b02"
            ]
        }
        "accessList": [],
        "v": "0x1",
        "r": "0x17e257e484e9e8c31ad7e0941df0f3f8b2a4b5ad968ec8586b9dbae110c99df7",
        "s": "0x3c12b1a325ee8eb97159c34f775b832ec39765df07c90f6f76e6a9576d568a5c"
    } */

    // state.get_global_provider().send_and_handle_bundle(bundles.first().unwrap().clone()).await?;

    Ok(())
}

#[tokio::test]
async fn test_auction_creation_bundle() -> Result<()> {
    let anvil = Anvil::default()
        // .fork("http://eth-mainnet.g.alchemy.com/v2/S1llhLoNFxJdv4K85HALN0xYqNXaa7d0")
        // .fork_block_number(20101046u64)
        .spawn();

    let config = Config {
            mainnet_rpc_url_ws: anvil.ws_endpoint(),
            mnemonic: "abstract vacuum mammal awkward pudding scene penalty purchase dinner depart evoke puzzle".to_string(),
            alchemy_api_key: "S1llhLoNFxJdv4K85HALN0xYqNXaa7d0".to_string(),
            reservoir_api_key: "f1bc813b-97f8-5808-83de-1238af13d6f9".to_string(),
            coinmarketcap_api_key: "6373b364-088c-4d1e-a9d3-c6020d33e3ac".to_string(),
            slack_url: "https://hooks.slack.com/services/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".to_string(),
            env: None,
        };

    let mut prices_client = PricesClient::new(config.clone());

    // prices_client.refresh_prices().await?;

    let prices_client = Arc::new(RwLock::new(prices_client));

    let mut state = BendDao::try_new(config, prices_client).await?;

    Ok(())
}
