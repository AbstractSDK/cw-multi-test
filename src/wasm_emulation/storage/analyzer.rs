use crate::prefixed_storage::decode_length;
use crate::prefixed_storage::to_length_prefixed;
use crate::prefixed_storage::CONTRACT_STORAGE_PREFIX;
use crate::wasm_emulation::channel::get_channel;
use crate::wasm_emulation::input::get_querier_storage;
use cosmwasm_std::Addr;
use cosmwasm_std::Coin;
use cw_orch_daemon::queriers::CosmWasm;
use cw_orch_daemon::queriers::DaemonQuerier;
use cw_utils::NativeBalance;
use ibc_chain_registry::chain::ChainData;
use rustc_serialize::json::Json;
use serde::__private::from_utf8_lossy;
use treediff::diff;
use treediff::tools::Recorder;

use crate::wasm::NAMESPACE_WASM;

use crate::{wasm_emulation::input::QuerierStorage, App};

use anyhow::Result as AnyResult;

pub struct StorageAnalyzer {
    pub storage: QuerierStorage,
    pub chain: ChainData,
}

impl StorageAnalyzer {
    pub fn new(app: &App, c: impl Into<ChainData>) -> AnyResult<Self> {
        Ok(Self {
            storage: get_querier_storage(&app.wrap())?,
            chain: c.into(),
        })
    }

    pub fn get_contract_storage(
        &self,
        contract_addr: impl Into<String>,
    ) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.storage
            .wasm
            .get_contract_storage(&Addr::unchecked(contract_addr.into()))
    }

    pub fn readable_storage(&self, contract_addr: impl Into<String>) -> Vec<(String, String)> {
        self.storage
            .wasm
            .get_contract_storage(&Addr::unchecked(contract_addr.into()))
            .into_iter()
            .map(|(key, value)| {
                (
                    from_utf8_lossy(&key).to_string(),
                    from_utf8_lossy(&value).to_string(),
                )
            })
            .collect()
    }

    /// We leverage the data structure we introduced for contracts to get their storage easily
    pub fn all_contract_storage(&self) -> Vec<(String, Vec<u8>, Vec<u8>)> {
        // In all wasm storage keys, we look for the `contract_addr/(...)/` pattern in the key
        self.storage
            .wasm
            .storage
            .iter()
            .filter(|(key, _)| {
                // The key must contain the NAMESPACE_WASM prefix
                let prefix = to_length_prefixed(NAMESPACE_WASM);
                key.len() >= prefix.len() && key[..prefix.len()] == prefix
            })
            .filter_map(|(key, value)| {
                // Now we need to get the contract addr from the namespace

                let prefix_len = to_length_prefixed(NAMESPACE_WASM).len();
                let resulting_key = &key[prefix_len..];
                let addr_len: usize = decode_length([resulting_key[0], resulting_key[1]])
                    .try_into()
                    .unwrap();
                let contract_addr_addr =
                    from_utf8_lossy(&resulting_key[2..(addr_len + 2)]).to_string();

                let split: Vec<_> = contract_addr_addr.split('/').collect();
                if split.len() != 2 || format!("{}/", split[0]) != CONTRACT_STORAGE_PREFIX {
                    return None;
                }

                Some((
                    split[1].to_string(),
                    resulting_key[addr_len + 2..].to_vec(),
                    value.clone(),
                ))
            })
            .collect()
    }

    pub fn all_readable_contract_storage(&self) -> Vec<(String, String, String)> {
        self.all_contract_storage()
            .into_iter()
            .map(|(contract, key, value)| {
                (
                    contract,
                    from_utf8_lossy(&key).to_string(),
                    from_utf8_lossy(&value).to_string(),
                )
            })
            .collect()
    }

    pub fn compare_all_readable_contract_storage(&self) {
        let (rt, channel) = get_channel(self.chain.clone()).unwrap();
        let wasm_querier = CosmWasm::new(channel);
        self.all_contract_storage()
            .into_iter()
            .for_each(|(contract_addr, key, value)| {
                // We look for the data at that key on the contract
                let distant_data = rt
                    .block_on(wasm_querier.contract_raw_state(contract_addr.clone(), key.clone()));

                if let Ok(data) = distant_data {
                    let local_json: Json =
                        if let Ok(v) = from_utf8_lossy(&value).to_string().parse() {
                            v
                        } else {
                            log::info!(
                                "Storage at {}, and key {}, was : {:x?}, now {:x?}",
                                contract_addr,
                                from_utf8_lossy(&key).to_string(),
                                data.data,
                                value
                            );
                            return;
                        };
                    let distant_json: Json =
                        if let Ok(v) = from_utf8_lossy(&data.data).to_string().parse() {
                            v
                        } else {
                            log::info!(
                                "Storage at {}, and key {}, was : {:x?}, now {:x?}",
                                contract_addr,
                                from_utf8_lossy(&key).to_string(),
                                data.data,
                                value
                            );
                            return;
                        };

                    let mut d = Recorder::default();
                    diff(&distant_json, &local_json, &mut d);

                    let changes: Vec<_> = d
                        .calls
                        .iter()
                        .filter(|change| {
                            !matches!(change, treediff::tools::ChangeType::Unchanged(..))
                        })
                        .collect();

                    log::info!(
                        "Storage at {}, and key {}, changed like so : {:?}",
                        contract_addr,
                        from_utf8_lossy(&key).to_string(),
                        changes
                    );
                } else if let Ok(v) = from_utf8_lossy(&value).to_string().parse::<Json>() {
                    log::info!(
                        "Storage at {}, and key {}, is new : {}",
                        contract_addr,
                        from_utf8_lossy(&key).to_string(),
                        v
                    );
                } else {
                    log::info!(
                        "Storage at {}, and key {}, is new : {:?}",
                        contract_addr,
                        from_utf8_lossy(&key).to_string(),
                        value
                    );
                }
            });
    }

    pub fn get_balance(&self, addr: impl Into<String>) -> Vec<Coin> {
        let addr: String = addr.into();
        self.storage
            .bank
            .storage
            .iter()
            .find(|(a, _)| a.as_str() == addr)
            .map(|(_, b)| b.0.clone())
            .unwrap_or(vec![])
    }

    pub fn get_all_local_balances(&self) -> Vec<(Addr, NativeBalance)> {
        self.storage.bank.storage.clone()
    }
}
