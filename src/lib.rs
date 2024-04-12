pub mod constants;

use ethers::contract::abigen;

abigen!(
    LendingPool,
    "[event Auction(address user, address indexed reserve, uint256 bidPrice, address indexed nftAsset, uint256 nftTokenId, address onBehalfOf, address indexed borrower, uint256 loanId)]",
);
