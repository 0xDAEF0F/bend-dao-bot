use anyhow::Result;
use bend_dao_collector::constants::LEND_POOL;
use bend_dao_collector::lend_pool::LendPool;
use bend_dao_collector::LendPoolEvents;
use chrono::Local;
use dotenv::dotenv;
use ethers::{
    providers::{Provider, StreamExt, Ws},
    types::Address,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;

    let wss_url = std::env::var("MAINNET_RPC_URL_WS")?;

    let provider = Provider::<Ws>::connect(wss_url).await?;
    let provider = Arc::new(provider);

    let address: Address = LEND_POOL.parse()?;

    let contract = LendPool::new(address, provider);

    let events = contract.events();

    let mut stream = events.stream().await?;

    while let Some(Ok(evt)) = stream.next().await {
        let local = Local::now();
        match evt {
            LendPoolEvents::AuctionFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendPoolEvents::BorrowFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendPoolEvents::DepositFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendPoolEvents::LiquidateFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendPoolEvents::PausedFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendPoolEvents::PausedTimeUpdatedFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendPoolEvents::RedeemFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendPoolEvents::RepayFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendPoolEvents::ReserveDataUpdatedFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendPoolEvents::UnpausedFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendPoolEvents::WithdrawFilter(a) => {
                println!("{local}\n{:?}", a);
            }
        }
    }

    Ok(())
}
