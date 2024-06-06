use alloy_primitives::Address as AlloyAddress;
use anyhow::Result;
use bend_dao_collector::benddao::loan::NftAsset;
use ethers::{
    types::{Address, U256},
    utils::format_ether,
};
use futures::StreamExt;
use hex_literal::hex;
use mev_share_sse::EventClient;

const NFT_ORACLE: [u8; 20] = hex!("7c2a19e54e48718f6c60908a9cff3396e4ea1eba");
const EXAMPLE_CONTRACT: [u8; 20] = hex!("684E7E397E0916806ed0F057aD7414eE87b0E349");

// SetAssetTwapPrice(address,uint256,uint256)
const TOPIC_0: [u8; 32] = hex!("58bdf68b6e757afad014720959e6c9ecd94de1cc24b964ebf48b08b50366b321");

#[tokio::main]
async fn main() -> Result<()> {
    let mainnet_sse = "https://mev-share.flashbots.net";
    let sepolia_sse = "https://mev-share-sepolia.flashbots.net";
    let sepolia_ws = "wss://eth-sepolia.g.alchemy.com/v2/ksiz54CMTBZAy1sL-04FvwZhpN7kw3A0";

    let client = EventClient::default();
    let mut stream = client.events(sepolia_sse).await.unwrap();

    println!("Subscribed to {}", stream.endpoint());

    while let Some(event) = stream.next().await {
        if let Ok(evt) = event {
            for log in evt.logs.iter() {
                if log.address == AlloyAddress::from(EXAMPLE_CONTRACT) {
                    println!("hello");
                }
                if log.address != AlloyAddress::from(EXAMPLE_CONTRACT) {
                    continue;
                }
                println!("nft oracle emitted logs");
                if !log.topics.first().is_some_and(|topic0| topic0.0 == TOPIC_0) {
                    continue;
                }
                let nft_address = &log.topics[1].0;
                let nft_address = Address::from_slice(&nft_address[12..]);
                println!("nft_address: {}", nft_address);
                let price = &log.data[..32];
                let price = U256::from_big_endian(&price);
                println!("price was {} ETH", format_ether(price));
                // if let Ok(nft_asset) = NftAsset::try_from(nft_address) {
                //     println!("oracle posted changes to: {:?}", nft_asset);
                //     let price = &log.data[..32];
                //     let price = U256::from_big_endian(&price);
                //     println!("price was {} ETH", format_ether(price));
                // };
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a() {
        let nft_oracle = AlloyAddress::from(NFT_ORACLE);
        println!("{}", nft_oracle);
    }
}
