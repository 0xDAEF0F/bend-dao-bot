use ethers::types::U256;
use log::warn;

use super::{
    calculate_bidding_amount,
    loan::{Loan, ReserveAsset},
};

#[derive(Debug)]
pub struct Balances {
    pub eth: U256,
    pub weth: U256,
    pub usdt: U256,
    pub is_weth_lend_pool_approved: bool, // WETH approval on max
    pub is_usdt_lend_pool_approved: bool, // USDT approval on max
}

impl Balances {
    /// 1] is contract max approved?
    /// 2] do we have enough ETH to cover gas costs?
    /// 3] do we have enough USDT/WETH to cover `total_debt + cushion`?
    pub fn can_initiate_auction(&self, loan: &Loan) -> bool {
        // handles logging
        if !self.is_usdt_weth_approved(loan) {
            return false;
        }

        if !self.has_enough_gas_to_auction_or_liquidate() {
            warn!("{:5>}", "not enough ETH to pay for gas costs");
            return false;
        }

        // handles logging
        if !self.has_enough_funds_to_participate_in_auction(loan) {
            return false;
        }

        true
    }

    // TODO: improve refinement 0.1 ETH is too much
    pub fn has_enough_gas_to_auction_or_liquidate(&self) -> bool {
        self.eth >= U256::exp10(17)
    }

    fn is_usdt_weth_approved(&self, loan: &Loan) -> bool {
        match loan.reserve_asset {
            ReserveAsset::Usdt => {
                if self.is_usdt_lend_pool_approved {
                    true
                } else {
                    warn!("{:>5}", "USDT not approved for LendPool");
                    false
                }
            }
            ReserveAsset::Weth => {
                if self.is_weth_lend_pool_approved {
                    true
                } else {
                    warn!("{:>5}", "WETH not approved for LendPool");
                    false
                }
            }
        }
    }

    fn has_enough_funds_to_participate_in_auction(&self, loan: &Loan) -> bool {
        let bidding_amount = calculate_bidding_amount(loan.total_debt);
        match loan.reserve_asset {
            ReserveAsset::Usdt => {
                if self.usdt >= bidding_amount {
                    true
                } else {
                    warn!("{:>5}", "not enough USDT to participate in auction");
                    false
                }
            }
            ReserveAsset::Weth => {
                if self.weth >= bidding_amount {
                    true
                } else {
                    warn!("{:>5}", "not enough WETH to participate in auction");
                    false
                }
            }
        }
    }
}
