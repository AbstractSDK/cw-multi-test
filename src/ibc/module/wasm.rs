use anyhow::{bail, Result as AnyResult};
use cosmwasm_std::{
    Addr, Api, BlockInfo, CustomMsg, CustomQuery, Event, IbcBasicResponse, IbcChannelCloseMsg,
    IbcChannelConnectMsg, IbcChannelOpenMsg, IbcChannelOpenResponse, IbcPacketAckMsg,
    IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, Response, Storage,
};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::{
    error::Error,
    ibc::types::{AppIbcBasicResponse, AppIbcReceiveResponse},
    CosmosRouter, WasmKeeper,
};

#[allow(missing_docs)]
pub trait IbcWasm<ExecC, QueryC> {
    fn ibc_channel_open(
        &self,
        _api: &dyn cosmwasm_std::Api,
        _contract: Addr,
        _storage: &mut dyn cosmwasm_std::Storage,
        _router: &dyn crate::CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &cosmwasm_std::BlockInfo,
        _request: cosmwasm_std::IbcChannelOpenMsg,
    ) -> anyhow::Result<cosmwasm_std::IbcChannelOpenResponse> {
        Ok(cosmwasm_std::IbcChannelOpenResponse::None)
    }

    fn ibc_channel_connect(
        &self,
        _api: &dyn cosmwasm_std::Api,
        _contract: Addr,
        _storage: &mut dyn cosmwasm_std::Storage,
        _router: &dyn crate::CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &cosmwasm_std::BlockInfo,
        _request: cosmwasm_std::IbcChannelConnectMsg,
    ) -> anyhow::Result<crate::ibc::types::AppIbcBasicResponse> {
        Ok(crate::ibc::types::AppIbcBasicResponse::default())
    }

    fn ibc_channel_close(
        &self,
        _api: &dyn cosmwasm_std::Api,
        _contract: Addr,
        _storage: &mut dyn cosmwasm_std::Storage,
        _router: &dyn crate::CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &cosmwasm_std::BlockInfo,
        _request: cosmwasm_std::IbcChannelCloseMsg,
    ) -> anyhow::Result<crate::ibc::types::AppIbcBasicResponse> {
        Ok(crate::ibc::types::AppIbcBasicResponse::default())
    }

    fn ibc_packet_receive(
        &self,
        _api: &dyn cosmwasm_std::Api,
        _contract: Addr,
        _storage: &mut dyn cosmwasm_std::Storage,
        _router: &dyn crate::CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &cosmwasm_std::BlockInfo,
        _request: cosmwasm_std::IbcPacketReceiveMsg,
    ) -> anyhow::Result<crate::ibc::types::AppIbcReceiveResponse> {
        panic!("No ibc packet receive implemented");
    }

    fn ibc_packet_acknowledge(
        &self,
        _api: &dyn cosmwasm_std::Api,
        _contract: Addr,
        _storage: &mut dyn cosmwasm_std::Storage,
        _router: &dyn crate::CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &cosmwasm_std::BlockInfo,
        _request: cosmwasm_std::IbcPacketAckMsg,
    ) -> anyhow::Result<crate::ibc::types::AppIbcBasicResponse> {
        panic!("No ibc packet acknowledgement implemented");
    }

    fn ibc_packet_timeout(
        &self,
        _api: &dyn cosmwasm_std::Api,
        _contract: Addr,
        _storage: &mut dyn cosmwasm_std::Storage,
        _router: &dyn crate::CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &cosmwasm_std::BlockInfo,
        _request: cosmwasm_std::IbcPacketTimeoutMsg,
    ) -> anyhow::Result<crate::ibc::types::AppIbcBasicResponse> {
        panic!("No ibc packet timeout implemented");
    }

    fn process_ibc_response(
        &self,
        api: &dyn Api,
        contract: Addr,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        res: IbcBasicResponse<ExecC>,
    ) -> AnyResult<AppIbcBasicResponse>;

    fn process_ibc_receive_response(
        &self,
        api: &dyn Api,
        contract: Addr,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        original_res: IbcReceiveResponse<ExecC>,
    ) -> AnyResult<AppIbcReceiveResponse>;

    fn verify_ibc_response<T>(response: IbcBasicResponse<T>) -> AnyResult<IbcBasicResponse<T>>
    where
        T: Clone + std::fmt::Debug + PartialEq + JsonSchema;

    fn verify_packet_response<T>(
        response: IbcReceiveResponse<T>,
    ) -> AnyResult<IbcReceiveResponse<T>>
    where
        T: Clone + std::fmt::Debug + PartialEq + JsonSchema;
}

