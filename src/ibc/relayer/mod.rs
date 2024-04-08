use anyhow::Result as AnyResult;
use cosmwasm_std::{
    Api, CustomMsg, CustomQuery, IbcPacketReceiveMsg, StdError, StdResult, Storage,
};
use serde::de::DeserializeOwned;

use crate::{
    transactions::transactional, App, AppResponse, Bank, Distribution, Gov, Ibc, Module, Staking,
    Stargate, Wasm,
};

mod channel;
mod packet;

pub use channel::{create_channel, create_connection, ChannelCreationResult};
pub use packet::{relay_packet, relay_packets_in_tx, RelayPacketResult, RelayingResult};

use super::{router::CosmosIbcRouter, IbcPacketRelayingMsg, IbcSimpleModule};

pub fn get_event_attr_value(
    response: &AppResponse,
    event_type: &str,
    attr_key: &str,
) -> StdResult<String> {
    for event in &response.events {
        if event.ty == event_type {
            for attr in &event.attributes {
                if attr.key == attr_key {
                    return Ok(attr.value.clone());
                }
            }
        }
    }

    Err(StdError::generic_err(format!(
        "event of type {event_type} does not have a value at key {attr_key}"
    )))
}

pub fn has_event(response: &AppResponse, event_type: &str) -> bool {
    for event in &response.events {
        if event.ty == event_type {
            return true;
        }
    }
    false
}

pub fn get_all_event_attr_value(
    response: &AppResponse,
    event: &str,
    attribute: &str,
) -> Vec<String> {
    response
        .events
        .iter()
        .filter(|e| e.ty.eq(event))
        .flat_map(|e| {
            e.attributes
                .iter()
                .filter(|a| a.key.eq(attribute))
                .map(|a| a.value.clone())
        })
        .collect()
}

impl<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, GovT, StargateT>
    App<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, IbcSimpleModule, GovT, StargateT>
where
    CustomT::ExecT: CustomMsg + DeserializeOwned + 'static,
    CustomT::QueryT: CustomQuery + DeserializeOwned + 'static,
    WasmT: Wasm<CustomT::ExecT, CustomT::QueryT>,
    BankT: Bank,
    ApiT: Api,
    StorageT: Storage,
    CustomT: Module,
    StakingT: Staking,
    DistrT: Distribution,
    GovT: Gov,
    StargateT: Stargate,
{
    // Send any msg
    pub(crate) fn relay(&mut self, msg: IbcPacketRelayingMsg) -> AnyResult<AppResponse> {
        let block_info = &self.block_info();

        transactional(self.storage_mut(), |write_cache, _| {
            // TODO, This also doesn't work because the app is borrowed mutably and immutably too many times
            // The only way it could work is with public cw-multi-test elements.
            self.router
                .ibc
                .relay(&self.api, write_cache, &self.router, block_info, msg)
        })
    }
}
