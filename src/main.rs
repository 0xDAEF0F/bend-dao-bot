use anyhow::Result;
use bend_dao_collector::constants::LENDING_POOL;
use bend_dao_collector::lending_pool::LendingPool;
use bend_dao_collector::LendingPoolEvents;
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

    let address: Address = LENDING_POOL.parse()?;

    let contract = LendingPool::new(address, provider);

    let events = contract.events();

    let mut stream = events.stream().await?;

    while let Some(Ok(evt)) = stream.next().await {
        let local = Local::now();
        match evt {
            LendingPoolEvents::AuctionFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendingPoolEvents::BorrowFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendingPoolEvents::DepositFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendingPoolEvents::LiquidateFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendingPoolEvents::PausedFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendingPoolEvents::PausedTimeUpdatedFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendingPoolEvents::RedeemFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendingPoolEvents::RepayFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendingPoolEvents::ReserveDataUpdatedFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendingPoolEvents::UnpausedFilter(a) => {
                println!("{local}\n{:?}", a);
            }
            LendingPoolEvents::WithdrawFilter(a) => {
                println!("{local}\n{:?}", a);
            }
        }
    }

    Ok(())
}
