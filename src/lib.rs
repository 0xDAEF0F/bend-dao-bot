pub mod benddao;
pub mod coinmarketcap;
pub mod constants;
pub mod global_provider;
pub mod math;
pub mod prices_client;
pub mod reservoir;
pub mod slack_bot;
pub mod utils;

use anyhow::Result;
use ethers::contract::abigen;

abigen!(LendPool, "abi/LendPool.json");
abigen!(LendPoolLoan, "abi/LendPoolLoan.json");
abigen!(NFTOracle, "abi/NFTOracle.json");
abigen!(ReserveOracle, "abi/ReserveOracle.json");
abigen!(Weth, "abi/Weth.json");
abigen!(Erc721, "abi/ERC721.json");
abigen!(Erc20, "abi/ERC20.json");

#[derive(Clone)]
pub struct ConfigVars {
    pub wss_rpc_url: String,
    pub mnemonic: String,
    pub reservoir_api_key: String,
    pub coinmarketcap_api_key: String,
    pub slack_url: String,
}

impl ConfigVars {
    pub fn try_new() -> Result<ConfigVars> {
        let wss_rpc_url = std::env::var("MAINNET_RPC_URL_WS")?;
        let mnemonic = std::env::var("MNEMONIC")?;
        let reservoir_api_key = std::env::var("RESERVOIR_API_KEY")?;
        let coinmarketcap_api_key = std::env::var("COINMARKETCAP_API_KEY")?;
        let slack_url = std::env::var("SLACK_URL")?;

        let config_vars = ConfigVars {
            wss_rpc_url,
            mnemonic,
            reservoir_api_key,
            coinmarketcap_api_key,
            slack_url,
        };

        Ok(config_vars)
    }
}
