#![cfg(test)]

use std::str::FromStr;

use anyhow::Result;
use ethers::signers::{LocalWallet, Signer};

#[tokio::test]
async fn test_local_wallet() -> Result<()> {
    let private_key = dotenv::var("PRIVATE_KEY")?;
    let local_wallet = LocalWallet::from_str(&private_key)?;

    let address = local_wallet.address();

    println!("address: {:?}", address);

    Ok(())
}
