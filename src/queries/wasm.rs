use std::collections::HashMap;

use anyhow::Result as AnyResult;
use cosmwasm_std::{Addr, Binary, CodeInfoResponse, CustomQuery, Order, Storage};
use cw_orch::daemon::queriers::CosmWasm;

use crate::{
    prefixed_storage::prefixed_read,
    wasm::{ContractData, CONTRACTS, NAMESPACE_WASM},
    wasm_emulation::{channel::RemoteChannel, input::WasmStorage, query::AllWasmQuerier},
    WasmKeeper,
};

pub struct WasmRemoteQuerier;

impl WasmRemoteQuerier {
    pub fn code_info(remote: RemoteChannel, code_id: u64) -> AnyResult<CodeInfoResponse> {
        let wasm_querier = CosmWasm {
            channel: remote.channel,
            rt_handle: Some(remote.rt.clone()),
        };

        let code_info = remote.rt.block_on(wasm_querier._code(code_id))?;
        Ok(code_info)
    }

    pub fn load_distant_contract(remote: RemoteChannel, address: &Addr) -> AnyResult<ContractData> {
        let wasm_querier = CosmWasm {
            channel: remote.channel,
            rt_handle: Some(remote.rt.clone()),
        };

        let code_info = remote
            .rt
            .block_on(wasm_querier._contract_info(address.clone()))?;

        Ok(ContractData {
            admin: code_info.admin.map(Addr::unchecked),
            code_id: code_info.code_id,
            creator: Addr::unchecked(code_info.creator),
        })
    }

    pub fn raw_query(
        remote: RemoteChannel,
        contract_addr: String,
        key: Binary,
    ) -> AnyResult<Vec<u8>> {
        let wasm_querier = CosmWasm {
            channel: remote.channel,
            rt_handle: Some(remote.rt.clone()),
        };
        let query_result = remote
            .rt
            .block_on(wasm_querier._contract_raw_state(contract_addr, key.to_vec()))
            .map(|query_result| query_result.data);
        Ok(query_result?)
    }
}

impl<ExecC, QueryC: CustomQuery> AllWasmQuerier for WasmKeeper<ExecC, QueryC> {
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
