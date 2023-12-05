use crate::wasm_emulation::channel::RemoteChannel;

use cosmrs::proto::cosmos::base::query::v1beta1::PageRequest;
use cosmrs::proto::cosmwasm::wasm::v1::Model;
use cosmwasm_std::Record;
use cosmwasm_std::{Order, Storage};
use cw_orch_daemon::queriers::DaemonQuerier;
use num_bigint::{BigInt, Sign};
use std::iter::{self, Peekable};

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

struct DistantIter {
    remote: RemoteChannel,
    contract_addr: String,
    data: Vec<Model>,
    position: usize,
    key: Option<Vec<u8>>, // if set to None, there is no more keys to investigate in the distant container
    start: Option<Vec<u8>>,
    end: Option<Vec<u8>>,
    reverse: bool,
}

/// Iterator to get multiple keys
struct Iter<'a> {
    distant_iter: DistantIter,
    local_iter: Peekable<Box<dyn Iterator<Item = Record> + 'a>>,
}

impl<'i> Iterator for Iter<'i> {
    type Item = Record;

    fn next(&mut self) -> Option<Self::Item> {
        // 1. We verify that there is enough elements in the distant iterator
        if self.distant_iter.position == self.distant_iter.data.len()
            && self.distant_iter.key.is_some()
            && (self.distant_iter.position == 0
                || !self.distant_iter.key.clone().unwrap().is_empty())
        {
            let wasm_querier = CosmWasm::new(self.distant_iter.remote.channel.clone());
            let new_keys = self
                .distant_iter
                .remote
                .rt
                .block_on(wasm_querier.all_contract_state(
                    self.distant_iter.contract_addr.clone(),
                    Some(PageRequest {
                        key: self.distant_iter.key.clone().unwrap(),
                        offset: 0,
                        limit: DISTANT_LIMIT,
                        count_total: false,
                        reverse: self.distant_iter.reverse,
                    }),
                ))
                .unwrap_or_default();

            // We make sure the data queried correspond to all the keys we need
            self.distant_iter
                .data
                .extend(new_keys.models.into_iter().filter(|m| {
                    let lower_than_end = if let Some(end) = self.distant_iter.end.clone() {
                        !gte(m.key.clone(), end)
                    } else {
                        true
                    };

                    let higher_than_start = if let Some(start) = self.distant_iter.start.clone() {
                        gte(m.key.clone(), start)
                    } else {
                        true
                    };

                    lower_than_end && higher_than_start
                }));
            self.distant_iter.key = new_keys.pagination.map(|p| p.next_key);
        }

        // 2. We find the first key in order between distant and local storage
        let next_local = self.local_iter.peek();
        let next_distant = self.distant_iter.data.get(self.distant_iter.position);

        if let Some(local) = next_local {
            if let Some(distant) = next_distant {
                // We compare the two keys with the order and return the higher key
                let key_local = BigInt::from_bytes_be(Sign::Plus, &local.0);
                let key_distant = BigInt::from_bytes_be(Sign::Plus, &distant.key);
                if (key_local < key_distant) == self.distant_iter.reverse {
                    self.distant_iter.position += 1;
                    Some((distant.key.clone(), distant.value.clone()))
                } else {
                    self.local_iter.next()
                }
            } else {
                self.local_iter.next()
            }
        } else if let Some(distant) = next_distant {
            self.distant_iter.position += 1;
            Some((distant.key.clone(), distant.value.clone()))
        } else {
            None
        }
    }
}

pub struct DualStorage<'a> {
    pub local_storage: Box<dyn Storage + 'a>,
    pub removed_keys: HashSet<Vec<u8>>,
    pub remote: RemoteChannel,
    pub contract_addr: String,
}

impl<'a> DualStorage<'a> {
    pub fn new(
        remote: RemoteChannel,
        contract_addr: String,
        local_storage: Box<dyn Storage + 'a>,
    ) -> AnyResult<DualStorage> {
        Ok(Self {
            local_storage,
            remote,
            removed_keys: HashSet::default(),
            contract_addr,
        })
    }
}

impl<'a> Storage for DualStorage<'a> {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        // First we try to get the value locally
        let mut value = self.local_storage.get(key);
        // If it's not available, we query it online if it was not removed locally
        if !self.removed_keys.contains(key) && value.as_ref().is_none() {
            let wasm_querier = CosmWasm::new(self.remote.channel.clone());

            let distant_result = self.remote.rt.block_on(
                wasm_querier.contract_raw_state(self.contract_addr.clone(), key.to_vec()),
            );

            if let Ok(result) = distant_result {
                if !result.data.is_empty() {
                    value = Some(result.data)
                }
            }
        }
        value
    }

    fn set(&mut self, key: &[u8], value: &[u8]) {
        self.removed_keys.remove(key); // It's not locally removed anymore, because we set it locally
        self.local_storage.set(key, value)
    }

    fn remove(&mut self, key: &[u8]) {
        self.removed_keys.insert(key.to_vec()); // We indicate locally if it's removed. So that we can remove keys and not query them on the distant chain
        self.local_storage.remove(key)
    }

    fn range<'b>(
        &'b self,
        start: Option<&[u8]>,
        end: Option<&[u8]>,
        order: Order,
    ) -> Box<dyn Iterator<Item = Record> + 'b> {
        let order_i32: i32 = order.try_into().unwrap();
        let descending_order: i32 = Order::Descending.try_into().unwrap();

        let querier_start = if order_i32 == descending_order {
            end.map(|s| s.to_vec()).unwrap_or(vec![])
        } else {
            start.map(|s| s.to_vec()).unwrap_or(vec![])
        };

        return Box::new(Iter {
            distant_iter: DistantIter {
                remote: self.remote.clone(),
                contract_addr: self.contract_addr.clone(),
                data: vec![],
                position: 0,
                key: Some(querier_start),
                end: end.map(|e| e.to_vec()),
                start: start.map(|e| e.to_vec()),
                reverse: order_i32 == descending_order,
            },
            local_iter: self.local_storage.range(start, end, order).peekable(),
        });
    }
}
