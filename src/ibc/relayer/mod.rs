//! Utils and traits to support IBC capabilites

use anyhow::Result as AnyResult;
use cosmwasm_std::{
    Api, Binary, CustomMsg, CustomQuery, IbcChannelCloseMsg, IbcChannelConnectMsg,
    IbcChannelOpenMsg, IbcPacketAckMsg, IbcPacketReceiveMsg, IbcPacketTimeoutMsg, StdError,
    StdResult, Storage,
};
use serde::de::DeserializeOwned;

use crate::{
    transactions::transactional, App, AppResponse, Bank, Distribution, Gov, Module, Staking,
    Stargate, Wasm,
};

mod channel;
mod packet;

pub use channel::{create_channel, create_connection, ChannelCreationResult};
pub use packet::{relay_packet, relay_packets_in_tx, RelayPacketResult, RelayingResult};

use super::{
    module::{IbcModule, IbcWasm},
    types::MockIbcQuery,
    IbcPacketRelayingMsg, IbcSimpleModule,
};

/// Gets the attribute value corresponding to an event
/// Used to analyze IBC transactions
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

/// Returns wether the event exists in the response
pub fn has_event(response: &AppResponse, event_type: &str) -> bool {
    for event in &response.events {
        if event.ty == event_type {
            return true;
        }
    }
    false
}

/// Gets all the attribute value for a specific event-attribute pair
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
    WasmT: Wasm<CustomT::ExecT, CustomT::QueryT> + IbcWasm<CustomT::ExecT, CustomT::QueryT>,
    BankT: Bank + IbcModule,
    ApiT: Api,
    StorageT: Storage,
    CustomT: Module,
    StakingT: Staking + IbcModule,
    DistrT: Distribution,
    GovT: Gov,
    StargateT: Stargate,
{
    /// Sends any relaying related message on the IBC module
    pub fn relay(&mut self, msg: IbcPacketRelayingMsg) -> AnyResult<AppResponse> {
        let App {
            router,
            api,
            storage,
            block,
        } = self;

        transactional(storage, |write_cache, _| {
            // TODO, This also doesn't work because the app is borrowed mutably and immutably too many times
            // The only way it could work is with public cw-multi-test elements.
            router.ibc.relay(&*api, write_cache, router, block, msg)
        })
    }

    /// Queries the IBC module
    pub fn ibc_query(&self, query: MockIbcQuery) -> AnyResult<Binary> {
        let Self {
            block,
            router,
            api,
            storage,
        } = self;

        let querier = router.querier(api, storage, block);

        router
            .ibc
            .general_query(api, storage, &querier, block, query)
    }
}

/// This is added for modules to implement actions upon ibc actions.
/// This kind of execution flow is copied from the WASM way of doing things and is not 100% completetely compatible with the IBC standard
/// Those messages should only be called by the Ibc module.
/// For additional Modules, the packet endpoints should be implemented
/// The Channel endpoints are usually not implemented besides storing the channel ids
#[cosmwasm_schema::cw_serde]
pub enum IbcModuleMsg {
    /// Open an IBC Channel (2 first steps)
    ChannelOpen(IbcChannelOpenMsg),
    /// Connect an IBC Channel (2 last steps)
    ChannelConnect(IbcChannelConnectMsg),
    /// Close an IBC Channel
    ChannelClose(IbcChannelCloseMsg),

    /// Receive an IBC Packet
    PacketReceive(IbcPacketReceiveMsg),
    /// Receive an IBC Acknowledgement for a packet
    PacketAcknowledgement(IbcPacketAckMsg),
    /// Receive an IBC Timeout for a packet
    PacketTimeout(IbcPacketTimeoutMsg),
}
