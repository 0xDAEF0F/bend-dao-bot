use ethers::types::U256;

use super::Auction;

#[derive(Default)]
pub struct PendingAuctions {
    pending_auctions: Vec<Auction>,
}

impl PendingAuctions {
    pub fn peek(&self) -> Option<&Auction> {
        self.pending_auctions.last()
    }

    /// should give the next auction that will end first.
    pub fn pop(&mut self) -> Option<Auction> {
        self.pending_auctions.pop()
    }

    /// adds a new auction and takes care of sorting them.
    pub fn add_auction(&mut self, auction: Auction) {
        todo!()
    }

    /// removes an auction from state.
    pub fn remove_auction(&mut self, token_id: U256) {
        todo!()
    }
}
