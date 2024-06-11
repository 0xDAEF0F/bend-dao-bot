use ethers::types::*;
use crate::benddao::loan::ReserveAsset;

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Auction {
    pub nft_asset: Address,
    pub nft_token_id: U256,
    pub current_bid: U256,
    pub bid_end_timestamp: U256, // unix timestamp in seconds
    // idk why it needs to be U64
    pub bid_end_block_number: u64,
    // matters for profit calculation
    pub token: ReserveAsset
}