impl<ExecC, QueryC> IbcWasm<ExecC, QueryC> for WasmKeeper<ExecC, QueryC>
where
    ExecC: CustomMsg + DeserializeOwned + 'static,
    QueryC: CustomQuery + DeserializeOwned + 'static,
{
    // The following ibc endpoints can only be used by the ibc module.
    // For channels
    fn ibc_channel_open(
        &self,
        api: &dyn Api,
        contract: Addr,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        request: IbcChannelOpenMsg,
    ) -> AnyResult<IbcChannelOpenResponse> {
        // For channel open, we simply return the result directly to the ibc module
        let contract_response = self.with_storage(
            api,
            storage,
            router,
            block,
            contract.clone(),
            |contract, deps, env| contract.ibc_channel_open(deps, env, request),
        )?;

        Ok(contract_response)
    }

    fn ibc_channel_connect(
        &self,
        api: &dyn Api,
        contract_addr: Addr,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        request: IbcChannelConnectMsg,
    ) -> AnyResult<AppIbcBasicResponse> {
        let res = Self::verify_ibc_response(self.with_storage(
            api,
            storage,
            router,
            block,
            contract_addr.clone(),
            |contract, deps, env| contract.ibc_channel_connect(deps, env, request),
        )?)?;

        self.process_ibc_response(api, contract_addr, storage, router, block, res)
    }
    fn ibc_channel_close(
        &self,
        api: &dyn Api,
        contract_addr: Addr,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        request: IbcChannelCloseMsg,
    ) -> AnyResult<AppIbcBasicResponse> {
        let res = Self::verify_ibc_response(self.with_storage(
            api,
            storage,
            router,
            block,
            contract_addr.clone(),
            |contract, deps, env| contract.ibc_channel_close(deps, env, request),
        )?)?;

        self.process_ibc_response(api, contract_addr, storage, router, block, res)
    }

    fn ibc_packet_receive(
        &self,
        api: &dyn Api,
        contract_addr: Addr,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        request: IbcPacketReceiveMsg,
    ) -> AnyResult<AppIbcReceiveResponse> {
        let res = Self::verify_packet_response(self.with_storage(
            api,
            storage,
            router,
            block,
            contract_addr.clone(),
            |contract, deps, env| contract.ibc_packet_receive(deps, env, request),
        )?)?;

        self.process_ibc_receive_response(api, contract_addr, storage, router, block, res)
    }

    fn ibc_packet_acknowledge(
        &self,
        api: &dyn Api,
        contract_addr: Addr,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        request: IbcPacketAckMsg,
    ) -> AnyResult<AppIbcBasicResponse> {
        let res = Self::verify_ibc_response(self.with_storage(
            api,
            storage,
            router,
            block,
            contract_addr.clone(),
            |contract, deps, env| contract.ibc_packet_acknowledge(deps, env, request),
        )?)?;

        self.process_ibc_response(api, contract_addr, storage, router, block, res)
    }

    fn ibc_packet_timeout(
        &self,
        api: &dyn Api,
        contract_addr: Addr,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        request: IbcPacketTimeoutMsg,
    ) -> AnyResult<AppIbcBasicResponse> {
        let res = Self::verify_ibc_response(self.with_storage(
            api,
            storage,
            router,
            block,
            contract_addr.clone(),
            |contract, deps, env| contract.ibc_packet_timeout(deps, env, request),
        )?)?;

        self.process_ibc_response(api, contract_addr, storage, router, block, res)
    }

    fn process_ibc_response(
        &self,
        api: &dyn Api,
        contract: Addr,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        res: IbcBasicResponse<ExecC>,
    ) -> AnyResult<AppIbcBasicResponse> {
        // We format the events correctly because we are executing wasm
        let contract_response = Response::new()
            .add_submessages(res.messages)
            .add_attributes(res.attributes)
            .add_events(res.events);

        let (res, msgs) = self.build_app_response(&contract, Event::new("ibc"), contract_response);

        // We process eventual messages that were sent out with the response
        let res = self.process_response(api, router, storage, block, contract, res, msgs)?;

        // We transfer back to an IbcBasicResponse
        Ok(AppIbcBasicResponse { events: res.events })
    }

    fn process_ibc_receive_response(
        &self,
        api: &dyn Api,
        contract: Addr,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        original_res: IbcReceiveResponse<ExecC>,
    ) -> AnyResult<AppIbcReceiveResponse> {
        // We format the events correctly because we are executing wasm
        let contract_response = Response::new()
            .add_submessages(original_res.messages)
            .add_attributes(original_res.attributes)
            .add_events(original_res.events);

        let (res, msgs) = self.build_app_response(&contract, Event::new("ibc"), contract_response);

        // We process eventual messages that were sent out with the response
        let res = self.process_response(api, router, storage, block, contract, res, msgs)?;

        // If the data field was overwritten by the response propagation, we replace the ibc ack
        let ack = if let Some(new_ack) = res.data {
            new_ack
        } else {
            original_res.acknowledgement
        };

        // We transfer back to an IbcBasicResponse
        Ok(AppIbcReceiveResponse {
            events: res.events,
            acknowledgement: ack,
        })
    }

    fn verify_ibc_response<T>(response: IbcBasicResponse<T>) -> AnyResult<IbcBasicResponse<T>>
    where
        T: Clone + std::fmt::Debug + PartialEq + JsonSchema,
    {
        Self::verify_attributes(&response.attributes)?;

        for event in &response.events {
            Self::verify_attributes(&event.attributes)?;
            let ty = event.ty.trim();
            if ty.len() < 2 {
                bail!(Error::event_type_too_short(ty));
            }
        }

        Ok(response)
    }

    fn verify_packet_response<T>(
        response: IbcReceiveResponse<T>,
    ) -> AnyResult<IbcReceiveResponse<T>>
    where
        T: Clone + std::fmt::Debug + PartialEq + JsonSchema,
    {
        Self::verify_attributes(&response.attributes)?;

        for event in &response.events {
            Self::verify_attributes(&event.attributes)?;
            let ty = event.ty.trim();
            if ty.len() < 2 {
                bail!(Error::event_type_too_short(ty));
            }
        }

        Ok(response)
    }
}
