//! Definition and implementation of the IbcModule trait for modules that support IBC
mod bank;
mod staking;
mod wasm;

pub use bank::{optional_unwrap_ibc_denom, IBC_LOCK_MODULE_ADDRESS};
pub use wasm::IbcWasm;

use crate::app::CosmosRouter;
use crate::error::AnyResult;
use cosmwasm_std::{Api, BlockInfo, Storage};

use crate::ibc::types::{AppIbcBasicResponse, AppIbcReceiveResponse};
use cosmwasm_std::{
    IbcChannelCloseMsg, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcChannelOpenResponse,
    IbcPacketAckMsg, IbcPacketReceiveMsg, IbcPacketTimeoutMsg,
};

/// Allows a module to execute IBC actions
pub trait IbcModule {
    /// Executes the contract ibc_channel_open endpoint
    fn ibc_channel_open<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _request: IbcChannelOpenMsg,
    ) -> AnyResult<IbcChannelOpenResponse> {
        Ok(IbcChannelOpenResponse::None)
    }

    /// Executes the contract ibc_channel_connect endpoint
    fn ibc_channel_connect<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _request: IbcChannelConnectMsg,
    ) -> AnyResult<AppIbcBasicResponse> {
        Ok(AppIbcBasicResponse::default())
    }

    /// Executes the contract ibc_channel_close endpoints
    fn ibc_channel_close<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _request: IbcChannelCloseMsg,
    ) -> AnyResult<AppIbcBasicResponse> {
        Ok(AppIbcBasicResponse::default())
    }

    /// Executes the contract ibc_packet_receive endpoint
    fn ibc_packet_receive<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _request: IbcPacketReceiveMsg,
    ) -> AnyResult<AppIbcReceiveResponse> {
        panic!("No ibc packet receive implemented");
    }

    /// Executes the contract ibc_packet_acknowledge endpoint
    fn ibc_packet_acknowledge<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _request: IbcPacketAckMsg,
    ) -> AnyResult<AppIbcBasicResponse> {
        panic!("No ibc packet acknowledgement implemented");
    }

    /// Executes the contract ibc_packet_timeout endpoint
    fn ibc_packet_timeout<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _request: IbcPacketTimeoutMsg,
    ) -> AnyResult<AppIbcBasicResponse> {
        panic!("No ibc packet timeout implemented");
    }
}
