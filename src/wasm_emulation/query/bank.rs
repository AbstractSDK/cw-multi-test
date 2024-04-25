use crate::wasm_emulation::channel::RemoteChannel;
use crate::wasm_emulation::query::gas::{GAS_COST_ALL_BALANCE_QUERY, GAS_COST_BALANCE_QUERY};
use crate::wasm_emulation::query::mock_querier::QueryResultWithGas;
use cosmwasm_std::Addr;
use cosmwasm_vm::GasInfo;
use std::str::FromStr;

use cw_utils::NativeBalance;

use cw_orch::daemon::queriers::Bank;

use cosmwasm_std::Binary;
use cosmwasm_std::Coin;
use std::collections::HashMap;

use cosmwasm_std::Uint128;
use cosmwasm_std::{AllBalanceResponse, BalanceResponse, BankQuery};

use cosmwasm_std::to_json_binary;
use cosmwasm_std::{ContractResult, SystemResult};

#[derive(Clone)]
pub struct BankQuerier {
    #[allow(dead_code)]
    /// HashMap<denom, amount>
    supplies: HashMap<String, Uint128>,
    /// HashMap<address, coins>
    balances: HashMap<String, Vec<Coin>>,
    remote: RemoteChannel,
}

impl BankQuerier {
    pub fn new(remote: RemoteChannel, init: Vec<(Addr, NativeBalance)>) -> Self {
        let balances: HashMap<_, _> = init
            .iter()
            .map(|(s, c)| (s.to_string(), c.clone().into_vec()))
            .collect();

        BankQuerier {
            supplies: Self::calculate_supplies(&balances),
            balances,
            remote,
        }
    }

    pub fn update_balance(
        &mut self,
        addr: impl Into<String>,
        balance: Vec<Coin>,
    ) -> Option<Vec<Coin>> {
        let result = self.balances.insert(addr.into(), balance);
        self.supplies = Self::calculate_supplies(&self.balances);

        result
    }

    fn calculate_supplies(balances: &HashMap<String, Vec<Coin>>) -> HashMap<String, Uint128> {
        let mut supplies = HashMap::new();

        let all_coins = balances.iter().flat_map(|(_, coins)| coins);

        for coin in all_coins {
            *supplies
                .entry(coin.denom.clone())
                .or_insert_with(Uint128::zero) += coin.amount;
        }

        supplies
    }

    pub fn query(&self, request: &BankQuery) -> QueryResultWithGas {
        let contract_result: ContractResult<Binary> = match request {
            BankQuery::Balance { address, denom } => {
                // proper error on not found, serialize result on found
                let mut amount = self
                    .balances
                    .get(address)
                    .and_then(|v| v.iter().find(|c| &c.denom == denom).map(|c| c.amount));

                // If the amount is not available, we query it from the distant chain
                if amount.is_none() {
                    let querier = Bank {
                        channel: self.remote.channel.clone(),
                        rt_handle: Some(self.remote.rt.clone()),
                    };

                    let query_result = self
                        .remote
                        .rt
                        .block_on(querier._balance(address, Some(denom.clone())))
                        .map(|result| Uint128::from_str(&result[0].amount).unwrap());

                    if let Ok(distant_amount) = query_result {
                        amount = Some(distant_amount)
                    }
                }

                let bank_res = BalanceResponse {
                    amount: Coin {
                        amount: amount.unwrap(),
                        denom: denom.to_string(),
                    },
                };
                to_json_binary(&bank_res).into()
            }
            BankQuery::AllBalances { address } => {
                // proper error on not found, serialize result on found
                let mut amount = self.balances.get(address).cloned();

                // We query only if the bank balance doesn't exist
                if amount.is_none() {
                    let querier = Bank {
                        channel: self.remote.channel.clone(),
                        rt_handle: Some(self.remote.rt.clone()),
                    };
                    let query_result: Result<Vec<Coin>, _> = self
                        .remote
                        .rt
                        .block_on(querier._balance(address, None))
                        .map(|result| {
                            result
                                .into_iter()
                                .map(|c| Coin {
                                    amount: Uint128::from_str(&c.amount).unwrap(),
                                    denom: c.denom,
                                })
                                .collect()
                        });
                    if let Ok(distant_amount) = query_result {
                        amount = Some(distant_amount)
                    }
                }

                let bank_res = AllBalanceResponse {
                    amount: amount.unwrap(),
                };
                to_json_binary(&bank_res).into()
            }
            &_ => panic!("Not implemented {:?}", request),
        };

        // We handle the gas_info
        let gas_info = match request {
            BankQuery::Balance { .. } => GAS_COST_BALANCE_QUERY,
            BankQuery::AllBalances { .. } => GAS_COST_ALL_BALANCE_QUERY,
            &_ => panic!("Not implemented {:?}", request),
        };

        // system result is always ok in the mock implementation
        (
            SystemResult::Ok(contract_result),
            GasInfo::with_externally_used(gas_info),
        )
    }
}
