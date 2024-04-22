use crate::coinmarketcap::price_response::PriceResponse;
use crate::reservoir::floor_response::CollectionBidsResponse;
use anyhow::Result;
use ethers::{
    types::{Address, U256},
    utils::to_checksum,
};
use reqwest::{header::HeaderValue, Client};
use url::Url;

pub struct PricesClient {
    http_client: Client,
}

impl PricesClient {
    pub fn new() -> PricesClient {
        PricesClient {
            http_client: Client::new(),
        }
    }

    // price in ETH
    pub async fn get_best_nft_bid(&self, collection: Address) -> Result<U256> {
        let api_key = dotenv::var("RESERVOIR_API_KEY")?;

        let mut url: Url = "https://api.reservoir.tools".parse()?;

        let path = format!(
            "collections/{}/bids/v1",
            to_checksum(&collection, None).to_lowercase()
        );
        url.set_path(&path);

        let res = self
            .http_client
            .get(url)
            .header("x-api-key", HeaderValue::from_str(&api_key)?)
            .send()
            .await?;
        let res: CollectionBidsResponse = res.json().await?;

        Ok(res.get_best_bid()?)
    }

    // scaled by 1e18
    pub async fn get_usdt_eth_price(&self) -> Result<U256> {
        let eth_usd_price = self.get_eth_usd_price().await?;
        let usdt_usd_price = self.get_usdt_usd_price().await?;

        let price = usdt_usd_price * 1e18 / eth_usd_price;
        let price = price.floor();
        let price = format!("{}", price);

        Ok(U256::from_dec_str(&price)?)
    }

    async fn get_eth_usd_price(&self) -> Result<f64> {
        let api_key = dotenv::var("COINMARKETCAP_API_KEY")?;

        let mut url: Url = "https://pro-api.coinmarketcap.com".parse()?;
        url.set_path("v2/cryptocurrency/quotes/latest");

        url.set_query(Some("id=1027"));

        let res = self
            .http_client
            .get(url)
            .header("X-CMC_PRO_API_KEY", api_key)
            .header("Accept", "application/json")
            .send()
            .await?;

        let res: PriceResponse = res.json().await?;

        Ok(res.get_usd_price())
    }

    async fn get_usdt_usd_price(&self) -> Result<f64> {
        let api_key = dotenv::var("COINMARKETCAP_API_KEY")?;

        let mut url: Url = "https://pro-api.coinmarketcap.com".parse()?;
        url.set_path("v2/cryptocurrency/quotes/latest");
        url.set_query(Some("id=825"));

        let res = self
            .http_client
            .get(url)
            .header("X-CMC_PRO_API_KEY", api_key)
            .header("Accept", "application/json")
            .send()
            .await?;

        let res: PriceResponse = res.json().await?;

        Ok(res.get_usd_price())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::BAYC_ADDRESS;

    #[tokio::test]
    async fn test_get_best_nft_bid() {
        let client = PricesClient::new();

        let price = client
            .get_best_nft_bid(BAYC_ADDRESS.parse().unwrap())
            .await
            .unwrap();

        assert!(price > U256::zero());
    }
}
