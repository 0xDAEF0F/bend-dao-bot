use std::{str::FromStr, sync::Arc};

use anyhow::Result;
use ethers::{
    providers::{Middleware, Provider, Ws},
    types::{Address, Bytes, H160, H256, U256},
    utils::format_ether,
};
use serde::{Deserialize, Serialize};

const URL: &str = "https://eth-mainnet.g.alchemy.com/v2/qIduQHjE0M6sV1a2ak5hEvtPPnjxVJS-";

#[tokio::main]
async fn main() -> Result<()> {
    println!("tst");

    let client = reqwest::Client::new();
    let provider = Arc::new(
        Provider::<Ws>::connect(
            "wss://eth-mainnet.g.alchemy.com/v2/S1llhLoNFxJdv4K85HALN0xYqNXaa7d0",
        )
        .await?,
    );

    println!("tst");


    let tx = provider
        .get_transaction(H256::from_str(
            "0x4ff4e7a963a540815a4b140db14da983fd4be6a0f70cdf9ec7ae20c7b4a11b45",
        )?)
        .await?
        .unwrap();

    let req = &Req {
        id: 1,
        jsonrpc: "2.0".to_string(),
        method: "alchemy_simulateExecution".to_string(),
        params: vec![TxObject {
            from: tx.from,
            to: tx.to.unwrap(),
            data: tx.input,
            value: tx.value,
        }],
    };

    println!("tst");


    let res = client
        .post(URL)
        .json(&req)
        .send()
        .await?
        .json::<Res>()
        .await?;

    let logs = res.result.logs;

    for log in logs {
        if &log.topics[0].to_string()
            == "0x58bdf68b6e757afad014720959e6c9ecd94de1cc24b964ebf48b08b50366b321"
        {
            let addr = if log.topics[1].len() == 42 {
                H160::from_str(&log.topics[1])?
            } else {
                H160::from_str(&log.topics[1].replace("x", "x0"))?
            };
            
            println!(
                "New asset price for {:?} of {}",
                addr,
                format_ether(U256::from_str_radix(&log.data.as_str()[..66], 16)?)
            )
        }
    }

    Ok(())
}

#[derive(Serialize)]
struct Req {
    id: u8,
    jsonrpc: String,
    method: String,
    params: Vec<TxObject>,
}

#[derive(Serialize)]
struct TxObject {
    from: Address,
    to: Address,
    data: Bytes,
    value: U256,
}

#[derive(Deserialize)]
struct Res {
    result: Results,
}

#[derive(Deserialize)]
struct Results {
    logs: Vec<Log>,
}

#[derive(Deserialize, Debug)]
struct Log {
    // address: Address,
    topics: Vec<String>,
    data: String,
}
