use crate::{constants::LEND_POOL_LOAN, LendPoolLoan, LoanData, ReserveDataUpdatedFilter};
use anyhow::{Error, Result};
use ethers::{
    contract::LogMeta,
    providers::{Http, Provider},
    types::{Address, U256, U64},
};
use futures::future::join_all;
use std::{collections::HashMap, sync::Arc};
use tokio::{spawn, task::JoinHandle};

#[derive(Debug)]
pub struct BendDao {
    pub reserve_data: HashMap<Address, ReserveData>,
    pub latest_floor_price: HashMap<Address, U256>,
    pub last_mined_block: U64,
    pub loans: Vec<Loan>,
    provider: Arc<Provider<Http>>,
}

#[derive(Debug, Clone)]
pub struct ReserveData {
    pub borrow_index: U256,
    pub interest_rate: U256,
}

#[derive(Debug)]
pub struct Loan {
    pub loan_id: U256,
    pub reserve_asset: Address,
    pub nft_collection: Address,
    pub nft_token_id: U256,
    pub scaled_debt: U256,
}

impl BendDao {
    pub fn try_new(url: &str) -> Result<Self> {
        let provider = Provider::<Http>::try_from(url)?;

        Ok(BendDao {
            reserve_data: HashMap::new(),
            latest_floor_price: HashMap::new(),
            last_mined_block: U64::zero(),
            loans: Vec::new(),
            provider: Arc::new(provider),
        })
    }

    pub async fn build_all_loans(&mut self) -> Result<()> {
        let lend_pool_loan =
            LendPoolLoan::new(LEND_POOL_LOAN.parse::<Address>()?, self.provider.clone());

        let last_loan_id: u64 = lend_pool_loan.get_current_loan_id().await?.as_u64();
        let start_loan_id: u64 = dotenv::var("ENVIRONMENT")
            .map(|env| {
                if env.to_lowercase() == "production" {
                    1
                } else {
                    last_loan_id - 10
                }
            })
            .unwrap_or(last_loan_id - 10);

        let mut handles = Vec::new();

        for loan_id in start_loan_id..last_loan_id {
            let lend_pool_loan = lend_pool_loan.clone();
            let future: JoinHandle<Result<LoanData, Error>> = spawn(async move {
                let loan: LoanData = lend_pool_loan.get_loan(loan_id.into()).await?;
                Ok(loan)
            });
            handles.push(future);
        }

        for loan in join_all(handles).await {
            let loan = loan??;
            // is active
            if loan.state == 2 {
                self.loans.push(Loan {
                    loan_id: loan.loan_id,
                    reserve_asset: loan.reserve_asset,
                    nft_collection: loan.nft_asset,
                    nft_token_id: loan.nft_token_id,
                    scaled_debt: loan.scaled_amount,
                });
            }
        }

        Ok(())
    }

    pub fn update_reserve_data(&mut self, reserve_data: ReserveDataUpdatedFilter) {
        if let Some(rd) = self.reserve_data.get_mut(&reserve_data.reserve) {
            rd.interest_rate = reserve_data.variable_borrow_rate;
            rd.borrow_index = reserve_data.variable_borrow_index;
        } else {
            let rd = ReserveData {
                interest_rate: reserve_data.variable_borrow_rate,
                borrow_index: reserve_data.variable_borrow_index,
            };
            self.reserve_data.insert(reserve_data.reserve, rd);
        }
    }

    pub fn update_block(&mut self, meta: LogMeta) {
        self.last_mined_block = meta.block_number;
    }
}
