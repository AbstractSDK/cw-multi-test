use crate::wasm_emulation::api::RealApi;
use crate::wasm_emulation::input::ReplyArgs;
use crate::wasm_emulation::output::StorageChanges;
use crate::wasm_emulation::query::MockQuerier;
use crate::wasm_emulation::storage::DualStorage;
use cosmwasm_std::Checksum;
use cosmwasm_std::CustomMsg;
use cosmwasm_std::StdError;
use cosmwasm_vm::{
    call_execute, call_instantiate, call_migrate, call_query, call_reply, call_sudo, Backend,
    BackendApi, Instance, InstanceOptions, Querier, Size,
};
use cw_orch::daemon::queriers::CosmWasm;

use cosmwasm_std::Order;
use cosmwasm_std::Storage;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;

use crate::wasm_emulation::input::InstanceArguments;
use crate::wasm_emulation::output::WasmRunnerOutput;

use cosmwasm_vm::internals::check_wasm;
use std::collections::HashSet;

use crate::Contract;

use cosmwasm_std::{Binary, CustomQuery, Deps, DepsMut, Env, MessageInfo, Reply, Response};

use anyhow::Result as AnyResult;

use super::input::ExecuteArgs;
use super::input::InstantiateArgs;
use super::input::MigrateArgs;
use super::input::QueryArgs;
use super::input::SudoArgs;
use super::input::WasmFunction;
use super::output::WasmOutput;
use super::query::mock_querier::ForkState;

fn apply_storage_changes<ExecC>(storage: &mut dyn Storage, output: &WasmRunnerOutput<ExecC>) {
    // We change all the values with the output
    for (key, value) in &output.storage.current_keys {
        storage.set(key, value);
    }

    // We remove all values that need to be removed from it
    for key in &output.storage.removed_keys {
        storage.remove(key);
    }
}

/// Taken from cosmwasm_vm::testing
/// This gas limit is used in integration tests and should be high enough to allow a reasonable
/// number of contract executions and queries on one instance. For this reason it is significatly
/// higher than the limit for a single execution that we have in the production setup.
const DEFAULT_GAS_LIMIT: u64 = 500_000_000_000_000; // ~0.5s
const DEFAULT_MEMORY_LIMIT: Option<Size> = Some(Size::mebi(16));

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DistantContract {
    pub contract_addr: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DistantCodeId {
    pub code_id: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LocalWasmContract {
    pub code: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum WasmContract {
    Local(LocalWasmContract),
    DistantContract(DistantContract),
    DistantCodeId(DistantCodeId),
}

impl std::fmt::Debug for LocalWasmContract {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "LocalContract {{ checksum: {} }}",
            Checksum::generate(&self.code),
        )
    }
}

impl WasmContract {
    pub fn new_local(code: Vec<u8>) -> Self {
        check_wasm(
            &code,
            &HashSet::from([
                "iterator".to_string(),
                "staking".to_string(),
                "stargate".to_string(),
            ]),
        )
        .unwrap();
        Self::Local(LocalWasmContract { code })
    }

    pub fn new_distant_contract(contract_addr: String) -> Self {
        Self::DistantContract(DistantContract { contract_addr })
    }

    pub fn new_distant_code_id(code_id: u64) -> Self {
        Self::DistantCodeId(DistantCodeId { code_id })
    }

    pub fn get_code<ExecC: CustomMsg + 'static, QueryC: CustomQuery + DeserializeOwned>(
        &self,
        fork_state: ForkState<ExecC, QueryC>,
    ) -> AnyResult<Vec<u8>> {
        match self {
            WasmContract::Local(LocalWasmContract { code, .. }) => Ok(code.clone()),
            WasmContract::DistantContract(DistantContract { contract_addr }) => {
                let wasm_querier = CosmWasm {
                    channel: fork_state.remote.channel.clone(),
                    rt_handle: Some(fork_state.remote.rt.clone()),
                };

                let code_info = fork_state
                    .remote
                    .rt
                    .block_on(wasm_querier._contract_info(contract_addr))?;
                let code = fork_state
                    .remote
                    .rt
                    .block_on(wasm_querier._code_data(code_info.code_id))?;
                Ok(code)
            }
            WasmContract::DistantCodeId(DistantCodeId { code_id }) => {
                let wasm_querier = CosmWasm {
                    channel: fork_state.remote.channel.clone(),
                    rt_handle: Some(fork_state.remote.rt.clone()),
                };

                let code = fork_state
                    .remote
                    .rt
                    .block_on(wasm_querier._code_data(*code_id))?;
                Ok(code)
            }
        }
    }

    pub fn run_contract<
        QueryC: CustomQuery + DeserializeOwned + 'static,
        ExecC: CustomMsg + DeserializeOwned,
    >(
        &self,
        args: InstanceArguments,
        fork_state: ForkState<ExecC, QueryC>,
    ) -> AnyResult<WasmRunnerOutput<ExecC>> {
        let InstanceArguments {
            function,
            init_storage,
        } = args;
        let address = function.get_address();
        let code = self.get_code(fork_state.clone())?;

        let api = RealApi::new(&fork_state.remote.pub_address_prefix);

        // We create the backend here from outside information;
        let backend = Backend {
            api,
            storage: DualStorage::new(
                fork_state.remote.clone(),
                address.to_string(),
                Some(init_storage),
            )?,
            querier: MockQuerier::<ExecC, QueryC>::new(fork_state),
        };
        let options = InstanceOptions {
            gas_limit: DEFAULT_GAS_LIMIT,
        };
        let memory_limit = DEFAULT_MEMORY_LIMIT;

        // Then we create the instance
        let mut instance = Instance::from_code(&code, backend, options, memory_limit)?;

        let gas_before = instance.get_gas_left();

        // Then we call the function that we wanted to call
        let result = execute_function(&mut instance, function)?;

        let gas_after = instance.get_gas_left();

        // We return the code response + any storage change (or the whole local storage object), with serializing
        let mut recycled_instance = instance.recycle().unwrap();

        let wasm_result = WasmRunnerOutput {
            storage: StorageChanges {
                current_keys: recycled_instance.storage.get_all_storage()?,
                removed_keys: recycled_instance.storage.removed_keys.into_iter().collect(),
            },
            gas_used: gas_before - gas_after,
            wasm: result,
        };

        Ok(wasm_result)
    }

    pub fn after_execution_callback<ExecC>(&self, output: &WasmRunnerOutput<ExecC>) {
        // We log the gas used
        let operation = match output.wasm {
            WasmOutput::Execute(_) => "execution",
            WasmOutput::Query(_) => "query",
            WasmOutput::Instantiate(_) => "instantiation",
            WasmOutput::Migrate(_) => "migration",
            WasmOutput::Sudo(_) => "sudo",
            WasmOutput::Reply(_) => "reply",
        };
        log::debug!(
            "Gas used {:?} for {:} on contract {:?}",
            output.gas_used,
            operation,
            self
        );
    }
}

