#![cfg(test)]

use bend_dao_collector::constants::RAY;
use bend_dao_collector::math::ray_mul;
use ethers::types::U256;

#[tokio::test]
async fn test_ray_mul() {
    let ray = U256::from_dec_str(RAY).unwrap();

    // test multiplying by zero
    assert_eq!(ray_mul(U256::zero(), U256::from(10u64)), Some(U256::zero()));
    assert_eq!(ray_mul(U256::from(10u64), U256::zero()), Some(U256::zero()));

    // test normal multiplication
    let a = U256::from(3u64) * ray;
    let b = U256::from(2u64) * ray;
    assert_eq!(ray_mul(a, b).unwrap(), U256::from(6u64) * ray);

    // test edge case for overflow
    let big_a = U256::MAX;
    let big_b = U256::from(2u64);
    assert!(ray_mul(big_a, big_b).is_none());

    // example to test rounding up
    let a = U256::one();
    let b = (ray * ray) + (ray / 2);
    let res = ray_mul(a, b).unwrap();
    assert_eq!(res, ray + 1);
}
