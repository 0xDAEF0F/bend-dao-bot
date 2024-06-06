use alloy_primitives::Address as AlloyAddress;
use anyhow::Result;
use futures::StreamExt;
use hex_literal::hex;
use mev_share_sse::EventClient;

const FLASHBOTS_SSE: &str = "https://mev-share.flashbots.net";

const CONTRACT_OF_INTEREST: [u8; 20] = hex!("0000000000000000000000000000000000000000");

// SetAssetTwapPrice(address,uint256,uint256)
const TOPIC_0: [u8; 32] = hex!("58bdf68b6e757afad014720959e6c9ecd94de1cc24b964ebf48b08b50366b321");

#[tokio::main]
async fn main() -> Result<()> {
    let client = EventClient::default();

    let mut stream = client.events(FLASHBOTS_SSE).await.unwrap();

    println!("Subscribed to {}", stream.endpoint());

    while let Some(event) = stream.next().await {
        if let Ok(evt) = event {
            for log in evt.logs.iter() {
                if log.address != AlloyAddress::from(CONTRACT_OF_INTEREST) {
                    continue;
                }
                if !log.topics.first().is_some_and(|topic0| topic0.0 == TOPIC_0) {
                    continue;
                }
                let _first_indexed = &log.topics[1].0;
                let _log_data = &log.data[..];
            }
        }
    }

    Ok(())
}
