use std::{borrow::Borrow, io::Read, str::FromStr, sync::Arc};

use abi::AbiEncode;
use anyhow::Result;
use ethers::{etherscan::contract, prelude::*};
use ethers_flashbots::{BundleRequest, FlashbotsMiddleware, SimulatedTransaction};
use gas_oracle::cache;
use k256::pkcs8::der::Encode;
use rand::thread_rng;
use revm::{db::{CacheDB, EmptyDB, EthersDB}, interpreter::sload, primitives::{Address, Bytes, TransactTo, U256 as Uint256}, Database, DatabaseRef, Evm, JournaledState};
use ethers::utils::hex;

abigen!(NFTOracle, "abi/NFTOracle.json");


#[tokio::main]
async fn main() -> Result<()> {
    let provider = Provider::<Ws>::connect("wss://eth-mainnet.g.alchemy.com/v2/S1llhLoNFxJdv4K85HALN0xYqNXaa7d0").await?;
    let provider = Arc::new(provider);

    let addr = Address::from_str("0x7C2A19e54e48718f6C60908a9Cff3396E4Ea1eBA")?;

    let ethers_db = EthersDB::new(provider.clone(), Some(BlockId::Number(BlockNumber::Number(U64::from(20041297))))).unwrap();
    let info = ethers_db.basic_ref(addr)?.unwrap();
    let mut cache_db = CacheDB::new(ethers_db.clone());
    cache_db.insert_account_info(addr, info);

    println!("{cache_db:#?}");

    let mut stream = provider.subscribe_full_pending_txs().await?;

    while let Some(hash) = stream.next().await {
        let tx = provider.get_transaction(H256::from_str("0x5e6fd12eebbd323aa86ab507c89b546e00fed07ca3f13d45a4f0b6fd285b7913")?).await?.unwrap();
        // println!("Tx: {:?}", tx);

        let timestamp = provider.get_block(tx.block_number.unwrap()).await?.unwrap().timestamp.as_u64();

        let current_time = chrono::Utc::now();

        let mut evm = Evm::builder()
            .with_db(cache_db.clone())
            .modify_tx_env(|env_tx| {
                env_tx.caller = Address::from(tx.from.as_fixed_bytes());
                env_tx.transact_to = TransactTo::Call(Address::from(tx.to.unwrap().as_fixed_bytes()));
                env_tx.data = Bytes::from(hex::decode(tx.input.to_string()).unwrap());
                env_tx.gas_price = Uint256::from(tx.gas_price.unwrap().as_u128());
                env_tx.gas_limit = tx.gas.as_u64();
            }).modify_env(|env| {
                println!("timestamp {:?}, {:?}", env.block.timestamp, timestamp);
                env.block.timestamp = Uint256::from(timestamp);

            })
            .build();   

            println!("Tx: {:?}", evm.tx());

            let ref_tx = evm.transact().unwrap();

            // select ExecutionResult struct
            let result = ref_tx.result;

            let new_time = chrono::Utc::now();

            println!("Result: {:?}", result.is_success());
            println!("Time: {:?}", new_time.signed_duration_since(current_time));

            let state = ref_tx.state;

            for (account_touched, change) in &state {
                for (slot, value) in &change.storage {
                    let true_value = provider.get_storage_at(H160::from_str(&account_touched.to_string())?, H256::from_uint(&U256::from_dec_str(&slot.to_string())?), Some(BlockId::Number(BlockNumber::Number(U64::from(20041299))))).await?;
                    if U256::from_str_radix(&format!("{:?}", true_value), 16)?.to_string() != value.present_value.to_string() {
                        println!("WARINGINGINAS IGN {:?} : {:?}", U256::from_str_radix(&format!("{:?}", true_value), 16)?, value.present_value.to_string());
                    }
                    cache_db.insert_account_storage(*account_touched, *slot, value.present_value)?;
                }
            }

            // println!("{cache_db:#?}");





        let tx = provider.get_transaction(H256::from_str("0x78e2486ea7808c51524884e2d4389e87d073d252e05aaa30da9c16c7fab5eadc")?).await?.unwrap();
        // println!("Tx: {:?}", tx);

        let timestamp = provider.get_block(tx.block_number.unwrap()).await?.unwrap().timestamp.as_u64();

        let current_time = chrono::Utc::now();

        let mut evm = Evm::builder()
            .with_db(cache_db.clone())
            .modify_tx_env(|env_tx| {
                env_tx.caller = Address::from(tx.from.as_fixed_bytes());
                env_tx.transact_to = TransactTo::Call(Address::from(tx.to.unwrap().as_fixed_bytes()));
                env_tx.data = Bytes::from(hex::decode(tx.input.to_string()).unwrap());
                env_tx.gas_price = Uint256::from(tx.gas_price.unwrap().as_u128());
                env_tx.gas_limit = tx.gas.as_u64();
            }).modify_env(|env| {
                println!("timestamp {:?}, {:?}", env.block.timestamp, timestamp);
                env.block.timestamp = Uint256::from(timestamp);

            })
            .build();   

            let ref_tx = evm.transact().unwrap();

            // select ExecutionResult struct
            let result = ref_tx.result;

            let new_time = chrono::Utc::now();

            println!("Result: {:?}", result.is_success());
            println!("Time: {:?}", new_time.signed_duration_since(current_time));

            let state = ref_tx.state;

            for (account_touched, change) in &state {
                for (slot, value) in &change.storage {
                    let true_value = provider.get_storage_at(H160::from_str(&account_touched.to_string())?, H256::from_uint(&U256::from_dec_str(&slot.to_string())?), Some(BlockId::Number(BlockNumber::Number(U64::from(20043083))))).await?;
                    if U256::from_str_radix(&format!("{:?}", true_value), 16)?.to_string() != value.present_value.to_string() {
                        println!("WARINGINGINAS IGN {:?} : {:?}", U256::from_str_radix(&format!("{:?}", true_value), 16)?, value.present_value.to_string());
                    }
                    cache_db.insert_account_storage(*account_touched, *slot, value.present_value)?;
                }
            }

        println!("{cache_db:#?}");


        let tx = provider.get_transaction(H256::from_str("0x5bc6db566e6965402b597e8ef85b79c2ef67e87d0144112bb6d19a3623b74cfb")?).await?.unwrap();
        // println!("Tx: {:?}", tx);

        let timestamp = provider.get_block(tx.block_number.unwrap()).await?.unwrap().timestamp.as_u64();

        let current_time = chrono::Utc::now();

        let mut evm = Evm::builder()
            .with_db(cache_db.clone())
            .modify_tx_env(|env_tx| {
                env_tx.caller = Address::from(tx.from.as_fixed_bytes());
                env_tx.transact_to = TransactTo::Call(Address::from(tx.to.unwrap().as_fixed_bytes()));
                env_tx.data = Bytes::from(hex::decode(tx.input.to_string()).unwrap());
                env_tx.gas_price = Uint256::from(tx.gas_price.unwrap().as_u128());
                env_tx.gas_limit = tx.gas.as_u64();
            }).modify_env(|env| {
                println!("timestamp {:?}, {:?}", env.block.timestamp, timestamp);
                env.block.timestamp = Uint256::from(timestamp);

            })
            .build();

            let time_2 = chrono::Utc::now();

            let res = evm.transact().unwrap();

            let time_3 = chrono::Utc::now();
            
            println!("sim time: {:?}", time_3.signed_duration_since(time_2));
            

            println!("Result: {:?}", res.result);

            let newer_time = chrono::Utc::now();

            println!("Time 2: {:?}", newer_time.signed_duration_since(current_time));

            break;
    }


    Ok(())
}