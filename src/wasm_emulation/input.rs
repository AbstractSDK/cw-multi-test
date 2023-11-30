use std::collections::HashMap;

use cosmwasm_std::Addr;
use cosmwasm_std::{Env, MessageInfo, Reply};

use cw_utils::NativeBalance;

use crate::prefixed_storage::get_full_contract_storage_namespace;
use crate::wasm::{CodeData, ContractData};

use super::contract::WasmContract;

#[derive(Debug, Clone, Default)]
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

#[derive(Debug, Clone, Default)]
pub struct BankStorage {
    pub storage: Vec<(Addr, NativeBalance)>,
}

#[derive(Clone, Default)]
pub struct QuerierStorage {
    pub wasm: WasmStorage,
    pub bank: BankStorage,
}

#[derive(Debug)]
pub struct InstanceArguments {
    pub function: WasmFunction,
    pub init_storage: Vec<(Vec<u8>, Vec<u8>)>,
}

#[derive(Debug)]
pub enum WasmFunction {
    Execute(ExecuteArgs),
    Instantiate(InstantiateArgs),
    Query(QueryArgs),
    Sudo(SudoArgs),
    Reply(ReplyArgs),
    Migrate(MigrateArgs),
}

#[derive(Debug)]
pub struct ExecuteArgs {
    pub env: Env,
    pub info: MessageInfo,
    pub msg: Vec<u8>,
}

#[derive(Debug)]
pub struct InstantiateArgs {
    pub env: Env,
    pub info: MessageInfo,
    pub msg: Vec<u8>,
}

#[derive(Debug)]
pub struct QueryArgs {
    pub env: Env,
    pub msg: Vec<u8>,
}

#[derive(Debug)]
pub struct SudoArgs {
    pub env: Env,
    pub msg: Vec<u8>,
}

#[derive(Debug)]
pub struct ReplyArgs {
    pub env: Env,
    pub reply: Reply,
}

#[derive(Debug)]
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