impl<ExecC, QueryC> Contract<ExecC, QueryC> for WasmContract
where
    ExecC: CustomMsg + DeserializeOwned,
    QueryC: CustomQuery + DeserializeOwned,
{
    fn execute(
        &self,
        deps: DepsMut<QueryC>,
        env: Env,
        info: MessageInfo,
        msg: Vec<u8>,
        fork_state: ForkState<ExecC, QueryC>,
    ) -> AnyResult<Response<ExecC>> {
        // We start by building the dependencies we will pass through the wasm executer
        let execute_args = InstanceArguments {
            function: WasmFunction::Execute(ExecuteArgs { env, info, msg }),
            init_storage: deps.storage.range(None, None, Order::Ascending).collect(),
        };

        let decoded_result = self.run_contract(execute_args, fork_state)?;

        apply_storage_changes(deps.storage, &decoded_result);
        self.after_execution_callback(&decoded_result);

        match decoded_result.wasm {
            WasmOutput::Execute(x) => Ok(x),
            _ => panic!("Wrong kind of answer from wasm container"),
        }
    }

    fn instantiate(
        &self,
        deps: DepsMut<QueryC>,
        env: Env,
        info: MessageInfo,
        msg: Vec<u8>,
        fork_state: ForkState<ExecC, QueryC>,
    ) -> AnyResult<Response<ExecC>> {
        // We start by building the dependencies we will pass through the wasm executer
        let instantiate_arguments = InstanceArguments {
            function: WasmFunction::Instantiate(InstantiateArgs { env, info, msg }),
            init_storage: deps.storage.range(None, None, Order::Ascending).collect(),
        };

        let decoded_result = self.run_contract(instantiate_arguments, fork_state)?;

        apply_storage_changes(deps.storage, &decoded_result);
        self.after_execution_callback(&decoded_result);

        match decoded_result.wasm {
            WasmOutput::Instantiate(x) => Ok(x),
            _ => panic!("Wrong kind of answer from wasm container"),
        }
    }

    fn query(
        &self,
        deps: Deps<QueryC>,
        env: Env,
        msg: Vec<u8>,
        fork_state: ForkState<ExecC, QueryC>,
    ) -> AnyResult<Binary> {
        // We start by building the dependencies we will pass through the wasm executer
        let query_arguments = InstanceArguments {
            function: WasmFunction::Query(QueryArgs { env, msg }),
            init_storage: deps.storage.range(None, None, Order::Ascending).collect(),
        };

        let decoded_result: WasmRunnerOutput<ExecC> =
            self.run_contract(query_arguments, fork_state)?;

        self.after_execution_callback(&decoded_result);

        match decoded_result.wasm {
            WasmOutput::Query(x) => Ok(x),
            _ => panic!("Wrong kind of answer from wasm container"),
        }
    }

    // this returns an error if the contract doesn't implement sudo
    fn sudo(
        &self,
        deps: DepsMut<QueryC>,
        env: Env,
        msg: Vec<u8>,
        fork_state: ForkState<ExecC, QueryC>,
    ) -> AnyResult<Response<ExecC>> {
        let sudo_args = InstanceArguments {
            function: WasmFunction::Sudo(SudoArgs { env, msg }),
            init_storage: deps.storage.range(None, None, Order::Ascending).collect(),
        };

        let decoded_result = self.run_contract(sudo_args, fork_state)?;

        apply_storage_changes(deps.storage, &decoded_result);
        self.after_execution_callback(&decoded_result);

        match decoded_result.wasm {
            WasmOutput::Sudo(x) => Ok(x),
            _ => panic!("Wrong kind of answer from wasm container"),
        }
    }

    // this returns an error if the contract doesn't implement reply
    fn reply(
        &self,
        deps: DepsMut<QueryC>,
        env: Env,
        reply: Reply,
        fork_state: ForkState<ExecC, QueryC>,
    ) -> AnyResult<Response<ExecC>> {
        let reply_args = InstanceArguments {
            function: WasmFunction::Reply(ReplyArgs { env, reply }),
            init_storage: deps.storage.range(None, None, Order::Ascending).collect(),
        };

        let decoded_result = self.run_contract(reply_args, fork_state)?;

        apply_storage_changes(deps.storage, &decoded_result);
        self.after_execution_callback(&decoded_result);

        match decoded_result.wasm {
            WasmOutput::Reply(x) => Ok(x),
            _ => panic!("Wrong kind of answer from wasm container"),
        }
    }

    // this returns an error if the contract doesn't implement migrate
    fn migrate(
        &self,
        deps: DepsMut<QueryC>,
        env: Env,
        msg: Vec<u8>,
        fork_state: ForkState<ExecC, QueryC>,
    ) -> AnyResult<Response<ExecC>> {
        let migrate_args = InstanceArguments {
            function: WasmFunction::Migrate(MigrateArgs { env, msg }),
            init_storage: deps.storage.range(None, None, Order::Ascending).collect(),
        };

        let decoded_result = self.run_contract(migrate_args, fork_state)?;

        apply_storage_changes(deps.storage, &decoded_result);
        self.after_execution_callback(&decoded_result);

        match decoded_result.wasm {
            WasmOutput::Migrate(x) => Ok(x),
            _ => panic!("Wrong kind of answer from wasm container"),
        }
    }
}

