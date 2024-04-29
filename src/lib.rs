pub mod benddao;
pub mod chain_provider;
pub mod coinmarketcap;
pub mod constants;
pub mod math;
pub mod prices_client;
pub mod reservoir;

use ethers::contract::abigen;

abigen!(LendPool, "abi/LendPool.json");
abigen!(LendPoolLoan, "abi/LendPoolLoan.json");
abigen!(NFTOracle, "abi/NFTOracle.json");
abigen!(ReserveOracle, "abi/ReserveOracle.json");
abigen!(Weth, "abi/Weth.json");
abigen!(Erc721, "abi/ERC721.json");
