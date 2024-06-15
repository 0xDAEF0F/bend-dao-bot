use anyhow::Result;
use bend_dao_collector::benddao::loan::NftAsset;
use bend_dao_collector::benddao::BendDao;
use bend_dao_collector::constants::*;
use bend_dao_collector::global_provider::GlobalProvider;
use bend_dao_collector::lend_pool::LendPool;
use bend_dao_collector::prices_client::PricesClient;
use bend_dao_collector::simulator::Simulator;
use bend_dao_collector::spoofer::get_new_state_with_twaps_modded;
use bend_dao_collector::{Config, LendPoolEvents};
use ethers::providers::Middleware;
use ethers::utils::format_ether;
use ethers::{
    providers::{Provider, StreamExt, Ws},
    types::*,
};
use futures::future::try_join_all;
use log::{error, info};
use messenger_rs::slack_hook::SlackClient;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let config: Config = envy::from_env()?;

    let prices_client = PricesClient::new(config.clone());
    let prices_client = Arc::new(RwLock::new(prices_client));
    let mut bend_dao = BendDao::try_new(config.clone(), prices_client.clone()).await?;

    let provider = bend_dao.get_provider();

    // KILL THIS
    let global_provider = Arc::new(bend_dao.get_global_provider());

    let slack = bend_dao.slack_bot.clone();

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
    let task_three_handle =
        last_minute_bid_task(bend_dao.clone(), global_provider, Arc::new(slack));
    let task_four_handle = refresh_nft_prices_task(prices_client);

    try_join_all([
        task_one_handle,
        task_two_handle,
        task_three_handle,
        task_four_handle,
    ])
    .await?;

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
                    if let Ok(_nft_asset) = NftAsset::try_from(evt.nft_asset) {
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
    global_provider: Arc<GlobalProvider>,
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

            info!("NftOracle posted prices");

            let twaps = simulator.simulate_twap_changes(&tx).await?;

            for &(addr, price) in twaps.iter() {
                if let Ok(nft_asset) = NftAsset::try_from(addr) {
                    info!("{:?}: {}", nft_asset, format_ether(price));
                }
            }

            let modded_state = get_new_state_with_twaps_modded(twaps);

            if let Some(bundle) = bend_dao_state
                .lock()
                .await
                .initiate_auctions_if_any(tx, Some(modded_state))
                .await?
            {
                match global_provider.send_and_handle_bundle(bundle).await {
                    Ok(_) => {
                        info!("bundle sent successfully");
                    }
                    Err(e) => {
                        error!("error sending bundle: {}", e);
                    }
                }
            }

            // sleep and wait for two blocks to be mined so that
            // the refresh includes the latest update
            sleep(Duration::from_secs(24)).await;

            bend_dao_state
                .lock()
                .await
                .refresh_monitored_loans()
                .await?;
        }

        Ok(())
    })
}

/// Task that monitors all ongoing auctions
fn last_minute_bid_task(
    bend_dao_state: Arc<Mutex<BendDao>>,
    global_provider: Arc<GlobalProvider>,
    slack: Arc<SlackClient>,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let provider = global_provider.provider.clone();
        let mut stream = provider.subscribe_blocks().await?;

        while let Some(block) = stream.next().await {
            let (ours, not_ours) = {
                bend_dao_state
                    .lock()
                    .await
                    .pending_auctions
                    .pop_auctions_due(block.timestamp)
            };

            for auctions_due in ours.iter().chain(not_ours.iter()) {
                let msg = format!(
                    "Auction due to outbid: {:?} #{}\n",
                    auctions_due.nft_asset, auctions_due.nft_token_id
                );
                let msg_ = format!("Bid ends: {}\n", auctions_due.bid_end_timestamp);
                let msg__ = format!("Current timestamp: {}", block.timestamp);
                info!("{}{}{}", msg, msg_, msg__);
            }

            let bundles = bend_dao_state
                .lock()
                .await
                .verify_and_package_outbids(&not_ours)
                .await?;

            for (i, bundle) in bundles.into_iter().enumerate() {
                let global_provider_clone = global_provider.clone();
                let slack_clone = slack.clone();
                let auction = not_ours[i];
                tokio::spawn(async move {
                    match global_provider_clone.send_and_handle_bundle(bundle).await {
                        Ok(_) => {
                            let message = format!(
                                "bid for {:?} #{:?}sent successfully, waiting 2 block to liquidate",
                                auction.nft_asset, auction.nft_token_id
                            );
                            info!("{}", message);

                            if let Err(e) = slack_clone.send_message(message).await {
                                error!("failed to send slack message {e}");
                            }

                            sleep(Duration::from_secs(24)).await;
                            match global_provider_clone.liquidate_loan(&auction).await {
                                Ok(_) => {
                                    let message = format!(
                                        "liquidated https://www.benddao.xyz/en/auctions/bid/{:?}/{:?} successfully",
                                        auction.nft_asset, auction.nft_token_id
                                    );
                                    info!("{}", message);
                                    if let Err(e) = slack_clone.send_message(message).await {
                                        error!("failed to send slack message {e}");
                                    }
                                }
                                Err(e) => {
                                    error!("error sending bundle: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("error sending bundle: {}", e);
                        }
                    }
                });
            }

            for auction in ours {
                match global_provider.liquidate_loan(&auction).await {
                    Ok(_) => {
                        let message = format!(
                            "liquidated https://www.benddao.xyz/en/auctions/bid/{:?}/{} successfully",
                            auction.nft_asset, auction.nft_token_id
                        );
                        info!("{}", message);
                        if let Err(e) = slack.send_message(message).await {
                            error!("failed to send slack message {e}");
                        }
                    }
                    Err(e) => {
                        error!("error sending bundle: {}", e);
                    }
                }
            }
        }

        Ok(())
    })
}

fn refresh_nft_prices_task(prices_client: Arc<RwLock<PricesClient>>) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        loop {
            prices_client.write().await.refresh_prices().await?;
            sleep(Duration::from_secs(4 * ONE_HOUR)).await;
        }
    })
}
