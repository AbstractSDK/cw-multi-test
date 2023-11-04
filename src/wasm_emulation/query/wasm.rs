use crate::prefixed_storage::get_full_contract_storage_namespace;
use crate::wasm_emulation::query::gas::{GAS_COST_CONTRACT_INFO, GAS_COST_RAW_COSMWASM_QUERY};
use crate::wasm_emulation::query::mock_querier::QueryResultWithGas;

use crate::wasm_emulation::contract::WasmContract;
use crate::wasm_emulation::input::QuerierStorage;
use crate::wasm_emulation::input::SerChainData;
use crate::wasm_emulation::input::WasmFunction;
use crate::wasm_emulation::output::WasmOutput;
use crate::wasm_emulation::output::WasmRunnerOutput;
use cosmwasm_std::testing::mock_env;
use cosmwasm_vm::GasInfo;
use cw_orch_daemon::queriers::DaemonQuerier;

use cosmwasm_std::{to_binary, Addr, ContractInfoResponse, SystemResult, SystemError};
use cosmwasm_std::{ContractResult, Empty};
use cw_orch_daemon::queriers::CosmWasm;

use cosmwasm_std::WasmQuery;

use crate::wasm_emulation::channel::get_channel;
use crate::WasmKeeper;

use crate::wasm_emulation::input::{InstanceArguments, QueryArgs};

pub struct WasmQuerier {
    chain: SerChainData,
    current_storage: QuerierStorage,
}

impl WasmQuerier {
    pub fn new(chain: impl Into<SerChainData>, storage: Option<QuerierStorage>) -> Self {
        let chain = chain.into();
        Self {
            chain,
            current_storage: storage.unwrap_or(Default::default()),
        }
    }

    pub fn query(&self, request: &WasmQuery) -> QueryResultWithGas {
        match request {
            WasmQuery::ContractInfo { contract_addr } => {
                let addr = Addr::unchecked(contract_addr);
                let data = if let Some(local_contract) =
                    self.current_storage.wasm.contracts.get(contract_addr)
                {
                    local_contract.clone()
                } else {
                    WasmKeeper::<Empty, Empty>::load_distant_contract(self.chain.clone(), &addr)
                        .unwrap()
                };
                let mut response = ContractInfoResponse::default();
                response.code_id = data.code_id.try_into().unwrap();
                response.creator = data.creator.to_string();
                response.admin = data.admin.map(|a| a.to_string());
                (
                    SystemResult::Ok(to_binary(&response).into()),
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
                    let (rt, channel) = get_channel(self.chain.clone()).unwrap();
                    let wasm_querier = CosmWasm::new(channel);
                    let query_result = rt
                        .block_on(
                            wasm_querier
                                .contract_raw_state(contract_addr.to_string(), key.to_vec()),
                        )
                        .map(|query_result| query_result.data);
                    query_result.unwrap()
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
                        WasmContract::new_distant_code_id(
                            local_contract.code_id.try_into().unwrap(),
                            self.chain.clone(),
                        )
                    }
                } else {
                    WasmContract::new_distant_contract(
                        contract_addr.to_string(),
                        self.chain.clone(),
                    )
                };

                let mut env = mock_env();
                env.contract.address = addr.clone();
                // Here we specify empty because we only car about the query result
                let result: Result<WasmRunnerOutput<Empty>, _> = contract
                    .run_contract(InstanceArguments {
                        function: WasmFunction::Query(QueryArgs {
                            env,
                            msg: msg.to_vec(),
                        }),
                        querier_storage: self.current_storage.clone(),
                        init_storage: self.current_storage.wasm.get_contract_storage(&addr),
                    });

                let result = if let Err(e) = result{
                    return (
                        SystemResult::Err(SystemError::InvalidRequest{
                            error: format!("Error querying a contract: {}", e),
                            request: msg.clone()
                        }),
                        GasInfo::with_externally_used(0),
                    )
                }else{
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
            _ => unimplemented!(),
        }
    }
}
