use std::{str::FromStr, sync::Arc};

use anyhow::Result;
use ethers::{
    providers::{Middleware, Provider, Ws},
    types::{Address, Bytes, H256, U256},
    utils::format_ether,
};
use serde::{Deserialize, Serialize};

const URL: &str = "https://eth-mainnet.g.alchemy.com/v2/qIduQHjE0M6sV1a2ak5hEvtPPnjxVJS-";

#[tokio::main]
async fn main() -> Result<()> {
    let client = reqwest::Client::new();
    let provider = Arc::new(
        Provider::<Ws>::connect(
            "wss://eth-mainnet.g.alchemy.com/v2/S1llhLoNFxJdv4K85HALN0xYqNXaa7d0",
        )
        .await?,
    );

    let tx = provider
        .get_transaction(H256::from_str(
            &"0x5bc6db566e6965402b597e8ef85b79c2ef67e87d0144112bb6d19a3623b74cfb",
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

    let req = client
        .post(URL)
        .json(&req)
        .send()
        .await?
        .json::<Res>()
        .await?;

    let logs = req.result.logs;

    for log in logs {
        if &log.topics[0].to_string()
            == "0x58bdf68b6e757afad014720959e6c9ecd94de1cc24b964ebf48b08b50366b321"
        {
            println!(
                "New asset price for {:?} of {}",
                log.topics[1],
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

#[derive(Deserialize)]
struct Log {
    address: Address,
    topics: Vec<String>,
    data: String,
}
