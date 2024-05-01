use ethers::types::U256;

#[derive(Debug)]
pub struct Balances {
    pub eth: U256,
    pub weth: U256,
    pub is_lend_pool_approved: bool, // WETH approval on max
}

impl Balances {
    // TODO: improve refinement 0.1 ETH is too much
    pub fn has_enough_gas_to_auction_or_liquidate(&self) -> bool {
        self.eth >= U256::exp10(17)
    }
}

impl Default for Balances {
    fn default() -> Balances {
        Balances {
            eth: U256::zero(),
            weth: U256::zero(),
            is_lend_pool_approved: false,
        }
    }
}
