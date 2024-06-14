use crate::{
    benddao::loan::{Loan, NftAsset},
    constants::*,
    types::*,
    utils::{get_loan_data, handle_sent_bundle},
    Config, Erc20, LendPool, LendPoolLoan, Weth,
};
use anyhow::{bail, Result};
use ethers::{
    core::{k256::ecdsa::SigningKey, rand::thread_rng},
    middleware::SignerMiddleware,
    providers::{Middleware, Provider, Ws},
    signers::{coins_bip39::English, LocalWallet, MnemonicBuilder, Signer, Wallet},
    types::{spoof::State, Address, Transaction, U256},
};
use ethers_flashbots::{BroadcasterMiddleware, BundleRequest};
use futures::future::join_all;
use log::{debug, info};
use std::sync::Arc;
use tokio::{task::JoinHandle, try_join};
use url::Url;

static BUILDER_URLS: &[&str] = &[
    "https://builder0x69.io",
    "https://rpc.beaverbuild.org",
    "https://relay.flashbots.net",
    "https://rsync-builder.xyz",
    "https://rpc.titanbuilder.xyz",
    "https://api.blocknative.com/v1/auction",
    "https://mev.api.blxrbdn.com",
    "https://eth-builder.com",
    "https://builder.gmbit.co/rpc",
    "https://buildai.net",
    "https://rpc.payload.de",
    "https://rpc.lightspeedbuilder.info",
    "https://rpc.nfactorial.xyz",
    "https://rpc.lokibuilder.xyz",
];

#[derive(Clone)]
pub struct GlobalProvider {
    pub local_wallet: LocalWallet,
    pub provider: Arc<Provider<Ws>>,
    pub signer_provider: Arc<
        SignerMiddleware<
            BroadcasterMiddleware<Arc<Provider<Ws>>, Wallet<SigningKey>>,
            Wallet<SigningKey>,
        >,
    >,
    pub lend_pool: LendPool<Provider<Ws>>,
    pub lend_pool_loan: LendPoolLoan<Provider<Ws>>,
    pub weth: Weth<Provider<Ws>>,
    pub usdt: Erc20<Provider<Ws>>,
}

impl GlobalProvider {
    pub async fn try_new(config_vars: Config) -> Result<GlobalProvider> {
        let provider = Provider::<Ws>::connect(&config_vars.mainnet_rpc_url_ws).await?;
        let provider = Arc::new(provider);

        info!(
            "connected to provider at: {}",
            config_vars.mainnet_rpc_url_ws
        );
        info!(
            "current block number: {}",
            provider.get_block_number().await?
        );

        let local_wallet = MnemonicBuilder::<English>::default()
            .phrase(config_vars.mnemonic.as_str())
            .build()?;

        let signer_provider = SignerMiddleware::new(
            BroadcasterMiddleware::new(
                provider.clone(),
                BUILDER_URLS
                    .iter()
                    .map(|url| Url::parse(url).unwrap())
                    .collect(),
                Url::parse("https://relay.flashbots.net")?,
                // TODO
                // replace with a specific bundle signer
                LocalWallet::new(&mut thread_rng()),
            ),
            local_wallet.clone(),
        );

        let signer_provider = Arc::new(signer_provider);

        let lend_pool = LendPool::new(Address::from(LEND_POOL), provider.clone());

        let address = Address::from(LEND_POOL_LOAN);
        let lend_pool_loan = LendPoolLoan::new(address, provider.clone());

        let address = Address::from(WETH);
        let weth = Weth::new(address, provider.clone());

        let address = Address::from(USDT);
        let usdt = Erc20::new(address, provider.clone());

        Ok(GlobalProvider {
            local_wallet,
            provider,
            signer_provider,
            lend_pool,
            lend_pool_loan,
            weth,
            usdt,
        })
    }

