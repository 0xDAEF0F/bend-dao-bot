use ethers::types::{Address, U256};

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Auction {
    pub current_bidder: Address,
    pub bid_start_timestamp: U256, // unix timestamp in seconds
    pub current_bid: U256,
}
