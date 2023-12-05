use crate::wasm_emulation::channel::RemoteChannel;
use crate::wasm_emulation::storage::mock_storage::{GAS_COST_LAST_ITERATION, GAS_COST_RANGE};

use super::mock_storage::MockStorage;
use cosmrs::proto::cosmos::base::query::v1beta1::PageRequest;
use cosmrs::proto::cosmwasm::wasm::v1::Model;
use cosmwasm_std::Order;
use cosmwasm_std::Record;
use cosmwasm_vm::BackendError;
use cosmwasm_vm::BackendResult;
use cosmwasm_vm::GasInfo;
use cosmwasm_vm::Storage;
use cw_orch_daemon::queriers::DaemonQuerier;
use num_bigint::{BigInt, Sign};
use std::collections::HashMap;
use std::iter;

use cw_orch_daemon::queriers::CosmWasm;

fn get_key_bigint(mut key1: Vec<u8>, mut key2: Vec<u8>) -> (BigInt, BigInt) {
    if key1.len() >= key2.len() {
        key2.extend(iter::repeat(0).take(key1.len() - key2.len()))
    } else {
        key1.extend(iter::repeat(0).take(key2.len() - key1.len()))
    }

    (
        BigInt::from_bytes_be(Sign::Plus, &key1),
        BigInt::from_bytes_be(Sign::Plus, &key2),
    )
}

fn gte(key1: Vec<u8>, key2: Vec<u8>) -> bool {
    let ints = get_key_bigint(key1, key2);

    ints.0 >= ints.1
}

fn _gt(key1: Vec<u8>, key2: Vec<u8>) -> bool {
    let ints = get_key_bigint(key1, key2);
    ints.0 > ints.1
}

use std::collections::HashSet;

use anyhow::Result as AnyResult;
const DISTANT_LIMIT: u64 = 5u64;

#[derive(Default, Debug, Clone)]
struct DistantIter {
    data: Vec<Model>,
    position: usize,
    key: Option<Vec<u8>>, // if set to None, there is no more keys to investigate in the distant container
    start: Option<Vec<u8>>,
    end: Option<Vec<u8>>,
    reverse: bool,
}

/// Iterator to get multiple keys
#[derive(Default, Debug, Clone)]
struct Iter {
    distant_iter: DistantIter,
    local_iter: u32,
}

pub struct DualStorage {
    pub local_storage: MockStorage,
    pub removed_keys: HashSet<Vec<u8>>,
    pub remote: RemoteChannel,
    pub contract_addr: String,
    iterators: HashMap<u32, Iter>,
}

impl DualStorage {
    pub fn new(
        remote: RemoteChannel,
        contract_addr: String,
        init: Option<Vec<(Vec<u8>, Vec<u8>)>>,
    ) -> AnyResult<DualStorage> {
        // We create an instance from a code_id, an address, and we run the code in it

        let mut local_storage = MockStorage::default();
        for (key, value) in init.unwrap() {
            local_storage.set(&key, &value).0?;
        }

        Ok(Self {
            local_storage,
            remote,
            removed_keys: HashSet::default(),
            contract_addr,
            iterators: HashMap::new(),
        })
    }

    pub fn get_all_storage(&mut self) -> AnyResult<Vec<(Vec<u8>, Vec<u8>)>> {
        let iterator_id = self.local_storage.scan(None, None, Order::Ascending).0?;
        let all_records = self.local_storage.all(iterator_id);

        Ok(all_records.0?)
    }
}

impl Storage for DualStorage {
    fn get(&self, key: &[u8]) -> BackendResult<Option<Vec<u8>>> {
        // First we try to get the value locally
        let (mut value, gas_info) = self.local_storage.get(key);
        // If it's not available, we query it online if it was not removed locally
        if !self.removed_keys.contains(key) && value.as_ref().unwrap().is_none() {
            let wasm_querier = CosmWasm::new(self.remote.channel.clone());

            let distant_result = self.remote.rt.block_on(
                wasm_querier.contract_raw_state(self.contract_addr.clone(), key.to_vec()),
            );

            if let Ok(result) = distant_result {
                if !result.data.is_empty() {
                    value = Ok(Some(result.data))
                }
            }
        }
        (value, gas_info)
    }

    fn scan(
        &mut self,
        start: Option<&[u8]>,
        end: Option<&[u8]>,
        order: Order,
    ) -> BackendResult<u32> {
        let gas_info = GasInfo::with_externally_used(GAS_COST_RANGE);
        let iterator_id = self.local_storage.scan(start, end, order).0.unwrap();

        let order_i32: i32 = order.try_into().unwrap();
        let descending_order: i32 = Order::Descending.try_into().unwrap();

        let querier_start = if order_i32 == descending_order {
            end.map(|s| s.to_vec()).unwrap_or(vec![])
        } else {
            start.map(|s| s.to_vec()).unwrap_or(vec![])
        };

        let iter = Iter {
            local_iter: iterator_id,
            distant_iter: DistantIter {
                data: vec![],
                position: 0,
                key: Some(querier_start),
                end: end.map(|e| e.to_vec()),
                start: start.map(|e| e.to_vec()),
                reverse: order_i32 == descending_order,
            },
        };

        let last_id: u32 = self
            .iterators
            .len()
            .try_into()
            .expect("Found more iterator IDs than supported");
        let new_id = last_id + 1;
        self.iterators.insert(new_id, iter.clone());

        (Ok(new_id), gas_info)
    }

