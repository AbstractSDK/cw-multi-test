use std::collections::HashMap;

use crate::wasm_emulation::channel::RemoteChannel;
use crate::wasm_emulation::query::bank::BankQuerier;
use crate::wasm_emulation::query::staking::StakingQuerier;
use crate::wasm_emulation::query::wasm::WasmQuerier;

use cosmwasm_std::CustomMsg;
use cosmwasm_std::Env;
use cosmwasm_vm::BackendResult;
use cosmwasm_vm::GasInfo;

use serde::de::DeserializeOwned;

use cosmwasm_std::Binary;
use cosmwasm_std::Coin;

use cosmwasm_std::SystemError;

use cosmwasm_std::from_json;
use cosmwasm_std::{ContractResult, SystemResult};
use cosmwasm_std::{CustomQuery, QueryRequest};
use cosmwasm_std::{FullDelegation, Validator};

use cosmwasm_std::Attribute;
use cosmwasm_std::QuerierResult;

use crate::wasm_emulation::input::QuerierStorage;
use crate::Contract;

use super::gas::GAS_COST_QUERY_ERROR;

#[derive(Clone)]
pub struct LocalForkedState<ExecC, QueryC> {
    pub contracts: HashMap<usize, *mut dyn Contract<ExecC, QueryC>>,
    pub env: Env,
}

#[derive(Clone)]
pub struct ForkState<ExecC, QueryC>
where
    QueryC: CustomQuery + DeserializeOwned + 'static,
    ExecC: CustomMsg + 'static,
{
    pub remote: RemoteChannel,
    /// Only query function right now, but we might pass along the whole application state to avoid stargate queries
    pub local_state: LocalForkedState<ExecC, QueryC>,
    pub querier_storage: QuerierStorage,
}

pub type QueryResultWithGas = (QuerierResult, GasInfo);

/// The same type as cosmwasm-std's QuerierResult, but easier to reuse in
/// cosmwasm-vm. It might diverge from QuerierResult at some point.
pub type MockQuerierCustomHandlerResult = SystemResult<ContractResult<Binary>>;

/// MockQuerier holds an immutable table of bank balances
/// and configurable handlers for Wasm queries and custom queries.
pub struct MockQuerier<
    ExecC: CustomMsg + DeserializeOwned + 'static,
    QueryC: CustomQuery + DeserializeOwned + 'static,
> {
    bank: BankQuerier,

    staking: StakingQuerier,
    wasm: WasmQuerier<ExecC, QueryC>,

    //Box<dyn Fn(Deps<'_, C>, Env, Vec<u8>) -> Result<Binary, anyhow::Error>>, //fn(deps: Deps<C>, env: Env, msg: Vec<u8>) -> Result<Binary, anyhow::Error>,
    /// A handler to handle custom queries. This is set to a dummy handler that
    /// always errors by default. Update it via `with_custom_handler`.
    ///
    /// Use box to avoid the need of another generic type
    custom_handler: Box<dyn for<'a> Fn(&'a QueryC) -> QueryResultWithGas>,
    remote: RemoteChannel,
}

impl<
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    > MockQuerier<ExecC, QueryC>
{
    pub fn new(fork_state: ForkState<ExecC, QueryC>) -> Self {
        // We create query_closures for all local_codes

        MockQuerier {
            bank: BankQuerier::new(
                fork_state.remote.clone(),
                fork_state.querier_storage.bank.storage.clone(),
            ),

            staking: StakingQuerier::default(),
            wasm: WasmQuerier::new(fork_state.clone()),
            // strange argument notation suggested as a workaround here: https://github.com/rust-lang/rust/issues/41078#issuecomment-294296365
            custom_handler: Box::from(|_: &_| -> QueryResultWithGas {
                (
                    SystemResult::Err(SystemError::UnsupportedRequest {
                        kind: "custom".to_string(),
                    }),
                    GasInfo::free(),
                )
            }),
            remote: fork_state.remote,
        }
    }

    // set a new balance for the given address and return the old balance
    pub fn update_balance(
        &mut self,
        addr: impl Into<String>,
        balance: Vec<Coin>,
    ) -> Option<Vec<Coin>> {
        self.bank.update_balance(addr, balance)
    }

    pub fn update_staking(
        &mut self,
        denom: &str,
        validators: &[Validator],
        delegations: &[FullDelegation],
    ) {
        self.staking = StakingQuerier::new(denom, validators, delegations);
    }

    pub fn with_custom_handler<CH: 'static>(mut self, handler: CH) -> Self
    where
        CH: Fn(&QueryC) -> QueryResultWithGas,
    {
        self.custom_handler = Box::from(handler);
        self
    }
}

impl<
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    > cosmwasm_vm::Querier for MockQuerier<ExecC, QueryC>
{
    fn query_raw(
        &self,
        bin_request: &[u8],
        _gas_limit: u64,
    ) -> BackendResult<SystemResult<ContractResult<Binary>>> {
        let request: QueryRequest<QueryC> = match from_json(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return (
                    Ok(SystemResult::Err(SystemError::InvalidRequest {
                        error: format!("Parsing query request: {}", e),
                        request: bin_request.into(),
                    })),
                    GasInfo::with_externally_used(GAS_COST_QUERY_ERROR),
                )
            }
        };
        let result = self.handle_query(&request);

        (Ok(result.0), result.1)
    }
}

impl<
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    > cosmwasm_std::Querier for MockQuerier<ExecC, QueryC>
{
    fn raw_query(&self, bin_request: &[u8]) -> SystemResult<ContractResult<Binary>> {
        let request: QueryRequest<QueryC> = match from_json(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };
        let result = self.handle_query(&request);

        result.0
    }
}

impl<
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    > MockQuerier<ExecC, QueryC>
{
    pub fn handle_query(&self, request: &QueryRequest<QueryC>) -> QueryResultWithGas {
        match &request {
            QueryRequest::Bank(bank_query) => self.bank.query(bank_query),
            QueryRequest::Custom(custom_query) => (*self.custom_handler)(custom_query),

            QueryRequest::Staking(staking_query) => self.staking.query(staking_query),
            QueryRequest::Wasm(msg) => self.wasm.query(self.remote.clone(), msg),
            QueryRequest::Stargate { .. } => (
                SystemResult::Err(SystemError::UnsupportedRequest {
                    kind: "Stargate".to_string(),
                }),
                GasInfo::with_externally_used(GAS_COST_QUERY_ERROR),
            ),
            &_ => panic!("Query Type Not implemented"),
        }
    }
}

pub fn digit_sum(input: &[u8]) -> usize {
    input.iter().fold(0, |sum, val| sum + (*val as usize))
}

/// Only for test code. This bypasses assertions in new, allowing us to create _*
/// Attributes to simulate responses from the blockchain
pub fn mock_wasmd_attr(key: impl Into<String>, value: impl Into<String>) -> Attribute {
    Attribute {
        key: key.into(),
        value: value.into(),
    }
}
