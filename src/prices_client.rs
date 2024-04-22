use anyhow::{anyhow, Result};
use dotenv::var;
use ethers::{
    types::{Address, U256},
    utils::to_checksum,
};
use reqwest::{header::HeaderValue, Client};
use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
struct CollectionBidsResponse {
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

pub struct PricesClient {
    http_client: Client,
    base_url: Url,
}

impl PricesClient {
    pub fn try_new() -> Result<PricesClient> {
        let api_key = var("RESERVOIR_API_KEY")?;

        let mut default_headers = reqwest::header::HeaderMap::new();
        default_headers.insert("x-api-key", HeaderValue::from_str(&api_key)?);

        let http_client = Client::builder().default_headers(default_headers).build()?;

        Ok(PricesClient {
            http_client,
            base_url: Url::parse("https://api.reservoir.tools")?,
        })
    }

    pub async fn get_best_nft_bid(&self, collection: Address) -> Result<U256> {
        let mut url = self.base_url.clone();

        let path = format!(
            "collections/{}/bids/v1",
            to_checksum(&collection, None).to_lowercase()
        );
        url.set_path(&path);

        let res = self.http_client.get(url).send().await?;

        let response: CollectionBidsResponse = res.json().await?;

        let price = &response
            .orders
            .first()
            .ok_or_else(|| anyhow!("no bids found"))?
            .price
            .net_amount
            .raw;

        let price = U256::from_dec_str(&price)?;

        Ok(price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::BAYC_ADDRESS;

    #[tokio::test]
    async fn test_get_best_nft_bid() {
        let client = PricesClient::try_new().unwrap();

        let price = client
            .get_best_nft_bid(BAYC_ADDRESS.parse().unwrap())
            .await
            .unwrap();

        assert!(price > U256::zero());
    }
}
