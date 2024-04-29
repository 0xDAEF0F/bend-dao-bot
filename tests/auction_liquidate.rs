#![cfg(test)]

use anyhow::Result;
use bend_dao_collector::{
    chain_provider::get_loan_data,
    constants::{
        addresses::{CLONEX, WETH},
        bend_dao::{LEND_POOL, LEND_POOL_LOAN},
    },
    Erc721, LendPool, LendPoolLoan, Weth,
};
use ethers::{
    middleware::SignerMiddleware,
    providers::{Http, Provider},
    signers::{LocalWallet, Signer},
    types::{Address, U256},
    utils::Anvil,
};
use std::sync::Arc;

#[tokio::test]
async fn test_auction_and_liquidate() -> Result<()> {
    let url = dotenv::var("MAINNET_RPC_URL")?;
    let fork_block_number: u64 = 19_755_076;
    let anvil = Anvil::new()
        .fork(url)
        .fork_block_number(fork_block_number)
        .spawn();

    let provider = Provider::<Http>::try_from(anvil.endpoint())?;
    let provider = Arc::new(provider);

    let wallet: LocalWallet = anvil.keys()[0].clone().into();
    let wallet_address = wallet.address();

    let client = SignerMiddleware::new(provider.clone(), wallet.with_chain_id(anvil.chain_id()));
    let client = Arc::new(client);

    let weth = Weth::new(WETH.parse::<Address>()?, client.clone());

    weth.deposit().value(U256::exp10(18)).send().await?.await?;
    weth.approve(LEND_POOL.parse::<Address>()?, U256::MAX)
        .send()
        .await?
        .await?;

    let balance_t0 = weth.balance_of(wallet_address).await?;
    assert_eq!(balance_t0, U256::exp10(18));

    let lend_pool = LEND_POOL.parse::<Address>()?;
    let lend_pool = LendPool::new(lend_pool, provider.clone());

    let lend_pool_loan = LEND_POOL_LOAN.parse::<Address>()?;
    let lend_pool_loan = LendPoolLoan::new(lend_pool_loan, provider.clone());

    let loan = get_loan_data(5138.into(), lend_pool.clone(), lend_pool_loan.clone())
        .await?
        .expect("loan should be there");

    let lend_pool = LendPool::new(LEND_POOL.parse::<Address>()?, client.clone());

    let nft_asset = format!("{}", loan.nft_asset).parse::<Address>()?;
    let _receipt = lend_pool
        .auction(
            nft_asset,
            loan.nft_token_id,
            U256::exp10(18),
            wallet_address,
        )
        .send()
        .await?
        .log()
        .await?;

    let balance_t1 = weth.balance_of(wallet_address).await?;
    assert_eq!(balance_t1, U256::zero());

    increase_time(provider.clone(), 86_400).await?;

    let _receipt = lend_pool
        .liquidate(nft_asset, loan.nft_token_id, U256::zero())
        .send()
        .await?
        .log()
        .await?;

    let clonex: Address = CLONEX.parse()?;
    let clonex = Erc721::new(clonex, provider.clone());

    let owner: Address = clonex.owner_of(U256::from(18241)).await?;

    assert_eq!(owner, wallet_address);

    Ok(())
}

// works in a testing environment
async fn increase_time(provider: Arc<Provider<Http>>, time: u64) -> Result<()> {
    provider
        .request::<_, i64>("evm_increaseTime", vec![time])
        .await?;

    provider
        .request::<Vec<()>, String>("evm_mine", vec![])
        .await?;

    Ok(())
}
