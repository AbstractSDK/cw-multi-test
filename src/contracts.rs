use schemars::JsonSchema;

use cosmwasm_std::{Binary, CustomQuery, Deps, DepsMut, Empty, Env, MessageInfo, Reply, Response};

use anyhow::Result as AnyResult;

use crate::wasm_emulation::channel::RemoteChannel;

/// Interface to call into a [Contract].
pub trait Contract<T, Q = Empty>
where
    T: Clone + std::fmt::Debug + PartialEq + JsonSchema,
    Q: CustomQuery,
{
    fn execute(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        info: MessageInfo,
        msg: Vec<u8>,
        remote: RemoteChannel,
    ) -> AnyResult<Response<T>>;

    fn instantiate(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        info: MessageInfo,
        msg: Vec<u8>,
        remote: RemoteChannel,
    ) -> AnyResult<Response<T>>;

    fn query(
        &self,
        deps: Deps<Q>,
        env: Env,
        msg: Vec<u8>,
        remote: RemoteChannel,
    ) -> AnyResult<Binary>;

    fn sudo(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: Vec<u8>,
        remote: RemoteChannel,
    ) -> AnyResult<Response<T>>;

    fn reply(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: Reply,
        remote: RemoteChannel,
    ) -> AnyResult<Response<T>>;

    fn migrate(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: Vec<u8>,
        remote: RemoteChannel,
    ) -> AnyResult<Response<T>>;
}
