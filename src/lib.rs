pub mod constants;
pub mod math;

use ethers::contract::abigen;

abigen!(LendPool, "abi/LendPool.json");
abigen!(LendPoolLoan, "abi/LendPoolLoan.json");
