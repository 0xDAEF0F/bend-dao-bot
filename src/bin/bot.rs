use anyhow::Result;
use bend_dao_collector::lend_pool::LendPool;
use bend_dao_collector::LendPoolEvents;
use bend_dao_collector::{benddao_state::BendDao, constants::bend_dao::LEND_POOL};
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

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;
    env_logger::init();

    let bend_dao = BendDao::try_new()?;
    let bend_dao = Arc::new(Mutex::new(bend_dao));

    let wss_url = std::env::var("MAINNET_RPC_URL_WS")?;
    let provider = Provider::<Ws>::connect(wss_url).await?;

    info!(
        "current block number is: {}",
        provider.get_block_number().await?
    );

    let provider = Arc::new(provider);

    bend_dao
        .lock()
        .await
        .build_all_loans(provider.clone())
        .await?;

    let provider_ = provider.clone();
    let bend_dao_ = bend_dao.clone();
    let task_one_handle = tokio::spawn(async move {
        info!("starting event listener task for lend pool events");

        let lend_pool: Address = LEND_POOL.parse()?;
        let lend_pool = LendPool::new(lend_pool, provider_);

        let events = lend_pool.events();
        let mut stream = events.subscribe().await?;

        while let Some(Ok(evt)) = stream.next().await {
            let mut lock = bend_dao_.lock().await;
            match evt {
                LendPoolEvents::BorrowFilter(evt) => {
                    // a loan has been created or borrowed more
                    lock.handle_borrow(evt.loan_id).await?;
                }
                LendPoolEvents::RepayFilter(evt) => {
                    // repayment occured. either partial or total
                    lock.handle_repay_loan(evt.loan_id).await?;
                }
                LendPoolEvents::AuctionFilter(evt) => {
                    // take out of loans and into pending auctions
                    lock.handle_auction(evt.loan_id).await?;
                }
                LendPoolEvents::LiquidateFilter(evt) => {
                    lock.handle_liquidation(evt.loan_id);
                }
                LendPoolEvents::RedeemFilter(evt) => {
                    // loan has been partially repaid by owner and
                    // moved from auctions to active
                    lock.handle_redeem(evt.loan_id).await?;
                }
                _ => {}
            }
        }

        info!("returning event listener task for lend pool");
        anyhow::Ok(())
    });

    let provider_ = provider.clone();
    let bend_dao_ = bend_dao.clone();
    let task_two_handle = tokio::spawn(async move {
        info!("starting task for new blocks");

        let mut stream = provider_.subscribe_blocks().await?;

        while let Some(_block) = stream.next().await {
            let mut lock = bend_dao_.lock().await;
            lock.handle_new_block().await?;
        }

        info!("ending task for new blocks");
        anyhow::Ok(())
    });

    let _ = join_all([task_one_handle, task_two_handle]).await;

    info!("ending bot program");
    Ok(())
}
