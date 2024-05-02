use super::auction::Auction;
use std::fmt::{Display, Formatter, Result};

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Status {
    Created, // not sure about this state
    Active,
    Auction(Auction),
    RepaidDefaulted,
}

impl Display for Status {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match &self {
            Self::Active => write!(f, "Active"),
            Self::Auction(_) => write!(f, "Auction"),
            Self::Created => write!(f, "Created"),
            Self::RepaidDefaulted => write!(f, "RepaidDefaulted"),
        }
    }
}
