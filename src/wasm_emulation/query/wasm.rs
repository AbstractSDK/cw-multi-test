use std::marker::PhantomData;

use crate::prefixed_storage::get_full_contract_storage_namespace;
use crate::queries::wasm::WasmRemoteQuerier;
use crate::wasm_emulation::query::gas::{
    GAS_COST_ALL_QUERIES, GAS_COST_CONTRACT_INFO, GAS_COST_RAW_COSMWASM_QUERY,
};
use crate::wasm_emulation::query::mock_querier::QueryResultWithGas;
use crate::wasm_emulation::query::MockQuerier;
use crate::Contract;

use crate::wasm_emulation::contract::WasmContract;
use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_vm::GasInfo;

use cosmwasm_std::{
    to_json_binary, Addr, ContractInfoResponse, CustomMsg, CustomQuery, OwnedDeps, Storage,
    SystemError, SystemResult,
};
use cosmwasm_std::{ContractInfo, ContractResult};

use cosmwasm_std::WasmQuery;
use serde::de::DeserializeOwned;

use crate::wasm_emulation::channel::RemoteChannel;

use super::mock_querier::ForkState;

pub struct WasmQuerier<
    ExecC: CustomMsg + DeserializeOwned + 'static,
    QueryC: CustomQuery + DeserializeOwned + 'static,
> {
    fork_state: ForkState<ExecC, QueryC>,
}

impl<
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    > WasmQuerier<ExecC, QueryC>
{
    pub fn new(fork_state: ForkState<ExecC, QueryC>) -> Self {
        Self { fork_state }
    }

    pub fn query(&self, remote: RemoteChannel, request: &WasmQuery) -> QueryResultWithGas {
        match request {
            WasmQuery::ContractInfo { contract_addr } => {
                let addr = Addr::unchecked(contract_addr);
                let data = if let Some(local_contract) = self
                    .fork_state
                    .querier_storage
                    .wasm
                    .contracts
                    .get(contract_addr)
                {
                    local_contract.clone()
                } else {
                    WasmRemoteQuerier::load_distant_contract(self.fork_state.remote.clone(), &addr)
                        .unwrap()
                };
                let mut response = ContractInfoResponse::default();
                response.code_id = data.code_id;
                response.creator = data.creator.to_string();
                response.admin = data.admin.map(|a| a.to_string());
                (
                    SystemResult::Ok(to_json_binary(&response).into()),
                    GasInfo::with_externally_used(GAS_COST_CONTRACT_INFO),
                )
            }
            WasmQuery::Raw { contract_addr, key } => {
                // We first try to load that information locally
                let mut total_key =
                    get_full_contract_storage_namespace(&Addr::unchecked(contract_addr)).to_vec();
                total_key.extend_from_slice(key);

                let value: Vec<u8> = if let Some(value) = self
                    .fork_state
                    .querier_storage
                    .wasm
                    .storage
                    .iter()
                    .find(|e| e.0 == total_key)
                {
                    value.1.clone()
                } else {
                    WasmRemoteQuerier::raw_query(remote, contract_addr.clone(), key.clone())
                        .unwrap()
                };

                (
                    SystemResult::Ok(ContractResult::Ok(value.into())),
                    GasInfo::with_externally_used(GAS_COST_RAW_COSMWASM_QUERY),
                )
            }
            WasmQuery::Smart { contract_addr, msg } => {
                let addr = Addr::unchecked(contract_addr);
                println!("Trying to query {:?}", contract_addr);

                let mut storage = MockStorage::default();
                // Set the storage
                for (key, value) in self
                    .fork_state
                    .querier_storage
                    .wasm
                    .get_contract_storage(&addr)
                {
                    storage.set(&key, &value);
                }

                let deps = OwnedDeps {
                    storage,
                    api: MockApi::default(),
                    querier: MockQuerier::new(self.fork_state.clone()),
                    custom_query_type: PhantomData::<QueryC>,
                };
                let mut env = self.fork_state.local_state.env.clone();
                env.contract = ContractInfo {
                    address: Addr::unchecked(contract_addr),
                };

                let result = if let Some(local_contract) = self
                    .fork_state
                    .querier_storage
                    .wasm
                    .contracts
                    .get(contract_addr)
                {
                    // If the contract data is already defined in our storage, we load it from there
                    if let Some(code) = self
                        .fork_state
                        .querier_storage
                        .wasm
                        .codes
                        .get(&(local_contract.code_id as usize))
                    {
                        // Local Wasm Contract case
                        <WasmContract as Contract<ExecC, QueryC>>::query(
                            code,
                            deps.as_ref(),
                            env,
                            msg.to_vec(),
                            self.fork_state.clone(),
                        )
                    } else if let Some(local_contract) = self
                        .fork_state
                        .local_state
                        .contracts
                        .get(&(local_contract.code_id as usize))
                    {
                        // Local Rust Contract case
                        unsafe {
                            local_contract.as_ref().unwrap().query(
                                deps.as_ref(),
                                env,
                                msg.to_vec(),
                                self.fork_state.clone(),
                            )
                        }
                    } else {
                        // Distant Registered Contract case
                        <WasmContract as Contract<ExecC, QueryC>>::query(
                            &WasmContract::new_distant_code_id(local_contract.code_id),
                            deps.as_ref(),
                            env,
                            msg.to_vec(),
                            self.fork_state.clone(),
                        )
                    }
                } else {
                    // Distant UnRegistered Contract case
                    <WasmContract as Contract<ExecC, QueryC>>::query(
                        &WasmContract::new_distant_contract(contract_addr.to_string()),
                        deps.as_ref(),
                        env,
                        msg.to_vec(),
                        self.fork_state.clone(),
                    )
                };

                let result = if let Err(e) = result {
                    return (
                        SystemResult::Err(SystemError::InvalidRequest {
                            error: format!("Error querying a contract: {}", e),
                            request: msg.clone(),
                        }),
                        GasInfo::with_externally_used(0),
                    );
                } else {
                    result.unwrap()
                };

                (
                    SystemResult::Ok(ContractResult::Ok(result)),
                    GasInfo::with_externally_used(GAS_COST_ALL_QUERIES),
                )
            }
            #[cfg(feature = "cosmwasm_1_2")]
            WasmQuery::CodeInfo { code_id } => {
                let code_data = self
                    .fork_state
                    .querier_storage
                    .wasm
                    .code_data
                    .get(&(*code_id as usize));
                let res = if let Some(code_data) = code_data {
                    let mut res = cosmwasm_std::CodeInfoResponse::default();
                    res.code_id = *code_id;
                    res.creator = code_data.creator.to_string();
                    res.checksum = code_data.checksum.clone();
                    res
                } else {
                    WasmRemoteQuerier::code_info(self.fork_state.remote.clone(), *code_id).unwrap()
                };
                (
                    SystemResult::Ok(to_json_binary(&res).into()),
                    GasInfo::with_externally_used(GAS_COST_CONTRACT_INFO),
                )
            }
            _ => unimplemented!(),
        }
    }
}
