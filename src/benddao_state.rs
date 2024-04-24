use crate::{
    constants::bend_dao::{BAYC_ADDRESS, HEALTH_FACTOR_THRESHOLD_TO_MONITOR, WRAPPED_CRYPTOPUNKS},
    data_source::DataSource,
    prices_client::PricesClient,
};
use anyhow::Result;
use ethers::types::U256;
use log::{debug, info};
use std::collections::{HashMap, HashSet};

#[derive(Debug, PartialEq)]
pub enum Status {
    Created, // not sure about this state
    Active,
    Auction,
}

#[derive(Debug, PartialEq)]
pub enum ReserveAsset {
    Weth,
    Usdt,
}

#[derive(Debug, PartialEq)]
pub enum NftAsset {
    Bayc,
    CryptoPunks,
}

impl ToString for NftAsset {
    fn to_string(&self) -> String {
        match self {
            NftAsset::Bayc => BAYC_ADDRESS.to_string(),
            NftAsset::CryptoPunks => WRAPPED_CRYPTOPUNKS.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct BendDao {
    loans: HashMap<U256, Loan>,
    monitored_loans: HashSet<U256>,
    data_source: DataSource,
    prices_client: PricesClient,
}

#[derive(Debug)]
pub struct Loan {
    pub loan_id: U256,
    pub status: Status,
    pub nft_token_id: U256,
    pub health_factor: U256,
    pub total_debt: U256,
    pub reserve_asset: ReserveAsset,
    pub nft_asset: NftAsset,
}

impl BendDao {
    pub fn try_new() -> Result<BendDao> {
        let url = dotenv::var("MAINNET_RPC_URL")?;
        Ok(BendDao {
            monitored_loans: HashSet::new(),
            loans: HashMap::new(),
            data_source: DataSource::try_new(&url)?,
            prices_client: PricesClient::default(),
        })
    }

    pub async fn handle_repay_loan(&mut self, loan_id: U256) -> Result<()> {
        match self.data_source.get_updated_loan(loan_id).await? {
            Some(loan) => {
                // still active (partial repay)
                self.handle_monitoring(&loan);
                self.loans.insert(loan_id, loan);
            }
            None => {
                // fully repaid
                self.loans.remove(&loan_id);
                self.monitored_loans.remove(&loan_id);
            }
        }
        Ok(())
    }

    pub async fn handle_auction(&mut self, loan_id: U256) -> Result<()> {
        let loan = self
            .data_source
            .get_updated_loan(loan_id)
            .await?
            .expect("loan should exist");

        self.handle_monitoring(&loan);

        self.loans.insert(loan_id, loan);

        Ok(())
    }

    pub async fn handle_borrow(&mut self, loan_id: U256) -> Result<()> {
        let loan = self
            .data_source
            .get_updated_loan(loan_id)
            .await?
            .expect("loan should exist");

        self.handle_monitoring(&loan);

        self.loans.insert(loan_id, loan);

        Ok(())
    }

    pub async fn handle_redeem(&mut self, loan_id: U256) -> Result<()> {
        let loan = self
            .data_source
            .get_updated_loan(loan_id)
            .await?
            .expect("loan should still be active");

        self.handle_monitoring(&loan);
        self.loans.insert(loan_id, loan);

        Ok(())
    }

    // take it off the system
    pub fn handle_liquidation(&mut self, loan_id: U256) {
        self.loans.remove(&loan_id);
        self.monitored_loans.remove(&loan_id);
    }

    fn handle_monitoring(&mut self, loan: &Loan) {
        // nothing to do here just take it off the monitoring list and return
        if loan.status == Status::Auction {
            self.monitored_loans.remove(&loan.loan_id);
            return;
        }

        if loan.health_factor < U256::from_dec_str(HEALTH_FACTOR_THRESHOLD_TO_MONITOR).unwrap() {
            self.monitored_loans.insert(loan.loan_id);
        } else {
            self.monitored_loans.remove(&loan.loan_id);
        }
    }

    pub async fn handle_new_block(&mut self) -> Result<()> {
        info!("iterating on monitored_loans");
        for loan_id in self.monitored_loans.iter() {
            info!("checking loan {} status", loan_id.as_u64());

            let updated_loan = self.data_source.get_updated_loan(*loan_id).await?.unwrap();

            if updated_loan.health_factor >= U256::exp10(18) {
                info!("loan_id: {} above 1.0", loan_id.as_u64());
                continue;
            }

            info!("loan {} auctionable", loan_id.as_u64());
            // determine the profitability

            let nft_price_eth = self
                .prices_client
                .get_best_nft_bid(updated_loan.nft_asset)
                .await?;

            let total_debt_eth = match updated_loan.reserve_asset {
                ReserveAsset::Usdt => {
                    let usdt_eth_price = self.prices_client.get_usdt_eth_price().await?;
                    updated_loan.total_debt * U256::exp10(18) / usdt_eth_price
                }
                ReserveAsset::Weth => updated_loan.total_debt,
            };

            if nft_price_eth < total_debt_eth {
                info!("price of nft less than total debt");
            } else {
                let profit = nft_price_eth - total_debt_eth;
                info!("potential profit of: {}", profit);
            }
        }
        Ok(())
    }

    pub async fn build_all_loans(&mut self) -> Result<()> {
        // this loan has not yet existed so not inclusive range
        let last_loan_id: u64 = self
            .data_source
            .lend_pool_loan
            .get_current_loan_id()
            .await?
            .as_u64();

        let start_loan_id: u64 = dotenv::var("ENVIRONMENT")
            .map(|env| {
                if env.to_lowercase() == "production" {
                    1
                } else {
                    last_loan_id - 2
                }
            })
            .unwrap_or(last_loan_id - 2);

        let all_loans = self
            .data_source
            .get_loans_range(start_loan_id..last_loan_id)
            .await?;

        for loan in all_loans {
            self.loans.insert(loan.loan_id, loan);
        }

        info!("all loans have been built");
        debug!("{:#?}", &self.loans);

        Ok(())
    }
}
