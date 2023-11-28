use std::collections::HashMap;

use anyhow::Result as AnyResult;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Addr;
use cosmwasm_std::Binary;
use cosmwasm_std::CustomQuery;
use cosmwasm_std::QuerierWrapper;
use cosmwasm_std::QueryRequest;
use cosmwasm_std::{Env, MessageInfo, Reply};

use cw_utils::NativeBalance;

use crate::bank::BankKeeper;
use crate::prefixed_storage::get_full_contract_storage_namespace;
use crate::wasm::{CodeData, ContractData};

use super::contract::WasmContract;
use super::query::AllQuerier;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct WasmStorage {
    pub contracts: HashMap<String, ContractData>,
    pub codes: HashMap<usize, WasmContract>,
    pub code_data: HashMap<usize, CodeData>,
    pub storage: Vec<(Vec<u8>, Vec<u8>)>,
}

impl WasmStorage {
    pub fn get_contract_storage(&self, contract_addr: &Addr) -> Vec<(Vec<u8>, Vec<u8>)> {
        let namespace =
            get_full_contract_storage_namespace(&Addr::unchecked(contract_addr)).to_vec();
        let namespace_len = namespace.len();
        let keys: Vec<(Vec<u8>, Vec<u8>)> = self
            .storage
            .iter()
            // We filter only value in this namespace
            .filter(|(k, _)| k.len() >= namespace_len && k[..namespace_len] == namespace)
            .cloned()
            // We remove the namespace prefix from the key
            .map(|(k, value)| (k[namespace_len..].to_vec(), value))
            .collect();

        keys
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct BankStorage {
    pub storage: Vec<(Addr, NativeBalance)>,
}

#[derive(Serialize, Clone, Deserialize, Default, Debug)]
pub struct QuerierStorage {
    pub wasm: WasmStorage,
    pub bank: <BankKeeper as AllQuerier>::Output,
}

pub const STARGATE_ALL_WASM_QUERY_URL: &str = "/local.wasm.all";
pub const STARGATE_ALL_BANK_QUERY_URL: &str = "/local.bank.all";

pub fn get_querier_storage<QueryC: CustomQuery>(
    q: &QuerierWrapper<QueryC>,
) -> AnyResult<QuerierStorage> {
    // We get the wasm storage for all wasm contract to make sure we dispatch everything (with the mock Querier)
    let wasm = q.query(&QueryRequest::Stargate {
        path: STARGATE_ALL_WASM_QUERY_URL.to_string(),
        data: Binary(vec![]),
    })?;
    let bank = q.query(&QueryRequest::Stargate {
        path: STARGATE_ALL_BANK_QUERY_URL.to_string(),
        data: Binary(vec![]),
    })?;
    Ok(QuerierStorage { wasm, bank })
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstanceArguments {
    pub function: WasmFunction,
    pub init_storage: Vec<(Vec<u8>, Vec<u8>)>,
    pub querier_storage: QuerierStorage,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum WasmFunction {
    Execute(ExecuteArgs),
    Instantiate(InstantiateArgs),
    Query(QueryArgs),
    Sudo(SudoArgs),
    Reply(ReplyArgs),
    Migrate(MigrateArgs),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExecuteArgs {
    pub env: Env,
    pub info: MessageInfo,
    pub msg: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstantiateArgs {
    pub env: Env,
    pub info: MessageInfo,
    pub msg: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct QueryArgs {
    pub env: Env,
    pub msg: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SudoArgs {
    pub env: Env,
    pub msg: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReplyArgs {
    pub env: Env,
    pub reply: Reply,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MigrateArgs {
    pub env: Env,
    pub msg: Vec<u8>,
}

impl WasmFunction {
    pub fn get_address(&self) -> Addr {
        match self {
            WasmFunction::Execute(ExecuteArgs { env, .. }) => env.contract.address.clone(),
            WasmFunction::Instantiate(InstantiateArgs { env, .. }) => env.contract.address.clone(),
            WasmFunction::Query(QueryArgs { env, .. }) => env.contract.address.clone(),
            WasmFunction::Reply(ReplyArgs { env, .. }) => env.contract.address.clone(),
            WasmFunction::Sudo(SudoArgs { env, .. }) => env.contract.address.clone(),
            WasmFunction::Migrate(MigrateArgs { env, .. }) => env.contract.address.clone(),
        }
    }
}