pub fn execute_function<
    A: BackendApi + 'static,
    B: cosmwasm_vm::Storage + 'static,
    C: Querier + 'static,
    ExecC: CustomMsg + DeserializeOwned,
>(
    instance: &mut Instance<A, B, C>,
    function: WasmFunction,
) -> AnyResult<WasmOutput<ExecC>> {
    match function {
        WasmFunction::Execute(args) => {
            let result = call_execute(instance, &args.env, &args.info, &args.msg)?
                .into_result()
                .map_err(StdError::generic_err)?;
            Ok(WasmOutput::Execute(result))
        }
        WasmFunction::Query(args) => {
            let result = call_query(instance, &args.env, &args.msg)?
                .into_result()
                .map_err(StdError::generic_err)?;
            Ok(WasmOutput::Query(result))
        }
        WasmFunction::Instantiate(args) => {
            let result = call_instantiate(instance, &args.env, &args.info, &args.msg)?
                .into_result()
                .map_err(StdError::generic_err)?;
            Ok(WasmOutput::Instantiate(result))
        }
        WasmFunction::Reply(args) => {
            let result = call_reply(instance, &args.env, &args.reply)?
                .into_result()
                .map_err(StdError::generic_err)?;
            Ok(WasmOutput::Reply(result))
        }
        WasmFunction::Migrate(args) => {
            let result = call_migrate(instance, &args.env, &args.msg)?
                .into_result()
                .map_err(StdError::generic_err)?;
            Ok(WasmOutput::Migrate(result))
        }
        WasmFunction::Sudo(args) => {
            let result = call_sudo(instance, &args.env, &args.msg)?
                .into_result()
                .map_err(StdError::generic_err)?;
            Ok(WasmOutput::Sudo(result))
        }
    }
}
