use super::Auction;
use ethers::types::*;

#[derive(Default)]
pub struct PendingAuctions {
    pending_auctions: Vec<Auction>,
}

impl PendingAuctions {
    /// peek ahead into the next auction that will expire next.
    pub fn peek(&self) -> Option<&Auction> {
        self.pending_auctions.first()
    }

    /// should give the next auction that will end first.
    pub fn pop_first(&mut self) -> Option<Auction> {
        if self.pending_auctions.is_empty() {
            None
        } else {
            Some(self.pending_auctions.remove(0))
        }
    }

    /// adds a new auction and takes care of sorting them.
    pub fn update_auction(&mut self, auction: Auction) {
        if let Some(idx) = self.pending_auctions.iter().position(|a| {
            a.nft_asset == auction.nft_asset && a.nft_token_id == auction.nft_token_id
        }) {
            // order does not need to change because the timestamp of the auction remains the same
            self.pending_auctions[idx] = auction;
            return;
        }

        // add it to the pending auctions
        self.pending_auctions.push(auction);

        // sort it by `bid_end_timestamp` on ascending order
        self.pending_auctions
            .sort_by(|a, b| a.bid_end_timestamp.cmp(&b.bid_end_timestamp));
    }

    /// removes an auction from state.
    pub fn remove_auction(&mut self, nft_asset: Address, nft_token_id: U256) {
        if let Some(idx) = self
            .pending_auctions
            .iter()
            .position(|a| a.nft_asset == nft_asset && a.nft_token_id == nft_token_id)
        {
            self.pending_auctions.remove(idx);
        }
    }
}
