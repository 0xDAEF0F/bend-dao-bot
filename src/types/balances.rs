use ethers::types::U256;

#[derive(Debug)]
pub struct Balances {
    pub eth: U256,
    pub weth: U256,
    pub usdt: U256,
    pub is_weth_lend_pool_approved: bool, // WETH approval on max
    pub is_usdt_lend_pool_approved: bool, // USDT approval on max
}
