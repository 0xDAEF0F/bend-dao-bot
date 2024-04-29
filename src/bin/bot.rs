use anyhow::Result;
use bend_dao_collector::lend_pool::LendPool;
use bend_dao_collector::{benddao::BendDao, constants::bend_dao::LEND_POOL};
use bend_dao_collector::{ConfigVars, LendPoolEvents};
use dotenv::dotenv;
use ethers::providers::Middleware;
use ethers::{
    providers::{Provider, StreamExt, Ws},
    types::Address,
};
use futures::future::join_all;
use log::info;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;
    env_logger::init();

    let config_vars = ConfigVars::try_new()?;

    let mut bend_dao = BendDao::try_new(config_vars.clone()).await?;

    let provider = bend_dao.get_provider();

    bend_dao.build_all_loans().await?;

    let bend_dao = Arc::new(Mutex::new(bend_dao));

    let task_one_handle = task_one(provider.clone(), bend_dao.clone());
    let task_two_handle = task_two(provider.clone(), bend_dao.clone());
    let task_three_handle = task_three(bend_dao.clone());

    join_all([task_one_handle, task_two_handle, task_three_handle]).await;

    info!("bot is shutting down");

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
            let mut bd_lock = bend_dao_state.lock().await;
            match evt {
                LendPoolEvents::BorrowFilter(evt) => {
                    // a loan has been created or re-borrowed more
                    bd_lock.update_loan_in_system(evt.loan_id).await?;
                }
                LendPoolEvents::RepayFilter(evt) => {
                    // repayment occured. either partial or total
                    bd_lock.update_loan_in_system(evt.loan_id).await?;
                }
                LendPoolEvents::AuctionFilter(evt) => {
                    bd_lock.update_loan_in_system(evt.loan_id).await?;
                }
                LendPoolEvents::RedeemFilter(evt) => {
                    // loan has been partially repaid by owner and
                    // moved from auctions to active again
                    bd_lock.update_loan_in_system(evt.loan_id).await?;
                }
                LendPoolEvents::LiquidateFilter(evt) => {
                    // loan was already taken off the system when the auction happened
                    info!("loan was liquidated in benddao. loan_id: {}", evt.loan_id);
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

// refresh all loans in the system
fn task_three(bend_dao_state: Arc<Mutex<BendDao>>) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        info!("starting the task to refresh loans every 24 hrs");
        loop {
            sleep(Duration::from_secs(24 * 60 * 60)).await;
            info!("refreshing all loans");
            bend_dao_state.lock().await.refresh_all_loans().await?;
        }
    })
}
