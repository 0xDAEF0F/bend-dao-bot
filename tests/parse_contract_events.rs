#![cfg(test)]

use anyhow::Result;
use bend_dao_collector::constants::bend_dao::LEND_POOL;
use bend_dao_collector::lend_pool::{AuctionFilter, LendPool};
use dotenv::dotenv;
use ethers::{
    providers::{Provider, Ws},
    types::{Address, U256},
};
use std::sync::Arc;

const AUCTION_EVENT_BLOCK: u64 = 19_625_942;

#[tokio::test]
async fn test_subscribe_parse_past_auction_event() -> Result<()> {
    dotenv()?;

    let wss_url = std::env::var("MAINNET_RPC_URL_WS")?;

    let provider = Provider::<Ws>::connect(wss_url).await?;
    let provider = Arc::new(provider);

    let address: Address = LEND_POOL.parse()?;

    let contract = LendPool::new(address, provider);

    let event = contract
        .auction_filter()
        .from_block(AUCTION_EVENT_BLOCK)
        .to_block(AUCTION_EVENT_BLOCK);

    let events: Vec<AuctionFilter> = event.query().await?;

    if let Some(auction) = events.into_iter().next() {
        assert_eq!(
            auction.user,
            "0x3b968d2d299b895a5fcf3bba7a64ad0f566e6f88".parse()?
        );
        assert_eq!(
            auction.reserve,
            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".parse()?
        );
        assert_eq!(auction.bid_price, U256::from_dec_str("363000000000000000")?);
        assert_eq!(auction.loan_id, U256::from_dec_str("12584")?);
    }

    Ok(())
}
