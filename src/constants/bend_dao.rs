use hex_literal::hex;

/// storage slot where `twapPriceMap` resides in `NftOracle`
pub const TWAP_PRICE_MAP_SLOT: [u8; 32] =
    hex!("00000000000000000000000000000000000000000000000000000000000000a0");

pub const BEND_INCEPTION_BLOCK: u64 = 14_417_009;

/// `1.05e18`
pub const HEALTH_FACTOR_THRESHOLD_TO_MONITOR: &str = "0xe92596fd6290000";
