pub mod constants;

use ethers::contract::abigen;

abigen!(LendingPool, "abi/LendPool.json");
