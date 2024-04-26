use crate::constants::addresses::{
    BAYC_ADDRESS, MAYC_ADDRESS, USDT_ADDRESS, WETH_ADDRESS, WRAPPED_CRYPTOPUNKS,
};
use crate::prices_client::PricesClient;
use anyhow::{bail, Result};
use core::fmt;
use ethers::types::{Address, U256};
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug)]
pub struct Loan {
    pub loan_id: U256,
    pub status: Status,
    pub nft_token_id: U256,
    pub health_factor: U256,
    pub total_debt: U256, // usdt scaled by 1e6 and eth scaled by 1e18
    pub reserve_asset: ReserveAsset,
    pub nft_asset: NftAsset,
}

impl Loan {
    pub async fn get_total_debt_eth(&self, prices_client: &PricesClient) -> Result<U256> {
        match self.reserve_asset {
            ReserveAsset::Weth => Ok(self.total_debt),
            ReserveAsset::Usdt => {
                let usd_eth_price = prices_client.get_usdt_eth_price().await?;
                let total_debt = self.total_debt * usd_eth_price / U256::exp10(6);
                Ok(total_debt)
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Status {
    Created, // not sure about this state
    Active,
    Auction,
}

#[derive(Debug, PartialEq)]
pub enum ReserveAsset {
    Weth,
    Usdt,
}

impl TryFrom<Address> for ReserveAsset {
    type Error = anyhow::Error;

    fn try_from(value: Address) -> Result<Self, Self::Error> {
        let addr = format!("{:?}", value);
        match addr.as_str() {
            WETH_ADDRESS => Ok(Self::Weth),
            USDT_ADDRESS => Ok(Self::Usdt),
            _ => bail!("could not convert from Address: {} to ReserveAsset", value),
        }
    }
}

impl Display for ReserveAsset {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ReserveAsset::Usdt => write!(f, "{USDT_ADDRESS}"),
            ReserveAsset::Weth => write!(f, "{WETH_ADDRESS}"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum NftAsset {
    Bayc,
    CryptoPunks,
    Mayc,
}

impl TryFrom<Address> for NftAsset {
    type Error = anyhow::Error;

    fn try_from(value: Address) -> Result<NftAsset, Self::Error> {
        let addr = format!("{:?}", value);
        match addr.as_str() {
            BAYC_ADDRESS => Ok(Self::Bayc),
            WRAPPED_CRYPTOPUNKS => Ok(Self::CryptoPunks),
            MAYC_ADDRESS => Ok(Self::Mayc),
            _ => bail!("could not convert from Address: {} to NftAsset", value),
        }
    }
}

impl Display for NftAsset {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            NftAsset::Bayc => write!(f, "{BAYC_ADDRESS}"),
            NftAsset::CryptoPunks => write!(f, "{WRAPPED_CRYPTOPUNKS}"),
            NftAsset::Mayc => write!(f, "{MAYC_ADDRESS}"),
        }
    }
}
