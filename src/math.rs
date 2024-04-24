use crate::constants::math::{RAY, SECONDS_PER_YEAR};
use ethers::types::U256;

/**
 * @dev Multiplies two ray, rounding half up to the nearest ray
 * @param a Ray
 * @param b Ray
 * @return The result of a*b, in ray
 **/
pub fn ray_mul(a: U256, b: U256) -> Option<U256> {
    if a == U256::zero() || b == U256::zero() {
        return Some(U256::zero());
    }

    let ray = U256::from_dec_str(RAY).unwrap();
    let half_ray = ray / 2;

    if a > (U256::MAX - half_ray) / b {
        return None;
    }

    Some((a * b + half_ray) / ray)
}

/**
 * @dev Function to calculate the interest using a compounded interest rate formula
 * To avoid expensive exponentiation, the calculation is performed using a binomial approximation:
 *
 *  (1+x)^n = 1+n*x+[n/2*(n-1)]*x^2+[n/6*(n-1)*(n-2)*x^3...
 *
 * The approximation slightly underpays liquidity providers and undercharges borrowers, with the advantage of great gas cost reductions
 * The whitepaper contains reference to the approximation and a table showing the margin of error per different time periods
 *
 * @param rate The interest rate, in ray
 * @param lastUpdateTimestamp The timestamp of the last update of the interest
 * @return The interest rate compounded during the timeDelta, in ray
 **/
pub fn calculate_compounded_interest(
    rate: U256,
    prev_timestamp: U256,
    current_timestamp: U256,
) -> U256 {
    let exp = current_timestamp - prev_timestamp;

    if exp == U256::zero() {
        return U256::exp10(27);
    }

    let exp_minus_one = exp - 1;

    let exp_minus_two = if exp > U256::from(2) {
        exp - 2
    } else {
        U256::zero()
    };

    let rate_per_second = rate / U256::from(SECONDS_PER_YEAR);

    let base_power_two = ray_mul(rate_per_second, rate_per_second).unwrap();
    let base_power_three = ray_mul(base_power_two, rate_per_second).unwrap();

    let second_term = (exp * (exp_minus_one) * (base_power_two)) / 2;
    let third_term = (exp * (exp_minus_one) * (exp_minus_two) * (base_power_three)) / 6;

    U256::exp10(27) + (rate_per_second * (exp)) + (second_term) + (third_term)
}
