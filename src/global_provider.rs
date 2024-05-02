use crate::{
    benddao::loan::{Loan, NftAsset},
    constants::{
        addresses::{USDT, WETH},
        bend_dao::{LEND_POOL, LEND_POOL_LOAN, NFT_ORACLE, RESERVE_ORACLE},
    },
    utils::get_loan_data,
    ConfigVars, Erc20, LendPool, LendPoolLoan, NFTOracle, ReserveOracle, Weth,
};
use anyhow::{bail, Result};
use ethers::{
    core::k256::ecdsa::SigningKey,
    middleware::SignerMiddleware,
    providers::{Middleware, Provider, Ws},
    signers::{coins_bip39::English, LocalWallet, MnemonicBuilder, Signer, Wallet},
    types::{Address, U256},
};
use futures::future::join_all;
use log::info;
use std::sync::Arc;
use tokio::task::JoinHandle;

pub struct GlobalProvider {
    pub local_wallet: LocalWallet,
    pub provider: Arc<Provider<Ws>>,
    pub signer_provider: Arc<SignerMiddleware<Arc<Provider<Ws>>, Wallet<SigningKey>>>,
    pub lend_pool: LendPool<Provider<Ws>>,
    pub lend_pool_loan: LendPoolLoan<Provider<Ws>>,
    pub nft_oracle: NFTOracle<Provider<Ws>>,
    pub reserve_oracle: ReserveOracle<Provider<Ws>>,
    pub lend_pool_with_signer: LendPool<SignerMiddleware<Arc<Provider<Ws>>, Wallet<SigningKey>>>,
    pub weth: Weth<SignerMiddleware<Arc<Provider<Ws>>, Wallet<SigningKey>>>,
    pub usdt: Erc20<SignerMiddleware<Arc<Provider<Ws>>, Wallet<SigningKey>>>,
}

impl GlobalProvider {
    pub async fn try_new(config_vars: ConfigVars) -> Result<GlobalProvider> {
        let provider = Provider::<Ws>::connect(&config_vars.wss_rpc_url).await?;
        let provider = Arc::new(provider);

        info!("connected to provider at: {}", config_vars.wss_rpc_url);
        info!(
            "current block number: {}",
            provider.get_block_number().await?
        );

        let local_wallet = MnemonicBuilder::<English>::default()
            .phrase(config_vars.mnemonic.as_str())
            .build()?;

        let signer_provider = SignerMiddleware::new(provider.clone(), local_wallet.clone());
        let signer_provider = Arc::new(signer_provider);

        let address = LEND_POOL.parse::<Address>()?;
        let lend_pool = LendPool::new(address, provider.clone());
        let lend_pool_with_signer = LendPool::new(address, signer_provider.clone());

        let address = LEND_POOL_LOAN.parse::<Address>()?;
        let lend_pool_loan = LendPoolLoan::new(address, provider.clone());

        let address = NFT_ORACLE.parse::<Address>()?;
        let nft_oracle = NFTOracle::new(address, provider.clone());

        let address = RESERVE_ORACLE.parse::<Address>()?;
        let reserve_oracle = ReserveOracle::new(address, provider.clone());

        let address = WETH.parse::<Address>()?;
        let weth = Weth::new(address, signer_provider.clone());

        let address = USDT.parse::<Address>()?;
        let usdt = Erc20::new(address, signer_provider.clone());

        Ok(GlobalProvider {
            local_wallet,
            provider,
            signer_provider,
            lend_pool,
            lend_pool_loan,
            nft_oracle,
            reserve_oracle,
            lend_pool_with_signer,
            weth,
            usdt,
        })
    }

    pub async fn get_loans_from_iter(&self, range: impl Iterator<Item = u64>) -> Result<Vec<Loan>> {
        let mut handles = Vec::new();
        let mut loans: Vec<Loan> = Vec::new();

        for loan_id in range {
            let loan_id = U256::from_little_endian(&loan_id.to_le_bytes());
            let lend_pool = self.lend_pool.clone();
            let lend_pool_loan = self.lend_pool_loan.clone();
            let future: JoinHandle<Result<Option<Loan>>> =
                tokio::spawn(
                    async move { get_loan_data(loan_id, lend_pool, lend_pool_loan).await },
                );
            handles.push(future);
        }

        let result = join_all(handles).await;

        for res in result {
            let loan = res??;
            if let Some(loan) = loan {
                loans.push(loan)
            }
        }

        Ok(loans)
    }

    pub async fn get_updated_loan(&self, loan_id: U256) -> Result<Option<Loan>> {
        get_loan_data(loan_id, self.lend_pool.clone(), self.lend_pool_loan.clone()).await
    }

    pub async fn wrap_eth(&self, amount: U256) -> Result<()> {
        let address = self.local_wallet.address();
        let wallet_balance: U256 = self.weth.balance_of(address).await?;

        if wallet_balance < amount {
            bail!("not enough balance to wrap eth")
        }

        self.weth
            .deposit()
            .value(amount)
            .send()
            .await?
            .log_msg(format!("wrapping {} eth", amount))
            .await?;

        Ok(())
    }

    pub async fn weth_approve(&self, address: Address, amount: U256) -> Result<()> {
        self.weth
            .approve(address, amount)
            .send()
            .await?
            .log_msg(format!("approving {} to spend {} weth", address, amount))
            .await?;

        Ok(())
    }

    pub async fn start_auction(&self, loan: &Loan, bid_price: U256) -> Result<()> {
        let nft_asset = loan.nft_asset.to_string().parse::<Address>()?;

        self.lend_pool_with_signer
            .auction(
                nft_asset,
                loan.nft_token_id,
                bid_price,
                self.local_wallet.address(),
            )
            .send()
            .await?
            .log_msg(format!(
                "starting auction for nft collection: {} for {} weth",
                loan.nft_asset, bid_price
            ))
            .await?;

        Ok(())
    }

    pub async fn liquidate_loan(&self, loan: &Loan) -> Result<()> {
        let nft_asset = loan.nft_asset.to_string().parse::<Address>()?;

        self.lend_pool_with_signer
            .liquidate(nft_asset, loan.nft_token_id, U256::zero())
            .send()
            .await?
            .log_msg(format!(
                "executing liquidation for {} r##{}",
                loan.nft_asset, loan.nft_token_id
            ))
            .await?;

        Ok(())
    }

    pub async fn has_auction_ended(&self, nft_asset: NftAsset, token_id: U256) -> Result<bool> {
        let latest_block = self.provider.get_block_number().await?;
        let timestamp = self
            .provider
            .get_block(latest_block)
            .await?
            .expect("block should be there")
            .timestamp;

        let (_loan_id, _bid_start_timestamp, bid_end_timestamp, _redeem_end_timestamp) = self
            .lend_pool
            .get_nft_auction_end_time(nft_asset.try_into()?, token_id)
            .await?;

        if timestamp >= bid_end_timestamp {
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
