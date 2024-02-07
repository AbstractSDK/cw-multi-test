use std::fmt;

use anyhow::Result as AnyResult;
use cosmwasm_std::{from_json, Api, Binary, CustomQuery, Storage};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::{
    ibc::{
        events::WRITE_ACK_EVENT,
        types::{IbcPacketData, MockIbcQuery},
        IbcPacketRelayingMsg,
    },
    App, AppResponse, Bank, Distribution, Gov, Ibc, Module, Staking, SudoMsg, Wasm,
};

use super::get_event_attr_value;

/// Timeouts the relay on the sending chain without broadcasting anything to the receiving chain
/// TODO : Should this close the channel ?
pub fn timeout_packet<
    BankT1,
    ApiT1,
    StorageT1,
    CustomT1,
    WasmT1,
    StakingT1,
    DistrT1,
    IbcT1,
    GovT1,
    BankT2,
    ApiT2,
    StorageT2,
    CustomT2,
    WasmT2,
    StakingT2,
    DistrT2,
    IbcT2,
    GovT2,
>(
    app1: &mut App<BankT1, ApiT1, StorageT1, CustomT1, WasmT1, StakingT1, DistrT1, IbcT1, GovT1>,
    _app2: &mut App<BankT2, ApiT2, StorageT2, CustomT2, WasmT2, StakingT2, DistrT2, IbcT2, GovT2>,
    src_port_id: String,
    src_channel_id: String,
    sequence: u64,
) -> AnyResult<AppResponse>
where
    CustomT1::ExecT: Clone + fmt::Debug + PartialEq + JsonSchema + DeserializeOwned + 'static,
    CustomT1::QueryT: CustomQuery + DeserializeOwned + 'static,
    WasmT1: Wasm<CustomT1::ExecT, CustomT1::QueryT>,
    BankT1: Bank,
    ApiT1: Api,
    StorageT1: Storage,
    CustomT1: Module,
    StakingT1: Staking,
    DistrT1: Distribution,
    IbcT1: Ibc,
    GovT1: Gov,

    CustomT2::ExecT: Clone + fmt::Debug + PartialEq + JsonSchema + DeserializeOwned + 'static,
    CustomT2::QueryT: CustomQuery + DeserializeOwned + 'static,
    WasmT2: Wasm<CustomT2::ExecT, CustomT2::QueryT>,
    BankT2: Bank,
    ApiT2: Api,
    StorageT2: Storage,
    CustomT2: Module,
    StakingT2: Staking,
    DistrT2: Distribution,
    IbcT2: Ibc,
    GovT2: Gov,
{
    let packet: IbcPacketData = from_json(app1.ibc_query(MockIbcQuery::SendPacket {
        channel_id: src_channel_id.clone(),
        port_id: src_port_id.clone(),
        sequence,
    })?)?;

    // Then we query the packet ack to deliver the response on chain 1
    let timeout_response = app1.sudo(SudoMsg::Ibc(IbcPacketRelayingMsg::Timeout { packet }))?;

    Ok(timeout_response)
}

/// Receive the packet on the remote chain and timeouts the packet on the sending chain without broadcasting anything to the receiving chain
/// TODO : Should this close the channel ?
pub fn receive_and_timeout_packet<
    BankT1,
    ApiT1,
    StorageT1,
    CustomT1,
    WasmT1,
    StakingT1,
    DistrT1,
    IbcT1,
    GovT1,
    BankT2,
    ApiT2,
    StorageT2,
    CustomT2,
    WasmT2,
    StakingT2,
    DistrT2,
    IbcT2,
    GovT2,
>(
    app1: &mut App<BankT1, ApiT1, StorageT1, CustomT1, WasmT1, StakingT1, DistrT1, IbcT1, GovT1>,
    app2: &mut App<BankT2, ApiT2, StorageT2, CustomT2, WasmT2, StakingT2, DistrT2, IbcT2, GovT2>,
    src_port_id: String,
    src_channel_id: String,
    sequence: u64,
) -> AnyResult<(AppResponse, AppResponse, Binary)>
where
    CustomT1::ExecT: Clone + fmt::Debug + PartialEq + JsonSchema + DeserializeOwned + 'static,
    CustomT1::QueryT: CustomQuery + DeserializeOwned + 'static,
    WasmT1: Wasm<CustomT1::ExecT, CustomT1::QueryT>,
    BankT1: Bank,
    ApiT1: Api,
    StorageT1: Storage,
    CustomT1: Module,
    StakingT1: Staking,
    DistrT1: Distribution,
    IbcT1: Ibc,
    GovT1: Gov,

    CustomT2::ExecT: Clone + fmt::Debug + PartialEq + JsonSchema + DeserializeOwned + 'static,
    CustomT2::QueryT: CustomQuery + DeserializeOwned + 'static,
    WasmT2: Wasm<CustomT2::ExecT, CustomT2::QueryT>,
    BankT2: Bank,
    ApiT2: Api,
    StorageT2: Storage,
    CustomT2: Module,
    StakingT2: Staking,
    DistrT2: Distribution,
    IbcT2: Ibc,
    GovT2: Gov,
{
    let packet: IbcPacketData = from_json(app1.ibc_query(MockIbcQuery::SendPacket {
        channel_id: src_channel_id.clone(),
        port_id: src_port_id.clone(),
        sequence,
    })?)?;

    // First we start by sending the packet on chain 2
    let receive_response = app2.sudo(SudoMsg::Ibc(IbcPacketRelayingMsg::Receive {
        packet: packet.clone(),
    }))?;

    let hex_ack = get_event_attr_value(&receive_response, WRITE_ACK_EVENT, "packet_ack_hex")?;

    let ack = Binary::from(hex::decode(hex_ack)?);

    // Then we query the packet ack to deliver the response on chain 1
    let timeout_response = app1.sudo(SudoMsg::Ibc(IbcPacketRelayingMsg::Timeout { packet }))?;

    Ok((receive_response, timeout_response, ack))
}
