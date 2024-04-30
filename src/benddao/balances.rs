use ethers::types::U256;

#[derive(Debug)]
pub struct Balances {
    pub eth: U256,
    pub weth: U256,
    pub is_lend_pool_approved: bool, // WETH approval on max
}

impl Balances {
    // set it for the time being at 0.1 eth
    pub fn has_enough_gas_to_call_auction(&self) -> bool {
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
