mod auction;
mod auction_bid;
mod balances;
mod pending_auctions;

pub use auction::*;
pub use auction_bid::*;
pub use balances::*;
use ethers_flashbots::{FlashbotsMiddlewareError, PendingBundle};
pub use pending_auctions::*;

pub type SentBundle<'a, M, S, P> =
    Vec<Result<PendingBundle<'a, P>, FlashbotsMiddlewareError<M, S>>>;
