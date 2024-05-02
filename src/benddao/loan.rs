use super::status::Status;
use crate::constants::addresses::{
    BAYC, CLONEX, CRYPTOPUNKS, MAYC, PUDGY_PENGUINS, STBAYC, USDT, WETH,
};
use crate::constants::bend_dao::HEALTH_FACTOR_THRESHOLD_TO_MONITOR;
use crate::prices_client::PricesClient;
use anyhow::{bail, Result};
use core::fmt;
use ethers::types::{Address, U256};
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, Clone)]
pub struct Loan {
    pub loan_id: U256,
    pub status: Status,
    pub nft_token_id: U256,
    pub health_factor: U256,
    pub total_debt: U256, // usdt scaled by 1e6 and eth scaled by 1e18
    pub reserve_asset: ReserveAsset,
    pub nft_asset: NftAsset,
}

impl Display for Loan {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // EXAMPLE
        // BAYC #1234 | USDT | HF: 1.0004 | Active
        let display_string = format!(
            "{:?} #{} | {:?} | HF: {:.4} | {}",
            self.nft_asset,
            self.nft_token_id,
            self.reserve_asset,
            self.health_factor(),
            self.status
        );
        write!(f, "{display_string}")
    }
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

    pub fn is_auctionable(&self) -> bool {
        self.health_factor < U256::exp10(18)
    }

    // less than a health factor of 1.05 returns true
    pub fn should_monitor(&self) -> bool {
        self.health_factor < HEALTH_FACTOR_THRESHOLD_TO_MONITOR.into()
    }

    // for displaying purposes
    pub fn health_factor(&self) -> f64 {
        self.health_factor.as_u64() as f64 / 1e18
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ReserveAsset {
    Weth,
    Usdt,
}

impl TryFrom<Address> for ReserveAsset {
    type Error = anyhow::Error;

    fn try_from(value: Address) -> Result<Self, Self::Error> {
        let addr = format!("{:?}", value);
        match addr.as_str() {
            WETH => Ok(Self::Weth),
            USDT => Ok(Self::Usdt),
            _ => bail!("could not convert from Address: {} to ReserveAsset", value),
        }
    }
}

impl Display for ReserveAsset {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ReserveAsset::Usdt => write!(f, "{USDT}"),
            ReserveAsset::Weth => write!(f, "{WETH}"),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum NftAsset {
    Bayc,
    CryptoPunks,
    Mayc,
    CloneX,
    PudgyPenguins,
    StBayc,
}

impl NftAsset {
    pub fn is_allowed_in_production(&self) -> bool {
        match self {
            NftAsset::CryptoPunks => true,
            NftAsset::Bayc => true,
            NftAsset::StBayc => true,
            NftAsset::CloneX => false,
            NftAsset::PudgyPenguins => false,
            NftAsset::Mayc => false,
        }
    }
}

impl TryFrom<Address> for NftAsset {
    type Error = anyhow::Error;

    fn try_from(value: Address) -> Result<NftAsset, Self::Error> {
        let addr = format!("{:?}", value);
        match addr.as_str() {
            BAYC => Ok(Self::Bayc),
            CRYPTOPUNKS => Ok(Self::CryptoPunks),
            MAYC => Ok(Self::Mayc),
            CLONEX => Ok(Self::CloneX),
            STBAYC => Ok(Self::StBayc),
            PUDGY_PENGUINS => Ok(Self::PudgyPenguins),
            _ => bail!("could not convert from Address: {} to NftAsset", value),
        }
    }
}

impl TryFrom<NftAsset> for Address {
    type Error = anyhow::Error;

    fn try_from(value: NftAsset) -> Result<Address, Self::Error> {
        let addr = format!("{:?}", value);
        Ok(addr.parse()?)
    }
}

impl Display for NftAsset {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            NftAsset::Bayc => write!(f, "{BAYC}"),
            NftAsset::CryptoPunks => write!(f, "{CRYPTOPUNKS}"),
            NftAsset::Mayc => write!(f, "{MAYC}"),
            NftAsset::CloneX => write!(f, "{CLONEX}"),
            NftAsset::PudgyPenguins => write!(f, "{PUDGY_PENGUINS}"),
            NftAsset::StBayc => write!(f, "{STBAYC}"),
        }
    }
}
