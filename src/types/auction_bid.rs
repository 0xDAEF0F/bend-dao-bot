use ethers::types::*;

pub struct AuctionBid {
    pub nft_asset: H160,
    pub nft_token_id: U256,
    pub bid_price: U256,
}
