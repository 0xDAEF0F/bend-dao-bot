use super::status::Status;
use crate::constants::*;
use anyhow::{bail, Result};
use core::fmt;
use ethers::types::*;
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
        // EXAMPLE: BAYC #1234 (USDT)
        let display_string = format!(
            "{:?} #{} ({:?})",
            self.nft_asset, self.nft_token_id, self.reserve_asset,
        );
        write!(f, "{display_string}")
    }
}

impl Loan {
    pub fn is_auctionable(&self) -> bool {
        self.health_factor < U256::exp10(18)
    }

    /// `Status::Active && health_factor < 1.05e18`
    pub fn should_monitor(&self) -> bool {
        match self.status {
            Status::Active => self.health_factor < HEALTH_FACTOR_THRESHOLD_TO_MONITOR.into(),
            _ => false,
        }
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

    fn try_from(reserve_asset: Address) -> Result<Self, Self::Error> {
        match reserve_asset.0 {
            WETH => Ok(Self::Weth),
            USDT => Ok(Self::Usdt),
            _ => bail!(
                "could not convert from Address: {} to ReserveAsset",
                reserve_asset
            ),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Hash, Eq)]
pub enum NftAsset {
    Azuki,
    Bayc,
    CryptoPunks,
    Mayc,
    CloneX,
    PudgyPenguins,
    StBayc,
}

pub const ALL_ALLOWED_NFT_ASSETS: [NftAsset; 1] = [NftAsset::CryptoPunks];

impl NftAsset {
    pub fn is_allowed_in_production(&self) -> bool {
        match self {
            NftAsset::CryptoPunks => true,
            NftAsset::Bayc => false,
            NftAsset::StBayc => false,
            NftAsset::CloneX => false,
            NftAsset::PudgyPenguins => false,
            NftAsset::Mayc => false,
            NftAsset::Azuki => false,
        }
    }
}

impl TryFrom<Address> for NftAsset {
    type Error = anyhow::Error;

    fn try_from(value: Address) -> Result<NftAsset, Self::Error> {
        match value.0 {
            AZUKI => Ok(Self::Azuki),
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

impl From<NftAsset> for Address {
    fn from(value: NftAsset) -> Address {
        match value {
            NftAsset::Azuki => AZUKI.into(),
            NftAsset::Bayc => BAYC.into(),
            NftAsset::CryptoPunks => CRYPTOPUNKS.into(),
            NftAsset::Mayc => MAYC.into(),
            NftAsset::CloneX => CLONEX.into(),
            NftAsset::PudgyPenguins => PUDGY_PENGUINS.into(),
            NftAsset::StBayc => STBAYC.into(),
        }
    }
}
