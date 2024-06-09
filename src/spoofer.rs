use crate::constants::*;
use ethers::{
    types::{spoof::State, Address, H256, U256},
    utils::keccak256,
};

pub fn get_new_state_with_twaps_modded(twaps: Vec<(Address, U256)>) -> State {
    let mut state = State::default();

    for (nft_address, twap) in twaps {
        let storage_slot = get_storage_slot(nft_address);
        state
            .account(Address::from(NFT_ORACLE))
            .store(storage_slot, u256_to_h256_be(twap));
    }

    state
}

fn get_storage_slot(nft_address: Address) -> H256 {
    let nft_address: H256 = nft_address.into();

    let slot = keccak256([nft_address.into(), TWAP_PRICE_MAP_SLOT].concat());

    H256::from_slice(&slot[..])
}

fn u256_to_h256_be(u: U256) -> H256 {
    let mut h = H256::default();
    u.to_big_endian(h.as_mut());
    h
}

#[cfg(test)]
mod test {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn convert_u256_to_h256() {
        let num = U256::from_dec_str("42").unwrap();
        let h = u256_to_h256_be(num);
        assert_eq!(
            h.0,
            hex!("000000000000000000000000000000000000000000000000000000000000002a")
        )
    }

    #[test]
    fn test_slot() {
        let stbayc = STBAYC.into();
        let stbayc_slot = get_storage_slot(stbayc);
        assert_eq!(
            stbayc_slot.0,
            hex!("d1e9bfb3b88592ebcc2c0a884947056bf92c8c954d6dbdb9e97e5f196d054c38") // bayc
        );
    }
}
