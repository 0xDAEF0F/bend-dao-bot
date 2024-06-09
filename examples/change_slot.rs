use anyhow::Result;
use bend_dao_collector::{constants::*, LendPool};
use ethers::{
    providers::{Http, Middleware, Provider, RawCall},
    types::*,
    utils::format_ether,
};
use hex_literal::hex;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let rpc_url = std::env::var("MAINNET_RPC_URL")?;
    let provider = Arc::new(Provider::<Http>::try_from(rpc_url)?);

    println!("block is: {}", provider.get_block_number().await?);

    let lend_pool = LendPool::new(Address::from(LEND_POOL), provider.clone());

    // https://evm.storage/eth/20034348/0x7c2a19e54e48718f6c60908a9cff3396e4ea1eba/twapPriceMap#map
    let cryptopunks_slot = hex!("0eee472b508465d456b2dcf2cdaaf996344f0302de1d5eeb7af8dc653aa71137");
    let cryptopunks_slot = H256::from(cryptopunks_slot);

    // storage value
    let _twap_price = provider
        .get_storage_at(Address::from(NFT_ORACLE), cryptopunks_slot, None)
        .await?;
    let _twap_price = U256::from(_twap_price.0);

    // change state
    let mut state = spoof::State::default();
    state
        .account(Address::from(NFT_ORACLE))
        .store(cryptopunks_slot, H256::zero());

    let call = lend_pool.get_nft_debt_data(CRYPTOPUNKS.into(), U256::from_dec_str("8461")?);

    let (_, _, _, _, _, health_factor) = call.clone().await?;
    println!("health_factor before: {}", format_ether(health_factor));

    let rc = call.call_raw().state(&state).await?;
    let (_, _, _, _, _, health_factor) = rc;

    println!("health_factor_after: {}", format_ether(health_factor));

    Ok(())
}
