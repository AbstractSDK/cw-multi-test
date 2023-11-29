use crate::prefixed_storage::get_full_contract_storage_namespace;
use crate::queries::wasm::WasmRemoteQuerier;
use crate::wasm_emulation::query::gas::{GAS_COST_CONTRACT_INFO, GAS_COST_RAW_COSMWASM_QUERY};
use crate::wasm_emulation::query::mock_querier::QueryResultWithGas;

use crate::wasm_emulation::contract::WasmContract;
use crate::wasm_emulation::input::QuerierStorage;
use crate::wasm_emulation::input::WasmFunction;
use crate::wasm_emulation::output::WasmOutput;
use crate::wasm_emulation::output::WasmRunnerOutput;
use cosmwasm_std::testing::mock_env;
use cosmwasm_vm::GasInfo;

use cosmwasm_std::{to_json_binary, Addr, ContractInfoResponse, SystemError, SystemResult};
use cosmwasm_std::{ContractResult, Empty};

use cosmwasm_std::WasmQuery;

use crate::wasm_emulation::channel::RemoteChannel;

use crate::wasm_emulation::input::{InstanceArguments, QueryArgs};

pub struct WasmQuerier {
    current_storage: QuerierStorage,
    remote: RemoteChannel,
}

impl WasmQuerier {
    pub fn new(remote: RemoteChannel, storage: Option<QuerierStorage>) -> Self {
        Self {
            current_storage: storage.unwrap_or(Default::default()),
            remote,
        }
    }

    pub fn query(&self, remote: RemoteChannel, request: &WasmQuery) -> QueryResultWithGas {
        match request {
            WasmQuery::ContractInfo { contract_addr } => {
                let addr = Addr::unchecked(contract_addr);
                let data = if let Some(local_contract) =
                    self.current_storage.wasm.contracts.get(contract_addr)
                {
                    local_contract.clone()
                } else {
                    WasmRemoteQuerier::load_distant_contract(self.remote.clone(), &addr).unwrap()
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
                    .current_storage
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
                // If the contract is already defined in our storage, we load it from there
                let contract = if let Some(local_contract) =
                    self.current_storage.wasm.contracts.get(contract_addr)
                {
                    if let Some(code_info) = self
                        .current_storage
                        .wasm
                        .codes
                        .get(&(local_contract.code_id as usize))
                    {
                        // We execute the query
                        code_info.clone()
                    } else {
                        WasmContract::new_distant_code_id(local_contract.code_id)
                    }
                } else {
                    WasmContract::new_distant_contract(contract_addr.to_string())
                };

                let mut env = mock_env();
                env.contract.address = addr.clone();
                // Here we specify empty because we only car about the query result
                let result: Result<WasmRunnerOutput<Empty>, _> = contract.run_contract(
                    InstanceArguments {
                        function: WasmFunction::Query(QueryArgs {
                            env,
                            msg: msg.to_vec(),
                        }),
                        querier_storage: self.current_storage.clone(),
                        init_storage: self.current_storage.wasm.get_contract_storage(&addr),
                    },
                    self.remote.clone(),
                );

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

                let bin = match result.wasm {
                    WasmOutput::Query(bin) => bin,
                    _ => panic!("Unexpected contract response, not possible"),
                };

                (
                    SystemResult::Ok(ContractResult::Ok(bin)),
                    GasInfo::with_externally_used(result.gas_used),
                )
            }
            #[cfg(feature = "cosmwasm_1_2")]
            WasmQuery::CodeInfo { code_id } => {
                let code_data = self
                    .current_storage
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
                    WasmRemoteQuerier::code_info(self.remote.clone(), *code_id).unwrap()
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
