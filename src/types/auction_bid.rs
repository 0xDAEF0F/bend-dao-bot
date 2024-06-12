use ethers::types::*;

use super::Auction;

pub struct AuctionBid {
    pub nft_asset: H160,
    pub nft_token_id: U256,
    pub bid_price: U256,
}

impl AuctionBid {
    pub fn new(auction: &Auction, bid_price: U256) -> Self {
        Self {
            nft_asset: auction.nft_asset,
            nft_token_id: auction.nft_token_id,
            bid_price,
        }
    }
}
