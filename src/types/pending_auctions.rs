use super::Auction;
use crate::constants::{DELAY_FOR_LAST_BID, OUR_EOA_ADDRESS};
use ethers::types::*;
use log::info;

// ideally we dont do this
// imo makes the code ugly
#[derive(Default)]
pub struct PendingAuctions {
    pub pending_auctions: Vec<Auction>,
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
    pub fn add_update_auction(&mut self, auction: Auction) -> bool {
        if let Some(idx) = self.pending_auctions.iter().position(|a| {
            a.nft_asset == auction.nft_asset && a.nft_token_id == auction.nft_token_id
        }) {
            // order does not need to change because the timestamp of the auction remains the same
            self.pending_auctions[idx] = auction;
            true
        } else {
            // add it to the pending auctions
            self.pending_auctions.push(auction);

            // sort it by `bid_end_timestamp` on ascending order
            self.pending_auctions
                .sort_by(|a, b| a.bid_end_timestamp.cmp(&b.bid_end_timestamp));
            false
        }
    }

    /// removes an auction from state.
    pub fn remove_auction(&mut self, nft_asset: Address, nft_token_id: U256) {
        if let Some(idx) = self
            .pending_auctions
            .iter()
            .position(|a| a.nft_asset == nft_asset && a.nft_token_id == nft_token_id)
        {
            self.pending_auctions.remove(idx);
        } else {
            info!("could not remove {:?} #{}", nft_asset, nft_token_id);
        }
    }

    pub fn pop_auctions_due(&mut self, current_timestamp: U256) -> (Vec<Auction>, Vec<Auction>) {
        let mut auctions_due = vec![];
        while let Some(auction) = self.peek() {
            if auction.bid_end_timestamp > current_timestamp + DELAY_FOR_LAST_BID {
                break;
            }
            // if ours, continue if not liquidatable
            if auction.current_bidder == OUR_EOA_ADDRESS.into() {
                if auction.bid_end_timestamp > current_timestamp {
                    continue;
                }
            }
            auctions_due.push(self.pop_first().unwrap());
        }
        let (ours, not_ours): (Vec<_>, Vec<_>) = auctions_due
            .into_iter()
            .partition(|auction| auction.current_bidder == OUR_EOA_ADDRESS.into());

        (ours, not_ours)
    }
}
