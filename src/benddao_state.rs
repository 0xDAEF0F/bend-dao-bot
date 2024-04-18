#![allow(unused)]

use crate::{constants::LEND_POOL_LOAN, LendPoolLoan, ReserveDataUpdatedFilter};
use anyhow::Result;
use ethers::{
    contract::LogMeta,
    providers::{Http, Provider},
    types::{Address, U256, U64},
};
use futures::future::join_all;
use std::{collections::HashMap, sync::Arc};
use tokio::{join, spawn, task::JoinHandle};

#[derive(Debug)]
pub struct BendDao {
    pub reserve_data: HashMap<Address, ReserveData>,
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
    pub fn build(url: &str) -> Result<Self> {
        let provider = Provider::<Http>::try_from(url)?;

        Ok(BendDao {
            reserve_data: HashMap::new(),
            last_mined_block: U64::zero(),
            loans: Vec::new(),
            provider: Arc::new(provider),
        })
    }

    pub async fn build_all_loans(&mut self) -> Result<()> {
        let lend_pool_loan =
            LendPoolLoan::new(LEND_POOL_LOAN.parse::<Address>()?, self.provider.clone());

        let last_loan_id: U256 = lend_pool_loan.get_current_loan_id().await?;

        let mut handles = Vec::new();

        for loan_id in 1..last_loan_id.as_u64() {
            let lend_pool_loan = lend_pool_loan.clone();
            let future = spawn(async move {
                let loan = lend_pool_loan.get_loan(loan_id.into()).await;
                match loan {
                    Ok(loan) => println!("Fetched loan: {:?}", loan),
                    Err(e) => println!("Error fetching loan: {:?}", e),
                }
            });

            handles.push(future);
        }

        let results = join_all(handles).await;
        for result in results {
            match result {
                Ok(loan) => println!("Successfully fetched loan: {:?}", loan),
                Err(e) => println!("Task failed: {:?}", e),
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
            let _ = self.reserve_data.insert(reserve_data.reserve, rd);
        }
    }

    pub fn update_block(&mut self, meta: LogMeta) {
        self.last_mined_block = meta.block_number;
    }
}
