use anyhow::{anyhow, Result};
use ethers::types::U256;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CollectionBidsResponse {
    orders: Vec<Order>,
}

#[derive(Debug, Deserialize)]
struct Order {
    price: Price,
}

#[derive(Debug, Deserialize)]
struct Price {
    #[serde(rename = "netAmount")]
    net_amount: NetAmount,
}

#[derive(Debug, Deserialize)]
struct NetAmount {
    raw: String,
}

impl CollectionBidsResponse {
    pub fn get_best_bid(&self) -> Result<U256> {
        let price = &self
            .orders
            .first()
            .ok_or_else(|| anyhow!("no bids found"))?
            .price
            .net_amount
            .raw;

        Ok(U256::from_dec_str(price)?)
    }
}
