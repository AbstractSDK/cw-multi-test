//! IBC implementation of the CosmosRouter

use crate::app::MockRouter;
use crate::Router;
use crate::{Bank, CosmosRouter, Distribution, Gov, Ibc, Module, Staking, Stargate, Wasm};

use super::module::{IbcModule, IbcWasm};
use super::relayer::IbcModuleMsg;
use super::types::{IbcModuleId, IbcResponse, IbcRouterMsg};

use anyhow::Result as AnyResult;
use cosmwasm_std::{Api, BlockInfo, CustomMsg, CustomQuery, Storage};
use serde::de::DeserializeOwned;

/// Adds ibc capabilities to the router
pub trait CosmosIbcRouter: CosmosRouter {
    /// Evaluates all ibc related actions
    fn ibc(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        block: &BlockInfo,
        msg: IbcRouterMsg,
    ) -> AnyResult<IbcResponse>;
}

impl<BankT, CustomT, WasmT, StakingT, DistrT, IbcT, GovT, StargateT> CosmosIbcRouter
    for Router<BankT, CustomT, WasmT, StakingT, DistrT, IbcT, GovT, StargateT>
where
    CustomT::ExecT: CustomMsg + DeserializeOwned + 'static,
    CustomT::QueryT: CustomQuery + DeserializeOwned + 'static,
    CustomT: Module,
    WasmT: Wasm<CustomT::ExecT, CustomT::QueryT> + IbcWasm<CustomT::ExecT, CustomT::QueryT>,
    BankT: Bank + IbcModule,
    StakingT: Staking + IbcModule,
    DistrT: Distribution,
    IbcT: Ibc,
    GovT: Gov,
    StargateT: Stargate,
{
    fn ibc(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        block: &BlockInfo,
        msg: IbcRouterMsg,
    ) -> AnyResult<IbcResponse> {
        match msg.module {
            IbcModuleId::Bank => match msg.msg {
                IbcModuleMsg::ChannelOpen(m) => self
                    .bank
                    .ibc_channel_open(api, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::ChannelConnect(m) => self
                    .bank
                    .ibc_channel_connect(api, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::ChannelClose(m) => self
                    .bank
                    .ibc_channel_close(api, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::PacketReceive(m) => self
                    .bank
                    .ibc_packet_receive(api, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::PacketAcknowledgement(m) => self
                    .bank
                    .ibc_packet_acknowledge(api, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::PacketTimeout(m) => self
                    .bank
                    .ibc_packet_timeout(api, storage, self, block, m)
                    .map(Into::into),
            },
            IbcModuleId::Staking => match msg.msg {
                IbcModuleMsg::ChannelOpen(m) => self
                    .staking
                    .ibc_channel_open(api, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::ChannelConnect(m) => self
                    .staking
                    .ibc_channel_connect(api, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::ChannelClose(m) => self
                    .staking
                    .ibc_channel_close(api, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::PacketReceive(m) => self
                    .staking
                    .ibc_packet_receive(api, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::PacketAcknowledgement(m) => self
                    .staking
                    .ibc_packet_acknowledge(api, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::PacketTimeout(m) => self
                    .staking
                    .ibc_packet_timeout(api, storage, self, block, m)
                    .map(Into::into),
            },
            IbcModuleId::Wasm(contract_addr) => match msg.msg {
                IbcModuleMsg::ChannelOpen(m) => self
                    .wasm
                    .ibc_channel_open(api, contract_addr, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::ChannelConnect(m) => self
                    .wasm
                    .ibc_channel_connect(api, contract_addr, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::ChannelClose(m) => self
                    .wasm
                    .ibc_channel_close(api, contract_addr, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::PacketReceive(m) => self
                    .wasm
                    .ibc_packet_receive(api, contract_addr, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::PacketAcknowledgement(m) => self
                    .wasm
                    .ibc_packet_acknowledge(api, contract_addr, storage, self, block, m)
                    .map(Into::into),
                IbcModuleMsg::PacketTimeout(m) => self
                    .wasm
                    .ibc_packet_timeout(api, contract_addr, storage, self, block, m)
                    .map(Into::into),
            },
        }
    }
}

impl<ExecC, QueryC> CosmosIbcRouter for MockRouter<ExecC, QueryC>
where
    ExecC: CustomMsg,
    QueryC: CustomQuery,
{
    fn ibc(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _block: &BlockInfo,
        _msg: IbcRouterMsg,
    ) -> AnyResult<IbcResponse> {
        panic!("Cannot ibc MockRouters");
    }
}