    pub async fn get_loans_from_iter(
        &self,
        range: impl Iterator<Item = u64>,
        state: Option<State>,
    ) -> Result<Vec<Loan>> {
        let mut handles = Vec::new();
        let mut loans: Vec<Loan> = Vec::new();

        for loan_id in range {
            let loan_id = U256::from_little_endian(&loan_id.to_le_bytes());
            let lend_pool = self.lend_pool.clone();
            let lend_pool_loan = self.lend_pool_loan.clone();
            let state = state.clone();
            let provider = self.provider.clone();
            let future: JoinHandle<Result<Option<Loan>>> = tokio::spawn(async move {
                get_loan_data(loan_id, provider, lend_pool, lend_pool_loan, state).await
            });
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
        get_loan_data(
            loan_id,
            self.provider.clone(),
            self.lend_pool.clone(),
            self.lend_pool_loan.clone(),
            None,
        )
        .await
    }

    pub async fn get_balances(&self) -> Result<Balances> {
        let local_wallet_address = self.local_wallet.address();
        let lend_pool_address = Address::from(LEND_POOL);

        let (eth, weth, usdt, usdt_approval_amount, weth_approval_amount) = try_join!(
            self.get_eth_balance(&local_wallet_address),
            self.get_weth_balance(&local_wallet_address),
            self.get_usdt_balance(&local_wallet_address),
            self.get_weth_lend_pool_approval(&local_wallet_address, &lend_pool_address),
            self.get_usdt_lend_pool_approval(&local_wallet_address, &lend_pool_address)
        )?;

        let balances = Balances {
            eth,
            weth,
            usdt,
            is_usdt_lend_pool_approved: usdt_approval_amount == U256::MAX,
            is_weth_lend_pool_approved: weth_approval_amount == U256::MAX,
        };

        debug!("{:?}", balances);

        Ok(balances)
    }

    pub async fn start_auctions(
        &self,
        loans: Vec<AuctionBid>,
        oracle_update_tx: Transaction,
    ) -> Result<BundleRequest> {
        // add oracle update
        let bundle = BundleRequest::new().push_transaction(oracle_update_tx);
        // add auction txs
        self.create_auction_bundle(bundle, loans, false).await
    }

    /// creates a vec of tx's for auction based off loans
    pub async fn create_auction_bundle(
        &self,
        mut bundle: BundleRequest,
        loans: Vec<AuctionBid>,
        // temp
        // TODO decide what to gas
        max_gas: bool,
    ) -> Result<BundleRequest> {
        for loan in loans {
            let nft_asset: Address = loan.nft_asset;

            let mut tx = self
                .lend_pool
                .auction(
                    nft_asset,
                    loan.nft_token_id,
                    loan.bid_price,
                    self.local_wallet.address(),
                )
                .tx;

            if max_gas {
                tx.set_gas_price(10);
            }

            let signature = self.local_wallet.sign_transaction(&tx).await?;

            bundle.add_transaction(tx.rlp_signed(&signature));
        }

        Ok(bundle)
    }

    pub async fn send_and_handle_bundle(&self, bundle: BundleRequest) -> Result<()> {
        let pending_bundle = self.signer_provider.inner().send_bundle(&bundle).await?;

        handle_sent_bundle(pending_bundle).await
    }

    // // simulates bundle and returns effective gas price
    // pub async fn simulate_bundle(&self, bundle: &BundleRequest) -> Result<U256> {
    //     let results = self.signer_provider.inner().simulate_bundle(bundle).await?;

    //     Ok(results.effective_gas_price())
    // }

    pub async fn liquidate_loan(&self, auction: &Auction) -> Result<()> {
        let tx = self
            .lend_pool
            .liquidate(auction.nft_asset, auction.nft_token_id, U256::zero())
            .tx;

        let reciept = self
            .signer_provider
            .send_transaction(tx, None)
            .await?
            .log_msg(format!(
                "executing liquidation for {:?} r##{}",
                auction.nft_asset, auction.nft_token_id
            ))
            .await?;

        if let Some(reciept) = reciept {
            info!(
                "loan successfully liquidated here: https://etherscan.io/tx/{:?}",
                reciept.transaction_hash
            );
        } else {
            bail!("auction failed")
        }

        Ok(())
    }

    pub async fn get_auction_end_timestamp(&self, nft_asset: Address, token_id: U256) -> U256 {
        let (_loan_id, _bid_start_timestamp, bid_end_timestamp, _redeem_end_timestamp) = self
            .lend_pool
            .get_nft_auction_end_time(nft_asset, token_id)
            .await
            .unwrap();
        bid_end_timestamp
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
            .get_nft_auction_end_time(nft_asset.into(), token_id)
            .await?;

        if timestamp >= bid_end_timestamp {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn get_eth_balance(&self, addr: &Address) -> Result<U256> {
        Ok(self.provider.get_balance(*addr, None).await?)
    }

    async fn get_weth_balance(&self, addr: &Address) -> Result<U256> {
        Ok(self.weth.balance_of(*addr).await?)
    }

    async fn get_usdt_balance(&self, addr: &Address) -> Result<U256> {
        Ok(self.usdt.balance_of(*addr).await?)
    }

    async fn get_weth_lend_pool_approval(
        &self,
        addr: &Address,
        lend_pool: &Address,
    ) -> Result<U256> {
        Ok(self.weth.allowance(*addr, *lend_pool).await?)
    }

    async fn get_usdt_lend_pool_approval(
        &self,
        addr: &Address,
        lend_pool: &Address,
    ) -> Result<U256> {
        Ok(self.usdt.allowance(*addr, *lend_pool).await?)
    }
}
