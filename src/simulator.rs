use crate::Config;
use anyhow::Result;
use ethers::types::{Bytes, Transaction, H160, U256};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub struct Simulator {
    url: String,
    client: Client, // pub db: CacheDB<EthersDB<Provider<Ws>>>,
}

impl Simulator {
    pub fn new(cfg: Config) -> Self {
        Self {
            url: format!(
                "https://eth-mainnet.g.alchemy.com/v2/{}",
                cfg.alchemy_api_key
            ),
            client: Client::new(),
        }
    }

    /// Simulates a transaction and returns the updadated twaps
    pub async fn simulate_twap_changes(&self, tx: &Transaction) -> Result<Vec<(H160, U256)>> {
        let req = &Req {
            id: 1,
            jsonrpc: "2.0".to_string(),
            method: "alchemy_simulateExecution".to_string(),
            params: vec![TxObject {
                from: tx.from,
                to: tx.to.unwrap(),
                data: tx.input.clone(),
                value: tx.value,
            }],
        };

        let res = self
            .client
            .post(&self.url)
            .json(&req)
            .send()
            .await?
            .json::<Res>()
            .await?;

        let mut prices = Vec::new();

        for log in res.result.logs {
            if &log.topics[0].to_string()
                == "0x58bdf68b6e757afad014720959e6c9ecd94de1cc24b964ebf48b08b50366b321"
            {
                prices.push((
                    H160::from_str(&log.topics[1]).unwrap(),
                    U256::from_str_radix(&log.data[..66], 16)?,
                ));
            }
        }

        Ok(prices)
    }
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
    from: H160,
    to: H160,
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
    // address: H160,
    topics: Vec<String>,
    data: String,
}

/* impl Simulator {
    pub async fn new(cfg: ConfigVars) -> Self {
        let provider = Provider::<Ws>::connect(&cfg.wss_rpc_url).await.unwrap();
        let ethers_db = EthersDB::new(Arc::new(provider), None).unwrap();
        ethers_db.basic_ref(address);
        let db = CacheDB::new(ethers_db);
        Self { db }
    }

    /// creates cache by doing a simulation of a past price update
    pub async fn create_cache(&mut self) -> Result<()> {
        self.db.load_account(Address::from_slice(&ORACLE_ADDRESS))?;
        Ok(())
    }

    pub async fn simulate_price_change() -> Result<Vec<>> {

    }

} */