    fn next(&mut self, iterator_id: u32) -> BackendResult<Option<Record>> {
        // In order to get the next element on the iterator, we need to compose with the two iterators we have
        let iterator = match self.iterators.get_mut(&iterator_id) {
            Some(i) => i,
            None => {
                println!("End next premature");
                return (
                    Err(BackendError::iterator_does_not_exist(iterator_id)),
                    GasInfo::free(),
                );
            }
        };
        // TODO, work with removed keys and don't take them

        // 1. We verify that there is enough elements in the distant iterator
        if iterator.distant_iter.position == iterator.distant_iter.data.len()
            && iterator.distant_iter.key.is_some()
        {
            let wasm_querier = CosmWasm::new(self.remote.channel.clone());
            let new_keys = self
                .remote
                .rt
                .block_on(wasm_querier.all_contract_state(
                    self.contract_addr.clone(),
                    Some(PageRequest {
                        key: iterator.distant_iter.key.clone().unwrap(),
                        offset: 0,
                        limit: DISTANT_LIMIT,
                        count_total: false,
                        reverse: iterator.distant_iter.reverse,
                    }),
                ))
                .unwrap_or_default();

            // We make sure the data queried correspond to all the keys we need
            iterator
                .distant_iter
                .data
                .extend(new_keys.models.into_iter().filter(|m| {
                    let lower_than_end = if let Some(end) = iterator.distant_iter.end.clone() {
                        !gte(m.key.clone(), end)
                    } else {
                        true
                    };

                    let higher_than_start = if let Some(start) = iterator.distant_iter.start.clone()
                    {
                        gte(m.key.clone(), start)
                    } else {
                        true
                    };

                    lower_than_end && higher_than_start
                }));
            iterator.distant_iter.key = new_keys.pagination.map(|p| p.next_key);
        }

        // 2. We find the first key in order between distant and local storage
        let next_local = self.local_storage.peak(iterator.local_iter).unwrap();
        let next_distant = iterator
            .distant_iter
            .data
            .get(iterator.distant_iter.position);

        // We select the distant storage only if the keys are valid (inside the start, end range)
        let is_valid_distant_key = next_distant.is_some()
            && (iterator.distant_iter.end.is_none() || // If there is end no bound 
					iterator.distant_iter.reverse || // if the iterator is reversed, 
					!gte(next_distant.unwrap().key.clone(), iterator.distant_iter.end.clone().unwrap()))
            && (iterator.distant_iter.start.is_none()
                || !iterator.distant_iter.reverse
                || gte(
                    next_distant.unwrap().key.clone(),
                    iterator.distant_iter.start.clone().unwrap(),
                ));

        let key_value = if let Some(local) = next_local {
            if is_valid_distant_key {
                let distant = next_distant.unwrap();
                // We compare the two keys with the order and return the higher key
                let key_local = BigInt::from_bytes_be(Sign::Plus, &local.0);
                let key_distant = BigInt::from_bytes_be(Sign::Plus, &distant.key);
                if (key_local < key_distant) == iterator.distant_iter.reverse {
                    iterator.distant_iter.position += 1;
                    Some((distant.key.clone(), distant.value.clone()))
                } else {
                    self.local_storage.next(iterator.local_iter).0.unwrap()
                }
            } else {
                self.local_storage.next(iterator.local_iter).0.unwrap()
            }
        } else if is_valid_distant_key {
            let distant = next_distant.unwrap();
            iterator.distant_iter.position += 1;
            Some((distant.key.clone(), distant.value.clone()))
        } else {
            None
        };

        // We add the gas cost
        if let Some((key, value)) = key_value {
            (
                Ok(Some((key.clone(), value.clone()))),
                GasInfo::with_externally_used((key.len() + value.len()) as u64),
            )
        } else {
            (
                Ok(None),
                GasInfo::with_externally_used(GAS_COST_LAST_ITERATION),
            )
        }
    }

    fn set(&mut self, key: &[u8], value: &[u8]) -> BackendResult<()> {
        self.removed_keys.remove(key); // It's not locally removed anymore, because we set it locally
        self.local_storage.set(key, value)
    }

    fn remove(&mut self, key: &[u8]) -> BackendResult<()> {
        self.removed_keys.insert(key.to_vec()); // We indicate locally if it's removed. So that we can remove keys and not query them on the distant chain
        self.local_storage.remove(key)
    }
}
