use crate::prefixed_storage::PrefixedStorage;
use crate::wasm_emulation::channel::RemoteChannel;
use crate::wasm_emulation::storage::mock_storage::{GAS_COST_LAST_ITERATION, GAS_COST_RANGE};

use super::mock_storage::MockStorage;
use cosmrs::proto::cosmos::base::query::v1beta1::PageRequest;
use cosmrs::proto::cosmwasm::wasm::v1::Model;
use cosmwasm_std::Record;
use cosmwasm_std::{Order, Storage};
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

#[derive(Default, Debug)]
struct DistantIter {
    data: Vec<Model>,
    position: usize,
    key: Option<Vec<u8>>, // if set to None, there is no more keys to investigate in the distant container
    start: Option<Vec<u8>>,
    end: Option<Vec<u8>>,
    reverse: bool,
}

/// Iterator to get multiple keys
#[derive(Default, Debug)]
struct Iter {
    distant_iter: DistantIter,
    local_iter: u32,
}

pub struct DualStorage<'a> {
    pub local_storage: Box<dyn Storage + 'a>,
    pub removed_keys: HashSet<Vec<u8>>,
    pub remote: RemoteChannel,
    pub contract_addr: String,
    iterators: HashMap<u32, Iter>,
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
            iterators: HashMap::new(),
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
        todo!()
    }
}
