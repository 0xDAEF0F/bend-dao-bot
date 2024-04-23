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
use std::sync::Arc;
use tokio::sync::Mutex;

// 1. build all current loans with health factors
// 2. group the health factors below 1.1 in a separate container
// 3. every block check for that containers health factors if they go below 1
// 4. if some loan is auctionable check if there is a profit

// events that are important to listen:
// -
// let url = "ws://103.219.171.12:8546";
// let mut bend_dao = BendDao::try_new(url)?;
// bend_dao.build_all_loans().await?;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;

    let bend_dao = BendDao::new();
    let bend_dao = Arc::new(Mutex::new(bend_dao));

    let wss_url = std::env::var("MAINNET_RPC_URL_WS")?;

    let provider = Provider::<Ws>::connect(wss_url).await?;
    let provider = Arc::new(provider);

    let provider_ = provider.clone();
    let bend_dao_ = bend_dao.clone();
    let handle = tokio::spawn(async move {
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
        anyhow::Ok(())
    });

    let provider_ = provider.clone();
    let bend_dao_ = bend_dao.clone();
    let handle2 = tokio::spawn(async move {
        let mut stream = provider_.subscribe_blocks().await?;

        while let Some(_block) = stream.next().await {
            let mut lock = bend_dao_.lock().await;
            lock.handle_new_block().await?;
        }

        anyhow::Ok(())
    });

    Ok(())
}
