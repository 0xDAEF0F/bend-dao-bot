#![cfg(test)]

use anyhow::Result;
use bend_dao_collector::benddao_state::BendDao;
use dotenv::dotenv;

#[tokio::test]
async fn get_all_active_loans() -> Result<()> {
    dotenv()?;

    let mut bend_dao = BendDao::build(&dotenv::var("MAINNET_RPC_URL")?)?;

    bend_dao.build_all_loans().await?;

    println!("Loans: {:#?}", bend_dao.loans);

    Ok(())
}
