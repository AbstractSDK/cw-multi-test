use crate::{
    prefixed_storage::{decode_length, to_length_prefixed, CONTRACT_STORAGE_PREFIX},
    wasm_emulation::channel::RemoteChannel,
    BankKeeper, Distribution, Gov, Ibc, Module, Staking, WasmKeeper,
};
use cosmwasm_std::{Addr, Api, Coin, CustomMsg, CustomQuery, Storage};
use cw_orch::prelude::BankQuerier;
use cw_utils::NativeBalance;
use rustc_serialize::json::Json;
use serde::de::DeserializeOwned;
use serde::Serialize;
use treediff::diff;
use treediff::tools::Recorder;

use crate::wasm::NAMESPACE_WASM;

use crate::{wasm_emulation::input::QuerierStorage, App};

use anyhow::Result as AnyResult;

#[derive(Serialize)]
pub struct SerializableCoin {
    amount: String,
    denom: String,
}

pub struct StorageAnalyzer {
    pub storage: QuerierStorage,
    pub remote: RemoteChannel,
}

impl StorageAnalyzer {
    pub fn new<ApiT, StorageT, CustomT, StakingT, DistrT, IbcT, GovT>(
        app: &App<
            BankKeeper,
            ApiT,
            StorageT,
            CustomT,
            WasmKeeper<CustomT::ExecT, CustomT::QueryT>,
            StakingT,
            DistrT,
            IbcT,
            GovT,
        >,
    ) -> AnyResult<Self>
    where
        CustomT::ExecT: CustomMsg + DeserializeOwned + 'static,
        CustomT::QueryT: CustomQuery + DeserializeOwned + 'static,

        ApiT: Api,
        StorageT: Storage,
        CustomT: Module,
        StakingT: Staking,
        DistrT: Distribution,
        IbcT: Ibc,
        GovT: Gov,
    {
        Ok(Self {
            storage: app.get_querier_storage()?,
            remote: app.remote.clone(),
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
                    String::from_utf8_lossy(&key).to_string(),
                    String::from_utf8_lossy(&value).to_string(),
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
                    String::from_utf8_lossy(&resulting_key[2..(addr_len + 2)]).to_string();

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
                    String::from_utf8_lossy(&key).to_string(),
                    String::from_utf8_lossy(&value).to_string(),
                )
            })
            .collect()
    }

    pub fn compare_all_readable_contract_storage(&self) {
        let wasm_querier = cw_orch::daemon::queriers::CosmWasm {
            channel: self.remote.channel.clone(),
            rt_handle: Some(self.remote.rt.clone()),
        };
        self.all_contract_storage()
            .into_iter()
            .for_each(|(contract_addr, key, value)| {
                // We look for the data at that key on the contract
                let distant_data = self
                    .remote
                    .rt
                    .block_on(wasm_querier._contract_raw_state(contract_addr.clone(), key.clone()));

                if let Ok(data) = distant_data {
                    let local_json: Json =
                        if let Ok(v) = String::from_utf8_lossy(&value).to_string().parse() {
                            v
                        } else {
                            log::info!(
                                "Storage at {}, and key {}, was : {:x?}, now {:x?}",
                                contract_addr,
                                String::from_utf8_lossy(&key).to_string(),
                                data.data,
                                value
                            );
                            return;
                        };
                    let distant_json: Json =
                        if let Ok(v) = String::from_utf8_lossy(&data.data).to_string().parse() {
                            v
                        } else {
                            log::info!(
                                "Storage at {}, and key {}, was : {:x?}, now {:x?}",
                                contract_addr,
                                String::from_utf8_lossy(&key).to_string(),
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
                        String::from_utf8_lossy(&key).to_string(),
                        changes
                    );
                } else if let Ok(v) = String::from_utf8_lossy(&value).to_string().parse::<Json>() {
                    log::info!(
                        "Storage at {}, and key {}, is new : {}",
                        contract_addr,
                        String::from_utf8_lossy(&key).to_string(),
                        v
                    );
                } else {
                    log::info!(
                        "Storage at {}, and key {}, is new : {:?}",
                        contract_addr,
                        String::from_utf8_lossy(&key).to_string(),
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

    pub fn compare_all_balances(&self) {
        let bank_querier = cw_orch::daemon::queriers::Bank {
            channel: self.remote.channel.clone(),
            rt_handle: Some(self.remote.rt.clone()),
        };
        self.get_all_local_balances()
            .into_iter()
            .for_each(|(addr, balances)| {
                // We look for the data at that key on the contract
                let distant_data = bank_querier.balance(addr.clone(), None);

                if let Ok(distant_coins) = distant_data {
                    let distant_coins = serde_json::to_string(&distant_coins).unwrap();
                    let distant_coins: Json = distant_coins.parse().unwrap();

                    let local_coins = serde_json::to_string(&balances.0).unwrap();
                    let local_coins: Json = local_coins.parse().unwrap();

                    let mut d = Recorder::default();
                    diff(&distant_coins, &local_coins, &mut d);

                    let changes: Vec<_> = d
                        .calls
                        .iter()
                        .filter(|change| {
                            !matches!(change, treediff::tools::ChangeType::Unchanged(..))
                        })
                        .collect();

                    log::info!("Bank balance for {} changed like so : {:?}", addr, changes);
                }
            });
    }

    pub fn get_all_local_balances(&self) -> Vec<(Addr, NativeBalance)> {
        self.storage.bank.storage.clone()
    }
}