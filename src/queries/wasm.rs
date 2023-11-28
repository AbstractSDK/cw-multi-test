use std::collections::HashMap;

use anyhow::Result as AnyResult;
use cosmwasm_std::{Addr, CodeInfoResponse, Order, Storage, Binary};
use cw_orch_daemon::queriers::{CosmWasm, DaemonQuerier};

use crate::{
    prefixed_storage::prefixed_read,
    wasm::{ContractData, CONTRACTS, NAMESPACE_WASM},
    wasm_emulation::{channel::RemoteChannel, input::WasmStorage, query::AllQuerier},
    WasmKeeper,
};

pub struct WasmRemoteQuerier;

impl WasmRemoteQuerier {
    pub fn code_info(remote: RemoteChannel, code_id: u64) -> AnyResult<CodeInfoResponse> {
        let wasm_querier = CosmWasm::new(remote.channel);

        let code_info = remote.rt.block_on(wasm_querier.code(code_id))?;
        let mut res = cosmwasm_std::CodeInfoResponse::default();
        res.code_id = code_id;
        res.creator = code_info.creator.to_string();
        res.checksum = code_info.data_hash.into();
        Ok(res)
    }

    pub fn load_distant_contract(remote: RemoteChannel, address: &Addr) -> AnyResult<ContractData> {
        let wasm_querier = CosmWasm::new(remote.channel);

        let code_info = remote
            .rt
            .block_on(wasm_querier.contract_info(address.clone()))?;

        Ok(ContractData {
            admin: {
                match code_info.admin.as_str() {
                    "" => None,
                    a => Some(Addr::unchecked(a)),
                }
            },
            code_id: code_info.code_id,
            created: code_info.created.unwrap().block_height,
            creator: Addr::unchecked(code_info.creator),
            label: code_info.label,
        })
    }

    pub fn raw_query(
        remote: RemoteChannel,
        contract_addr: String,
        key: Binary,
    ) -> AnyResult<Vec<u8>> {
        let wasm_querier = CosmWasm::new(remote.channel);
        let query_result = remote
            .rt
            .block_on(wasm_querier.contract_raw_state(contract_addr, key.to_vec()))
            .map(|query_result| query_result.data);
        Ok(query_result?)
    }
}

impl<ExecC, QueryC> AllQuerier for WasmKeeper<ExecC, QueryC> {
    type Output = WasmStorage;
    fn query_all(&self, storage: &dyn Storage) -> AnyResult<WasmStorage> {
        let all_local_state: Vec<_> = storage.range(None, None, Order::Ascending).collect();

        let contracts = CONTRACTS
            .range(
                &prefixed_read(storage, NAMESPACE_WASM),
                None,
                None,
                Order::Ascending,
            )
            .map(|res| match res {
                Ok((key, value)) => Ok((key.to_string(), value)),
                Err(e) => Err(e),
            })
            .collect::<Result<HashMap<_, _>, _>>()?;

        Ok(WasmStorage {
            contracts,
            storage: all_local_state,
            codes: self.code_base.clone(),
            code_data: self.code_data.clone(),
        })
    }
}
