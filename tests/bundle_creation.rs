#![cfg(test)]

use anyhow::Result;
use bend_dao_collector::benddao::loan::{NftAsset, ReserveAsset};
use bend_dao_collector::benddao::BendDao;
use bend_dao_collector::prices_client::PricesClient;
use bend_dao_collector::types::Auction;
use bend_dao_collector::{constants::*, prices_client, Config};
use bend_dao_collector::{utils::get_loan_data, Erc721, LendPool, LendPoolLoan, Weth};
use chrono::DateTime;
use ethers::types::H160;
use ethers::utils::parse_ether;
use ethers::{
    middleware::SignerMiddleware,
    providers::{Http, Provider},
    signers::{LocalWallet, Signer},
    types::{Address, U256},
    utils::Anvil,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::Instant;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_bid_bundle_creation() -> Result<()> {
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

    prices_client.refresh_prices().await?;

    let prices_client = Arc::new(RwLock::new(prices_client));

    let mut state = BendDao::try_new(config, prices_client).await?;

    let auctions = vec![Auction {
        nft_asset: NftAsset::CryptoPunks,
        nft_token_id: U256::from(12),
        current_bid: parse_ether(1u8).unwrap(),
        current_bidder: H160::default(),
        bid_end_timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs().into(),
        reserve_asset: ReserveAsset::Weth,
    }];

    let bundles = state.verify_and_package_outbids(&auctions).await?;

    dbg!(bundles);

    Ok(())
}
