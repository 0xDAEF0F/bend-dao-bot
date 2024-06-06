use anyhow::Result;
use ethers::providers::{Http, Middleware, Provider};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let rpc_url = std::env::var("MAINNET_RPC_URL")?;
    let provider = Arc::new(Provider::<Http>::try_from(rpc_url)?);

    println!("block is: {}", provider.get_block_number().await?);

    Ok(())
}
