use crate::app::CosmosRouter;
use crate::error::{bail, AnyResult};
use crate::AppResponse;
use cosmwasm_std::{Addr, Api, Binary, BlockInfo, CustomQuery, Querier, Storage};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::marker::PhantomData;

/// Module interface.
pub trait Module {
    type ExecT;
    type QueryT;
    type SudoT;

    /// Runs any [ExecT](Self::ExecT) message,
    /// which can be called by any external actor or smart contract.
    fn execute<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: Addr,
        msg: Self::ExecT,
    ) -> AnyResult<AppResponse>
    where
        ExecC: Debug + Clone + PartialEq + JsonSchema + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static;

    /// Runs any [QueryT](Self::QueryT) message,
    /// which can be called by any external actor or smart contract.
    fn query(
        &self,
        api: &dyn Api,
        storage: &dyn Storage,
        querier: &dyn Querier,
        block: &BlockInfo,
        request: Self::QueryT,
    ) -> AnyResult<Binary>;

    /// Runs privileged actions, like minting tokens, or governance proposals.
    /// This allows modules to have full access to these privileged actions,
    /// that cannot be triggered by smart contracts.
    ///
    /// There is no sender, as this must be previously authorized before calling.
    fn sudo<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        msg: Self::SudoT,
    ) -> AnyResult<AppResponse>
    where
        ExecC: Debug + Clone + PartialEq + JsonSchema + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static;
}

pub struct FailingModule<ExecT, QueryT, SudoT> {
    module_type: String,
    _t: PhantomData<(ExecT, QueryT, SudoT)>,
}

impl<ExecT, QueryT, SudoT> FailingModule<ExecT, QueryT, SudoT> {
    pub fn new(module_type: &str) -> Self {
        Self {
            module_type: module_type.to_string(),
            _t: PhantomData,
        }
    }
}

impl<ExecT, QueryT, SudoT> Module for FailingModule<ExecT, QueryT, SudoT>
where
    ExecT: Debug,
    QueryT: Debug,
    SudoT: Debug,
{
    type ExecT = ExecT;
    type QueryT = QueryT;
    type SudoT = SudoT;

    /// Runs any [ExecT](Self::ExecT) message, always returns an error.
    fn execute<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        sender: Addr,
        msg: Self::ExecT,
    ) -> AnyResult<AppResponse> {
        bail!(
            "Unexpected exec msg {:?} from {:?} on module {}",
            msg,
            sender,
            self.module_type
        )
    }

    /// Runs any [QueryT](Self::QueryT) message, always returns an error.
    fn query(
        &self,
        _api: &dyn Api,
        _storage: &dyn Storage,
        _querier: &dyn Querier,
        _block: &BlockInfo,
        request: Self::QueryT,
    ) -> AnyResult<Binary> {
        bail!(
            "Unexpected custom query {:?} on module {}",
            request,
            self.module_type
        )
    }

    /// Runs any [SudoT](Self::SudoT) privileged action, always returns an error.
    fn sudo<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        msg: Self::SudoT,
    ) -> AnyResult<AppResponse> {
        bail!(
            "Unexpected sudo msg {:?} on module {}",
            msg,
            self.module_type
        )
    }
}

pub struct AcceptingModule<ExecT, QueryT, SudoT>(PhantomData<(ExecT, QueryT, SudoT)>);

impl<ExecT, QueryT, SudoT> AcceptingModule<ExecT, QueryT, SudoT> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<ExecT, QueryT, SudoT> Default for AcceptingModule<ExecT, QueryT, SudoT> {
    fn default() -> Self {
        Self::new()
    }
}

impl<ExecT, QueryT, SudoT> Module for AcceptingModule<ExecT, QueryT, SudoT>
where
    ExecT: Debug,
    QueryT: Debug,
    SudoT: Debug,
{
    type ExecT = ExecT;
    type QueryT = QueryT;
    type SudoT = SudoT;

    /// Runs any [ExecT](Self::ExecT) message, always returns a default response.
    fn execute<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _sender: Addr,
        _msg: Self::ExecT,
    ) -> AnyResult<AppResponse> {
        Ok(AppResponse::default())
    }

    /// Runs any [QueryT](Self::QueryT) message, always returns an empty binary.
    fn query(
        &self,
        _api: &dyn Api,
        _storage: &dyn Storage,
        _querier: &dyn Querier,
        _block: &BlockInfo,
        _request: Self::QueryT,
    ) -> AnyResult<Binary> {
        Ok(Binary::default())
    }

    /// Runs any [SudoT](Self::SudoT) privileged action, always returns a default response.
    fn sudo<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _msg: Self::SudoT,
    ) -> AnyResult<AppResponse> {
        Ok(AppResponse::default())
    }
}
