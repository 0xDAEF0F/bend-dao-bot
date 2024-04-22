pub mod benddao_state;
pub mod constants;
pub mod data_source;
pub mod math;
pub mod prices_client;

use ethers::contract::abigen;

abigen!(LendPool, "abi/LendPool.json");
abigen!(LendPoolLoan, "abi/LendPoolLoan.json");
abigen!(NFTOracle, "abi/NFTOracle.json");
abigen!(ReserveOracle, "abi/ReserveOracle.json");
