pub mod benddao;
pub mod coinmarketcap;
pub mod constants;
pub mod global_provider;
pub mod math;
pub mod prices_client;
pub mod reservoir;
pub mod utils;

use anyhow::Result;
use dotenv::dotenv;
use ethers::{
    contract::abigen,
    core::k256::ecdsa::SigningKey,
    middleware::SignerMiddleware,
    providers::{Http, Provider, Ws},
    signers::{coins_bip39::English, MnemonicBuilder, Wallet},
    types::U256,
};
use std::sync::Arc;
use utils::get_loan_data;

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

enum DualProvider {
    Ws(Arc<SignerMiddleware<Arc<Provider<Ws>>, Wallet<SigningKey>>>),
    Http(Arc<SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>>),
}

struct GProvider(DualProvider);

impl GProvider {
    pub async fn try_new(url: &str, mnemonic: &str) -> Result<GProvider> {
        let local_wallet = MnemonicBuilder::<English>::default()
            .phrase(mnemonic)
            .build()?;

        if url.starts_with("http") {
            Ok(GProvider(DualProvider::Http(Arc::new(
                SignerMiddleware::new(Provider::try_from(url)?.into(), local_wallet),
            ))))
        } else {
            Ok(GProvider(DualProvider::Ws(Arc::new(
                SignerMiddleware::new(Provider::connect(url).await?.into(), local_wallet),
            ))))
        }
    }

    // pub async fn get_updated_loan(&self, loan_id: U256) -> Result<Option<Loan>> {
    //     // let lend_pool = LendPool::new(, client)
    //     get_loan_data(loan_id, self.lend_pool.clone(), self.lend_pool_loan.clone()).await
    // }
}
