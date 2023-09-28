use crate::wasm_emulation::input::SerChainData;
use crate::wasm_emulation::query::bank::BankQuerier;
use crate::wasm_emulation::query::staking::StakingQuerier;
use crate::wasm_emulation::query::wasm::WasmQuerier;

use cosmwasm_vm::BackendResult;
use cosmwasm_vm::GasInfo;

use serde::de::DeserializeOwned;

use cosmwasm_std::Binary;
use cosmwasm_std::Coin;

use cosmwasm_std::SystemError;

use cosmwasm_std::from_slice;
use cosmwasm_std::{ContractResult, Empty, SystemResult};
use cosmwasm_std::{CustomQuery, QueryRequest};
use cosmwasm_std::{FullDelegation, Validator};

use cosmwasm_std::Attribute;
use cosmwasm_std::QuerierResult;

use crate::wasm_emulation::input::QuerierStorage;

use super::gas::GAS_COST_QUERY_ERROR;

pub type QueryResultWithGas = (QuerierResult, GasInfo);

/// The same type as cosmwasm-std's QuerierResult, but easier to reuse in
/// cosmwasm-vm. It might diverge from QuerierResult at some point.
pub type MockQuerierCustomHandlerResult = SystemResult<ContractResult<Binary>>;

/// MockQuerier holds an immutable table of bank balances
/// and configurable handlers for Wasm queries and custom queries.
pub struct MockQuerier<C: DeserializeOwned = Empty> {
    bank: BankQuerier,

    staking: StakingQuerier,
    wasm: WasmQuerier,
    /// A handler to handle custom queries. This is set to a dummy handler that
    /// always errors by default. Update it via `with_custom_handler`.
    ///
    /// Use box to avoid the need of another generic type
    custom_handler: Box<dyn for<'a> Fn(&'a C) -> QueryResultWithGas>,
}

impl<C: DeserializeOwned> MockQuerier<C> {
    pub fn new(chain: impl Into<SerChainData>, storage: Option<QuerierStorage>) -> Self {
        let chain = chain.into();
        MockQuerier {
            bank: BankQuerier::new(
                chain.clone(),
                storage.as_ref().map(|storage| storage.bank.storage.clone()),
            ),

            staking: StakingQuerier::default(),
            wasm: WasmQuerier::new(chain, storage),
            // strange argument notation suggested as a workaround here: https://github.com/rust-lang/rust/issues/41078#issuecomment-294296365
            custom_handler: Box::from(|_: &_| -> QueryResultWithGas {
                (
                    SystemResult::Err(SystemError::UnsupportedRequest {
                        kind: "custom".to_string(),
                    }),
                    GasInfo::free(),
                )
            }),
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
        CH: Fn(&C) -> QueryResultWithGas,
    {
        self.custom_handler = Box::from(handler);
        self
    }
}

impl<C: CustomQuery + DeserializeOwned> cosmwasm_vm::Querier for MockQuerier<C> {
    fn query_raw(
        &self,
        bin_request: &[u8],
        _gas_limit: u64,
    ) -> BackendResult<SystemResult<ContractResult<Binary>>> {
        let request: QueryRequest<C> = match from_slice(bin_request) {
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

impl<C: CustomQuery + DeserializeOwned> MockQuerier<C> {
    pub fn handle_query(&self, request: &QueryRequest<C>) -> QueryResultWithGas {
        match &request {
            QueryRequest::Bank(bank_query) => self.bank.query(bank_query),
            QueryRequest::Custom(custom_query) => (*self.custom_handler)(custom_query),

            QueryRequest::Staking(staking_query) => self.staking.query(staking_query),
            QueryRequest::Wasm(msg) => self.wasm.query(msg),
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
