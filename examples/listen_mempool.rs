use std::{io::Read, str::FromStr, sync::Arc};

use anyhow::Result;
use ethers::prelude::*;
use ethers_flashbots::{BundleRequest, FlashbotsMiddleware, SimulatedTransaction};
use rand::thread_rng;
use revm::{db::{CacheDB, EthersDB}, primitives::{Address, Bytes, TransactTo, U256 as Uint256}, Evm};
use ethers::utils::hex;

#[tokio::main]
async fn main() -> Result<()> {
    let provider = Provider::<Ws>::connect("wss://eth-mainnet.g.alchemy.com/v2/S1llhLoNFxJdv4K85HALN0xYqNXaa7d0").await?;
    let provider = Arc::new(provider);

    let ethers_db = EthersDB::new(provider.clone(),Some(BlockId::Number(BlockNumber::Number(U64::from(20034389))))).unwrap();
    let cache_db = CacheDB::new(ethers_db);

    println!("test");

    let mut stream = provider.subscribe_pending_txs().await?;

    while let Some(hash) = stream.next().await {

        let tx = provider.get_transaction(H256::from_str("0xdf68a39ebcf7b40c7c073cf3c3653691d6822604a942cc38eebd6196efafb3f5")?).await?.unwrap();
        // println!("Tx: {:?}", tx);

        if tx.input.is_empty() {
            continue;
        }
        let mut evm = Evm::builder()
            .with_db(cache_db.clone())
            .modify_tx_env(|env_tx| {
                env_tx.caller = Address::from(tx.from.as_fixed_bytes());
                env_tx.transact_to = TransactTo::Call(Address::from(tx.to.unwrap().as_fixed_bytes()));
                env_tx.data = Bytes::from(hex::decode(tx.input.to_string()).unwrap());
                env_tx.gas_price = Uint256::from(tx.gas_price.unwrap().as_u128());
                env_tx.gas_limit = tx.gas.as_u64();
            }).modify_env(|env| {
                env.block.timestamp = Uint256::from(1717704166
                );
            })
            .build();

            println!("Tx: {:?}", evm.tx());

            let ref_tx = evm.transact().unwrap();
            // select ExecutionResult struct
            let result = ref_tx.result;

            println!("Result: {:?}", result);
            break;
    }


    Ok(())
}