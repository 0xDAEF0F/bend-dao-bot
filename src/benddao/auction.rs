use crate::constants::math::{ONE_DAY, ONE_MINUTE};
use chrono::Utc;
use ethers::{
    signers::{LocalWallet, Signer},
    types::{Address, U256},
};
use tokio::time::{Duration, Instant};

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Auction {
    pub highest_bidder: Address,
    pub bid_start_timestamp: U256, // unix timestamp in seconds
}

impl Auction {
    pub fn get_bid_end(&self) -> Instant {
        let time_elapsed_in_aution =
            Utc::now().timestamp() as u64 - self.bid_start_timestamp.as_u64();

        let cushion_time = ONE_MINUTE * 5;

        Instant::now() + Duration::from_secs(ONE_DAY + cushion_time - time_elapsed_in_aution)
    }

    pub fn is_ours(&self, local_wallet: &LocalWallet) -> bool {
        self.highest_bidder == local_wallet.address()
    }
}
