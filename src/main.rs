use anyhow::Result;
use bend_dao_collector::benddao::loan::NftAsset;
use bend_dao_collector::benddao::BendDao;
use bend_dao_collector::constants::*;
use bend_dao_collector::lend_pool::LendPool;
use bend_dao_collector::simulator::Simulator;
use bend_dao_collector::spoofer::get_new_state_with_twaps_modded;
use bend_dao_collector::{Config, LendPoolEvents};
use ethers::providers::Middleware;
use ethers::{
    providers::{Provider, StreamExt, Ws},
    types::*,
};
use futures::future::join_all;
use log::{error, info, warn};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{sleep, sleep_until, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let config: Config = envy::from_env()?;

    let mut bend_dao = BendDao::try_new(config.clone()).await?;

    let provider = bend_dao.get_provider();

    bend_dao.build_all_loans().await?;

    let bend_dao = Arc::new(Mutex::new(bend_dao));

    // seperated/out of bend dao struct so lock is shorter
    let simulator = Simulator::new(config);

    let task_one_handle = task_one(provider.clone(), bend_dao.clone());
    let task_two_handle = task_two(provider.clone(), bend_dao.clone(), simulator);
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
                        let _ = bd_lock.slack_bot.send_message(&msg).await;
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

// listen to mempool for oracle updates
fn task_two(
    provider: Arc<Provider<Ws>>,
    bend_dao_state: Arc<Mutex<BendDao>>,
    simulator: Simulator,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        info!("starting task for mempool updates");

        let mut stream = provider.subscribe_full_pending_txs().await?;

        while let Some(tx) = stream.next().await {
            if tx.to.is_none() // contract creation
                || tx.to.unwrap().0 != NFT_ORACLE
                || tx.from.0 != NFT_ORACLE_CONTROLLER_EOA
            {
                continue;
            }

            let twaps = simulator.simulate_twap_changes(tx).await?;
            let modded_state = get_new_state_with_twaps_modded(twaps);
            // now check health factors
        }

        Ok(())
    })
}

/// refresh all loans in the system
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
                            let log = "loan was successfully liquidated".to_string();
                            warn!("{log}");
                            let _ = bend_dao_state
                                .lock()
                                .await
                                .slack_bot
                                .send_message(&log)
                                .await;
                        }
                        Err(e) => {
                            let log = format!("could not liquidate loan: {}", e);
                            error!("{log}");
                            let mut lock = bend_dao_state.lock().await;
                            lock.slack_bot
                                .send_message(&log)
                                .await
                                .unwrap_or_else(|err| {
                                    error!("could not send slack message: {}", err)
                                });
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
