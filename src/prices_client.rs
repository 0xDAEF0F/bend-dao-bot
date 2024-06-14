use crate::benddao::loan::ALL_ALLOWED_NFT_ASSETS;
use crate::reservoir::floor_response::CollectionBidsResponse;
use crate::Config;
use crate::{benddao::loan::NftAsset, coinmarketcap::price_response::PriceResponse};
use anyhow::Result;
use ethers::types::{Address, U256};
use futures::future::try_join_all;
use reqwest::{header::HeaderValue, Client};
use std::collections::HashMap;
use std::sync::Arc;
use url::Url;

const RESERVOIR_BASE_URL: &str = "https://api.reservoir.tools";
const COINMARKETCAP_BASE_URL: &str = "https://pro-api.coinmarketcap.com";

pub struct PricesClient {
    http_client: Client,
    eth_usd_price: U256,
    prices: HashMap<NftAsset, U256>,
    reservoir_api_key: String,
    coinmarketcap_api_key: String,
}

impl PricesClient {
    pub fn new(config: Config) -> PricesClient {
        PricesClient {
            prices: HashMap::new(),
            eth_usd_price: U256::zero(),
            reservoir_api_key: config.reservoir_api_key,
            coinmarketcap_api_key: config.coinmarketcap_api_key,
            http_client: Client::new(),
        }
    }

    /// Prices in ETH (1e18)
    pub fn get_nft_price(&self, nft_asset: NftAsset) -> U256 {
        let nft_asset = match nft_asset {
            NftAsset::StBayc => NftAsset::Bayc,
            _ => nft_asset,
        };
        *self.prices.get(&nft_asset).unwrap_or(&U256::zero())
    }

    /// Prices in ETH (1e18)
    pub fn get_eth_usd_price(&self) -> U256 {
        self.eth_usd_price
    }

    pub async fn refresh_prices(&mut self) -> Result<()> {
        self.refresh_eth_usd_price().await?;
        self.refresh_nft_prices().await?;
        Ok(())
    }

    async fn refresh_nft_prices(&mut self) -> Result<()> {
        let mut handles = Vec::new();

        let reservoir_api_key = Arc::new(self.reservoir_api_key.clone());
        let client = Arc::new(self.http_client.clone());

        for nft_asset in ALL_ALLOWED_NFT_ASSETS {
            let client = Arc::clone(&client);
            let reservoir_api_key = Arc::clone(&reservoir_api_key);
            let future = tokio::spawn(async move {
                let nft_asset = nft_asset;
                let price =
                    PricesClient::get_best_nft_bid(client, nft_asset, &reservoir_api_key).await?;
                anyhow::Ok((nft_asset, price))
            });
            handles.push(future);
        }

        let result = try_join_all(handles).await?;

        for res in result {
            let (addr, price) = res?;
            self.prices.insert(addr, price);
        }

        Ok(())
    }

    async fn refresh_eth_usd_price(&mut self) -> Result<()> {
        let usd_eth_price = self.get_usd_eth_price().await?;

        let eth_usd_price = (1e18_f64 / usd_eth_price.floor()) as u64;
        let eth_usd_price = U256::from(eth_usd_price);

        self.eth_usd_price = eth_usd_price;

        Ok(())
    }

    /// Price in ETH (1e18)
    async fn get_best_nft_bid(
        client: Arc<Client>,
        nft_asset: NftAsset,
        reservoir_api_key: &str,
    ) -> Result<U256> {
        let nft_asset = match nft_asset {
            NftAsset::StBayc => NftAsset::Bayc,
            nft_asset => nft_asset,
        };
        let mut url: Url = RESERVOIR_BASE_URL.parse()?;
        let path = format!("collections/{:?}/bids/v1", Address::from(nft_asset));
        url.set_path(&path);
        url.set_query(Some("type=collection")); // collection wide bids

        let res = client
            .get(url)
            .header("x-api-key", HeaderValue::from_str(reservoir_api_key)?)
            .send()
            .await?;
        let res: CollectionBidsResponse = res.json().await?;

        res.get_best_bid()
    }

    async fn get_usd_eth_price(&self) -> Result<f64> {
        let mut url: Url = COINMARKETCAP_BASE_URL.parse()?;
        url.set_path("v2/cryptocurrency/quotes/latest");
        url.set_query(Some("id=1027"));

        let res = self
            .http_client
            .get(url)
            .header("X-CMC_PRO_API_KEY", &self.coinmarketcap_api_key)
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
    use ethers::utils::parse_ether;

    #[tokio::test]
    async fn test_eth_price() -> Result<()> {
        dotenv::dotenv().ok();
        let config_vars: Config = envy::from_env()?;

        let mut client = PricesClient::new(config_vars);

        client.refresh_eth_usd_price().await?;

        let eth_usd_price = client.get_eth_usd_price();

        println!("eth_usd_price: {}", eth_usd_price);

        // unlikely 1 ETH < 999 USD
        assert!(eth_usd_price > U256::from((1_f64 / 999_f64).floor() as u64));

        Ok(())
    }

    #[tokio::test]
    async fn test_bayc_price() -> Result<()> {
        dotenv::dotenv().ok();
        let config_vars: Config = envy::from_env()?;

        let mut client = PricesClient::new(config_vars);

        client.refresh_prices().await?;

        let bayc_eth_price = client.get_nft_price(NftAsset::Bayc);

        println!("bayc_eth_price: {}", bayc_eth_price);

        // unlikely 1 BAYC < 1 ETH
        assert!(bayc_eth_price > parse_ether("1").unwrap());

        let eth_usd = client.get_eth_usd_price();
        let bayc_usd = bayc_eth_price * U256::exp10(6) / eth_usd;

        println!("bayc_usd_price: {}", bayc_usd);

        // unlikely 1 BAYC < 10_000 USD
        assert!(bayc_usd > U256::from(10_000) * U256::exp10(6));

        Ok(())
    }
}
