use ethers::types::*;

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Auction {
    pub nft_asset: Address,
    pub nft_token_id: U256,
    pub current_bidder: Address,
    pub current_bid: U256,
    pub bid_end_timestamp: U256, // unix timestamp in seconds
}
