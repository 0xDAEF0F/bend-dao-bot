use anyhow::Result;
use bend_dao_collector::benddao::loan::NftAsset;
use bend_dao_collector::constants::math::ONE_HOUR;
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
use log::{error, info, warn};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{sleep, sleep_until, Duration};

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
    let task_four_handle = task_four(bend_dao.clone());

    join_all([
        task_one_handle,
        task_two_handle,
        task_three_handle,
        task_four_handle,
    ])
    .await;

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

        let lend_pool: Address = LEND_POOL.into();
        let lend_pool = LendPool::new(lend_pool, provider);

        let events = lend_pool.events();
        let mut stream = events.subscribe().await?;

        while let Some(Ok(evt)) = stream.next().await {
            let mut bd_lock = bend_dao_state.lock().await;
            match evt {
                LendPoolEvents::BorrowFilter(evt) => {
                    if NftAsset::try_from(evt.nft_asset).is_ok() {
                        // a loan has been created or re-borrowed more
                        bd_lock.update_loan_in_system(evt.loan_id).await?;
                    }
                }
                LendPoolEvents::RepayFilter(evt) => {
                    if NftAsset::try_from(evt.nft_asset).is_ok() {
                        // repayment occured. either partial or total
                        bd_lock.update_loan_in_system(evt.loan_id).await?;
                    }
                }
                LendPoolEvents::AuctionFilter(evt) => {
                    if NftAsset::try_from(evt.nft_asset).is_ok() {
                        bd_lock.update_loan_in_system(evt.loan_id).await?;
                    }
                }
                LendPoolEvents::RedeemFilter(evt) => {
                    if NftAsset::try_from(evt.nft_asset).is_ok() {
                        // loan has been partially repaid by owner and
                        // moved from auctions to active again
                        bd_lock.update_loan_in_system(evt.loan_id).await?;
                    }
                }
                LendPoolEvents::LiquidateFilter(evt) => {
                    if let Ok(nft_asset) = NftAsset::try_from(evt.nft_asset) {
                        let msg = format!(
                            "liquidation happened. {:?} #{}",
                            nft_asset, evt.nft_token_id
                        );
                        let _ = bd_lock.slack_bot.send_msg(&msg).await;
                        // loan was already taken off the system when the auction happened
                        info!("{:?} #{} liquidated", nft_asset, evt.nft_token_id);
                    }
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
        info!("starting the task to refresh loans every 6 hrs");
        loop {
            sleep(Duration::from_secs(ONE_HOUR * 6)).await;
            info!("refreshing all loans");
            bend_dao_state.lock().await.build_all_loans().await?;
        }
    })
}

// handle liquidations task
fn task_four(bend_dao_state: Arc<Mutex<BendDao>>) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        loop {
            let maybe_instant = bend_dao_state.lock().await.get_next_liquidation();
            match maybe_instant {
                Some((loan_id, instant)) => {
                    sleep_until(instant).await;
                    let liq_result = bend_dao_state.lock().await.try_liquidate(loan_id).await;
                    match liq_result {
                        Ok(()) => {
                            let log = format!("loan was successfully liquidated");
                            warn!("{log}");
                            let _ = bend_dao_state.lock().await.slack_bot.send_msg(&log).await;
                        }
                        Err(e) => {
                            let log = format!("could not liquidate loan: {}", e);
                            error!("{log}");
                            let mut lock = bend_dao_state.lock().await;
                            let _ = lock.slack_bot.send_msg(&log).await;
                            // do not try to liquidate again
                            lock.our_pending_auctions.remove(&loan_id);
                        }
                    }
                }
                None => {
                    info!("no pending future auctions. sleeping for 6 hours.");
                    sleep(Duration::from_secs(ONE_HOUR * 6)).await;
                }
            }
        }
    })
}
