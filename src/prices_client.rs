use crate::reservoir::floor_response::CollectionBidsResponse;
use crate::{benddao::loan::NftAsset, coinmarketcap::price_response::PriceResponse};
use anyhow::Result;
use ethers::types::U256;
use reqwest::{header::HeaderValue, Client};
use url::Url;

#[derive(Debug, Default)]
pub struct PricesClient {
    http_client: Client,
}

impl PricesClient {
    // price in ETH (1e18)
    pub async fn get_best_nft_bid(&self, nft_asset: NftAsset) -> Result<U256> {
        let api_key = dotenv::var("RESERVOIR_API_KEY")?;

        let mut url: Url = "https://api.reservoir.tools".parse()?;

        let path = format!("collections/{}/bids/v1", nft_asset.to_string());
        url.set_path(&path);
        url.set_query(Some("type=collection")); // collection wide bids

        let res = self
            .http_client
            .get(url)
            .header("x-api-key", HeaderValue::from_str(&api_key)?)
            .send()
            .await?;
        let res: CollectionBidsResponse = res.json().await?;

        res.get_best_bid()
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
    use crate::{benddao::loan::Loan, data_source::DataSource};

    use super::*;

    #[tokio::test]
    async fn test_get_best_nft_bid() {
        let client = PricesClient::default();

        let price = client.get_best_nft_bid(NftAsset::Bayc).await.unwrap();

        assert!(price > U256::zero());
    }

    #[tokio::test]
    async fn test_get_profit_for_nft() -> Result<()> {
        let url = dotenv::var("MAINNET_RPC_URL")?;

        let data_source = DataSource::try_new(&url)?;
        let prices_client = PricesClient::default();

        let loan_id = U256::from(13069); // token id: #3599
        let loan: Loan = data_source.get_updated_loan(loan_id).await?.unwrap();

        let _eth_price = prices_client
            .get_best_nft_bid(NftAsset::Mayc)
            .await
            .unwrap();

        let usdt_eth = prices_client.get_usdt_eth_price().await?;
        println!("usdt_eth: {}", usdt_eth);

        let total_debt_eth = loan.total_debt * usdt_eth / U256::exp10(6);

        assert!(total_debt_eth > U256::exp10(18));

        println!("total_debt_eth: {}", total_debt_eth);
        println!("total_debt_usdt: {}", loan.total_debt);

        Ok(())
    }
}
