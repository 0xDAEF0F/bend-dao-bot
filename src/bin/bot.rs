use anyhow::Result;
use bend_dao_collector::lend_pool::LendPool;
use bend_dao_collector::LendPoolEvents;
use bend_dao_collector::{benddao::BendDao, constants::bend_dao::LEND_POOL};
use dotenv::dotenv;
use ethers::providers::Middleware;
use ethers::{
    providers::{Provider, StreamExt, Ws},
    types::Address,
};
use futures::future::join_all;
use log::{debug, info};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;
    env_logger::init();

    let wss_url = std::env::var("MAINNET_RPC_URL_WS")?;

    debug!("wss_url: {wss_url}");

    let provider = Provider::<Ws>::connect(wss_url).await?;
    let provider = Arc::new(provider);

    info!(
        "current block number is: {}",
        provider.get_block_number().await?
    );

    let mut bend_dao = BendDao::try_new()?;

    bend_dao.build_all_loans().await?;

    let bend_dao = Arc::new(Mutex::new(bend_dao));

    let task_one_handle = task_one(provider.clone(), bend_dao.clone());
    let task_two_handle = task_two(provider.clone(), bend_dao.clone());

    join_all([task_one_handle, task_two_handle]).await;

    info!("ending bot");

    Ok(())
}

// listen to bend dao lend pool events and modify state
fn task_one(
    provider: Arc<Provider<Ws>>,
    bend_dao_state: Arc<Mutex<BendDao>>,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        info!("starting event listener task for lend pool events");

        let lend_pool: Address = LEND_POOL.parse()?;
        let lend_pool = LendPool::new(lend_pool, provider);

        let events = lend_pool.events();
        let mut stream = events.subscribe().await?;

        while let Some(Ok(evt)) = stream.next().await {
            let mut lock = bend_dao_state.lock().await;
            match evt {
                LendPoolEvents::BorrowFilter(evt) => {
                    // a loan has been created or re-borrowed more
                    lock.update_loan_in_system(evt.loan_id).await?;
                }
                LendPoolEvents::RepayFilter(evt) => {
                    // repayment occured. either partial or total
                    lock.update_loan_in_system(evt.loan_id).await?;
                }
                LendPoolEvents::AuctionFilter(evt) => {
                    lock.update_loan_in_system(evt.loan_id).await?;
                }
                LendPoolEvents::RedeemFilter(evt) => {
                    // loan has been partially repaid by owner and
                    // moved from auctions to active again
                    lock.update_loan_in_system(evt.loan_id).await?;
                }
                LendPoolEvents::LiquidateFilter(_evt) => {
                    // not necessary to do anything because loan
                    // was already taken off the system when the auction
                    // happenened
                }
                _ => {}
            }
        }

        info!("returning event listener task for lend pool");

        Ok(())
    })
}

// listen to new blocks in ethereum and update the state
fn task_two(
    provider: Arc<Provider<Ws>>,
    bend_dao_state: Arc<Mutex<BendDao>>,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        info!("starting task for new blocks");

        let mut stream = provider.subscribe_blocks().await?;

        while let Some(block) = stream.next().await {
            info!("new block: {:?}", block.number);
            bend_dao_state.lock().await.handle_new_block().await?;
        }

        info!("ending task for new blocks");

        Ok(())
    })
}
