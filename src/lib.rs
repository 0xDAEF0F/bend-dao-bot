pub mod benddao;
pub mod coinmarketcap;
pub mod constants;
pub mod global_provider;
pub mod math;
pub mod prices_client;
pub mod reservoir;
pub mod simulator;
pub mod utils;

use ethers::contract::abigen;
use serde::Deserialize;

abigen!(LendPool, "abi/LendPool.json");
abigen!(LendPoolLoan, "abi/LendPoolLoan.json");
abigen!(NFTOracle, "abi/NFTOracle.json");
abigen!(ReserveOracle, "abi/ReserveOracle.json");
abigen!(Weth, "abi/Weth.json");
abigen!(Erc721, "abi/ERC721.json");
abigen!(Erc20, "abi/ERC20.json");

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub mainnet_rpc_url_ws: String,
    pub mnemonic: String,
    pub alchemy_api_key: String,
    pub reservoir_api_key: String,
    pub coinmarketcap_api_key: String,
    pub slack_url: String,
    pub env: Option<String>,
}
