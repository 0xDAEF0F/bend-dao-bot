mod auction;
mod auction_bid;
mod balances;
mod pending_auctions;

pub use auction::*;
pub use auction_bid::*;
pub use balances::*;
use ethers::{providers::{JsonRpcClient, Middleware}, signers::Signer};
use ethers_flashbots::{FlashbotsMiddlewareError, PendingBundle};
pub use pending_auctions::*;

pub type SentBundle<'a, M: Middleware, S: Signer, P: JsonRpcClient> = Vec<Result<PendingBundle<'a, P>, FlashbotsMiddlewareError<M, S>>>;
