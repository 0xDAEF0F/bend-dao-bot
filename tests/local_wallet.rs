#![cfg(test)]

use anyhow::Result;
use ethers::signers::{coins_bip39::English, LocalWallet, MnemonicBuilder, Signer};
use std::str::FromStr;

#[tokio::test]
async fn test_local_wallet() -> Result<()> {
    let private_key = dotenv::var("PRIVATE_KEY")?;
    let local_wallet = LocalWallet::from_str(&private_key)?;

    let address = local_wallet.address();

    println!("address: {:?}", address);

    Ok(())
}

#[tokio::test]
async fn test_local_wallet_from_mnemonic() -> Result<()> {
    let mnemonic = "test test test test test test test test test test test junk";
    let local_wallet = MnemonicBuilder::<English>::default()
        .phrase(mnemonic)
        .build()?;

    assert_eq!(
        local_wallet.address(),
        "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse()?
    );

    Ok(())
}
