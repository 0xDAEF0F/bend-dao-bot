pub mod benddao;
pub mod coinmarketcap;
pub mod constants;
pub mod global_provider;
pub mod math;
pub mod prices_client;
pub mod reservoir;

use anyhow::Result;
use dotenv::dotenv;
use ethers::contract::abigen;

abigen!(LendPool, "abi/LendPool.json");
abigen!(LendPoolLoan, "abi/LendPoolLoan.json");
abigen!(NFTOracle, "abi/NFTOracle.json");
abigen!(ReserveOracle, "abi/ReserveOracle.json");
abigen!(Weth, "abi/Weth.json");
abigen!(Erc721, "abi/ERC721.json");

#[derive(Clone)]
pub struct ConfigVars {
    pub wss_rpc_url: String,
    pub mnemonic: String,
    pub reservoir_api_key: String,
    pub coinmarketcap_api_key: String,
}

impl ConfigVars {
    pub fn try_new() -> Result<ConfigVars> {
        dotenv()?;

        let wss_rpc_url = std::env::var("MAINNET_RPC_URL_WS")?;
        let mnemonic = std::env::var("MNEMONIC")?;
        let reservoir_api_key = std::env::var("RESERVOIR_API_KEY")?;
        let coinmarketcap_api_key = std::env::var("COINMARKETCAP_API_KEY")?;

        let config_vars = ConfigVars {
            wss_rpc_url,
            mnemonic,
            reservoir_api_key,
            coinmarketcap_api_key,
        };

        Ok(config_vars)
    }
}
