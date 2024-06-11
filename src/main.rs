use anyhow::Result;
use bend_dao_collector::benddao::loan::NftAsset;
use bend_dao_collector::benddao::BendDao;
use bend_dao_collector::global_provider::GlobalProvider;
use bend_dao_collector::lend_pool::LendPool;
use bend_dao_collector::simulator::Simulator;
use bend_dao_collector::spoofer::get_new_state_with_twaps_modded;
use bend_dao_collector::{constants::*, global_provider};
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

    // KILL THIS
    let global_provider = bend_dao.get_global_provider();

    bend_dao.refresh_monitored_loans().await?;

    let bend_dao = Arc::new(Mutex::new(bend_dao));

    // seperated/out of bend dao struct so lock is shorter
    let simulator = Simulator::new(config);

    let task_one_handle = bend_dao_event_task(provider.clone(), bend_dao.clone());
    let task_two_handle = nft_oracle_mempool_task(
        provider.clone(),
        bend_dao.clone(),
        global_provider.clone(),
        simulator,
    );
    let task_three_handle = last_minute_bid_task(bend_dao.clone(), global_provider);

    join_all([task_one_handle, task_two_handle, task_three_handle]).await;

    info!("bot is shutting down");

    Ok(())
}

/// listens to benddao events and modifies state
fn bend_dao_event_task(
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
                LendPoolEvents::AuctionFilter(evt) => {
                    if NftAsset::try_from(evt.nft_asset).is_ok() {
                        bd_lock.react_to_auction(evt).await;
                    }
                }
                LendPoolEvents::RedeemFilter(evt) => {
                    if NftAsset::try_from(evt.nft_asset).is_ok() {
                        bd_lock.react_to_redeem(evt).await;
                    }
                }
                LendPoolEvents::LiquidateFilter(evt) => {
                    if let Ok(nft_asset) = NftAsset::try_from(evt.nft_asset) {
                        bd_lock.react_to_liquidation(evt).await;
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
fn nft_oracle_mempool_task(
    provider: Arc<Provider<Ws>>,
    bend_dao_state: Arc<Mutex<BendDao>>,
    global_provider: GlobalProvider,
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

            bend_dao_state
                .lock()
                .await
                .initiate_auctions_if_any(tx, Some(modded_state))
                .await?;

            // sleep and wait for two blocks to be mined so that
            // the refresh includes the latest update
            sleep(Duration::from_secs(12)).await;

            bend_dao_state
                .lock()
                .await
                .refresh_monitored_loans()
                .await?;
        }

        Ok(())
    })
}

/// task that monitors all ongoing auctions
fn last_minute_bid_task(
    bend_dao_state: Arc<Mutex<BendDao>>,
    provider: Arc<Provider<WsClient>>,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let mut stream = provider.subscribe_blocks().await?;

        while let Some(block) = stream.next().await {
            let auctions_due = bend_dao_state
                .lock()
                .await
                .pending_auctions
                .pop_auctions_due(block.timestamp);

            if auctions_due.is_empty() {
                continue;
            }

            // check profitability

            // submit the bundle to `bid`

            // create a new thread and sleep and liquidate
        }
    })
}
