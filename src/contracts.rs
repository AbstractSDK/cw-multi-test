use std::{
    error::Error,
    fmt::{self, Debug, Display},
    ops::Deref,
};

use schemars::JsonSchema;

use cosmwasm_std::{
    from_json, Binary, CosmosMsg, CustomMsg, CustomQuery, Deps, DepsMut, Empty, Env, MessageInfo,
    QuerierWrapper, Reply, Response, StdError, SubMsg,
};

use anyhow::Result as AnyResult;
use serde::de::DeserializeOwned;

use crate::wasm_emulation::{
    query::{mock_querier::ForkState, MockQuerier},
    storage::{
        dual_std_storage::DualStorage,
        storage_wrappers::{ReadonlyStorageWrapper, StorageWrapper},
    },
};
use anyhow::{anyhow, bail};
/// Interface to call into a [Contract].
pub trait Contract<T, Q = Empty>
where
    T: CustomMsg + DeserializeOwned + Clone + std::fmt::Debug + PartialEq + JsonSchema,
    Q: CustomQuery + DeserializeOwned,
{
    fn execute(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        info: MessageInfo,
        msg: Vec<u8>,
        fork_state: ForkState<T, Q>,
    ) -> AnyResult<Response<T>>;

    fn instantiate(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        info: MessageInfo,
        msg: Vec<u8>,
        fork_state: ForkState<T, Q>,
    ) -> AnyResult<Response<T>>;

    fn query(
        &self,
        deps: Deps<Q>,
        env: Env,
        msg: Vec<u8>,
        fork_state: ForkState<T, Q>,
    ) -> AnyResult<Binary>;

    fn sudo(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: Vec<u8>,
        fork_state: ForkState<T, Q>,
    ) -> AnyResult<Response<T>>;

    fn reply(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: Reply,
        fork_state: ForkState<T, Q>,
    ) -> AnyResult<Response<T>>;

    fn migrate(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: Vec<u8>,
        fork_state: ForkState<T, Q>,
    ) -> AnyResult<Response<T>>;
}

type ContractFn<T, C, E, Q> =
    fn(deps: DepsMut<Q>, env: Env, info: MessageInfo, msg: T) -> Result<Response<C>, E>;
type PermissionedFn<T, C, E, Q> = fn(deps: DepsMut<Q>, env: Env, msg: T) -> Result<Response<C>, E>;
type ReplyFn<C, E, Q> = fn(deps: DepsMut<Q>, env: Env, msg: Reply) -> Result<Response<C>, E>;
type QueryFn<T, E, Q> = fn(deps: Deps<Q>, env: Env, msg: T) -> Result<Binary, E>;

type ContractClosure<T, C, E, Q> = fn(DepsMut<Q>, Env, MessageInfo, T) -> Result<Response<C>, E>;
type PermissionedClosure<T, C, E, Q> = fn(DepsMut<Q>, Env, T) -> Result<Response<C>, E>;
type ReplyClosure<C, E, Q> = fn(DepsMut<Q>, Env, Reply) -> Result<Response<C>, E>;
type QueryClosure<T, E, Q> = fn(Deps<Q>, Env, T) -> Result<Binary, E>;

#[derive(Clone, Copy)]
/// Wraps the exported functions from a contract and provides the normalized format
/// Place T4 and E4 at the end, as we just want default placeholders for most contracts that don't have sudo
pub struct ContractWrapper<
    T1,
    T2,
    T3,
    E1,
    E2,
    E3,
    C = Empty,
    Q = Empty,
    T4 = Empty,
    E4 = StdError,
    E5 = StdError,
    T6 = Empty,
    E6 = StdError,
> where
    T1: DeserializeOwned + Debug,
    T2: DeserializeOwned,
    T3: DeserializeOwned,
    T4: DeserializeOwned,
    T6: DeserializeOwned,
    E1: Display + Debug + Send + Sync + 'static,
    E2: Display + Debug + Send + Sync + 'static,
    E3: Display + Debug + Send + Sync + 'static,
    E4: Display + Debug + Send + Sync + 'static,
    E5: Display + Debug + Send + Sync + 'static,
    E6: Display + Debug + Send + Sync + 'static,
    C: Clone + fmt::Debug + PartialEq + JsonSchema,
    Q: CustomQuery + DeserializeOwned + 'static,
{
    execute_fn: ContractClosure<T1, C, E1, Q>,
    instantiate_fn: ContractClosure<T2, C, E2, Q>,
    pub query_fn: QueryClosure<T3, E3, Q>,
    sudo_fn: Option<PermissionedClosure<T4, C, E4, Q>>,
    reply_fn: Option<ReplyClosure<C, E5, Q>>,
    migrate_fn: Option<PermissionedClosure<T6, C, E6, Q>>,
}

impl<T1, T2, T3, E1, E2, E3, C, Q> ContractWrapper<T1, T2, T3, E1, E2, E3, C, Q>
where
    T1: DeserializeOwned + Debug + 'static,
    T2: DeserializeOwned + 'static,
    T3: DeserializeOwned + 'static,
    E1: Display + Debug + Send + Sync + 'static,
    E2: Display + Debug + Send + Sync + 'static,
    E3: Display + Debug + Send + Sync + 'static,
    C: Clone + fmt::Debug + PartialEq + JsonSchema + 'static,
    Q: CustomQuery + DeserializeOwned + 'static,
{
    pub fn new(
        execute_fn: ContractFn<T1, C, E1, Q>,
        instantiate_fn: ContractFn<T2, C, E2, Q>,
        query_fn: QueryFn<T3, E3, Q>,
    ) -> Self {
        Self {
            execute_fn,
            instantiate_fn,
            query_fn,
            sudo_fn: None,
            reply_fn: None,
            migrate_fn: None,
        }
    }
}

impl<T1, T2, T3, E1, E2, E3, C, Q, T4, E4, E5, T6, E6>
    ContractWrapper<T1, T2, T3, E1, E2, E3, C, Q, T4, E4, E5, T6, E6>
where
    T1: DeserializeOwned + Debug + 'static,
    T2: DeserializeOwned + 'static,
    T3: DeserializeOwned + 'static,
    T4: DeserializeOwned + 'static,
    T6: DeserializeOwned + 'static,
    E1: Display + Debug + Send + Sync + 'static,
    E2: Display + Debug + Send + Sync + 'static,
    E3: Display + Debug + Send + Sync + 'static,
    E4: Display + Debug + Send + Sync + 'static,
    E5: Display + Debug + Send + Sync + 'static,
    E6: Display + Debug + Send + Sync + 'static,
    C: Clone + fmt::Debug + PartialEq + JsonSchema + 'static,
    Q: CustomQuery + DeserializeOwned + 'static,
{
    pub fn with_sudo<T4A, E4A>(
        self,
        sudo_fn: PermissionedFn<T4A, C, E4A, Q>,
    ) -> ContractWrapper<T1, T2, T3, E1, E2, E3, C, Q, T4A, E4A, E5, T6, E6>
    where
        T4A: DeserializeOwned + 'static,
        E4A: Display + Debug + Send + Sync + 'static,
    {
        ContractWrapper {
            execute_fn: self.execute_fn,
            instantiate_fn: self.instantiate_fn,
            query_fn: self.query_fn,
            sudo_fn: Some(sudo_fn),
            reply_fn: self.reply_fn,
            migrate_fn: self.migrate_fn,
        }
    }

    pub fn with_reply<E5A>(
        self,
        reply_fn: ReplyFn<C, E5A, Q>,
    ) -> ContractWrapper<T1, T2, T3, E1, E2, E3, C, Q, T4, E4, E5A, T6, E6>
    where
        E5A: Display + Debug + Send + Sync + 'static,
    {
        ContractWrapper {
            execute_fn: self.execute_fn,
            instantiate_fn: self.instantiate_fn,
            query_fn: self.query_fn,
            sudo_fn: self.sudo_fn,
            reply_fn: Some(reply_fn),
            migrate_fn: self.migrate_fn,
        }
    }

    pub fn with_migrate<T6A, E6A>(
        self,
        migrate_fn: PermissionedFn<T6A, C, E6A, Q>,
    ) -> ContractWrapper<T1, T2, T3, E1, E2, E3, C, Q, T4, E4, E5, T6A, E6A>
    where
        T6A: DeserializeOwned + 'static,
        E6A: Display + Debug + Send + Sync + 'static,
    {
        ContractWrapper {
            execute_fn: self.execute_fn,
            instantiate_fn: self.instantiate_fn,
            query_fn: self.query_fn,
            sudo_fn: self.sudo_fn,
            reply_fn: self.reply_fn,
            migrate_fn: Some(migrate_fn),
        }
    }
}

fn decustomize_deps_mut<'a, Q>(deps: &'a mut DepsMut<Q>) -> DepsMut<'a, Empty>
where
    Q: CustomQuery + DeserializeOwned + 'static,
{
    DepsMut {
        storage: deps.storage,
        api: deps.api,
        querier: QuerierWrapper::new(deps.querier.deref()),
    }
}

fn decustomize_deps<'a, Q>(deps: &'a Deps<'a, Q>) -> Deps<'a, Empty>
where
    Q: CustomQuery + DeserializeOwned + 'static,
{
    Deps {
        storage: deps.storage,
        api: deps.api,
        querier: QuerierWrapper::new(deps.querier.deref()),
    }
}

fn customize_response<C>(resp: Response<Empty>) -> Response<C>
where
    C: Clone + fmt::Debug + PartialEq + JsonSchema,
{
    let mut customized_resp = Response::<C>::new()
        .add_submessages(resp.messages.into_iter().map(customize_msg::<C>))
        .add_events(resp.events)
        .add_attributes(resp.attributes);
    customized_resp.data = resp.data;
    customized_resp
}

fn customize_msg<C>(msg: SubMsg<Empty>) -> SubMsg<C>
where
    C: Clone + fmt::Debug + PartialEq + JsonSchema,
{
    SubMsg {
        msg: match msg.msg {
            CosmosMsg::Wasm(wasm) => CosmosMsg::Wasm(wasm),
            CosmosMsg::Bank(bank) => CosmosMsg::Bank(bank),
            CosmosMsg::Staking(staking) => CosmosMsg::Staking(staking),
            CosmosMsg::Distribution(distribution) => CosmosMsg::Distribution(distribution),
            CosmosMsg::Custom(_) => unreachable!(),
            #[cfg(feature = "stargate")]
            CosmosMsg::Ibc(ibc) => CosmosMsg::Ibc(ibc),
            #[cfg(feature = "stargate")]
            CosmosMsg::Stargate { type_url, value } => CosmosMsg::Stargate { type_url, value },
            _ => panic!("unknown message variant {:?}", msg),
        },
        id: msg.id,
        gas_limit: msg.gas_limit,
        reply_on: msg.reply_on,
    }
}

impl<T1, T2, T3, E1, E2, E3, C, T4, E4, E5, T6, E6, Q> Contract<C, Q>
    for ContractWrapper<T1, T2, T3, E1, E2, E3, C, Q, T4, E4, E5, T6, E6>
where
    T1: DeserializeOwned + Debug + Clone,
    T2: DeserializeOwned + Debug + Clone,
    T3: DeserializeOwned + Debug + Clone,
    T4: DeserializeOwned,
    T6: DeserializeOwned,
    E1: Display + Debug + Send + Sync + Error + 'static,
    E2: Display + Debug + Send + Sync + Error + 'static,
    E3: Display + Debug + Send + Sync + Error + 'static,
    E4: Display + Debug + Send + Sync + 'static,
    E5: Display + Debug + Send + Sync + 'static,
    E6: Display + Debug + Send + Sync + 'static,
    C: CustomMsg + DeserializeOwned + Clone + fmt::Debug + PartialEq + JsonSchema,
    Q: CustomQuery + DeserializeOwned,
{
    fn execute(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        info: MessageInfo,
        msg: Vec<u8>,
        fork_state: ForkState<C, Q>,
    ) -> AnyResult<Response<C>> {
        let querier = MockQuerier::new(fork_state.clone());
        let mut storage = DualStorage::new(
            fork_state.remote,
            env.contract.address.to_string(),
            Box::new(StorageWrapper::new(deps.storage)),
        )?;
        let deps = DepsMut {
            storage: &mut storage,
            api: deps.api,
            querier: QuerierWrapper::new(&querier),
        };

        let msg: T1 = from_json(msg)?;
        (self.execute_fn)(deps, env, info, msg).map_err(|err| anyhow!(err))
    }

    fn instantiate(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        info: MessageInfo,
        msg: Vec<u8>,
        fork_state: ForkState<C, Q>,
    ) -> AnyResult<Response<C>> {
        let querier = MockQuerier::new(fork_state.clone());
        let mut storage = DualStorage::new(
            fork_state.remote,
            env.contract.address.to_string(),
            Box::new(StorageWrapper::new(deps.storage)),
        )?;
        let deps = DepsMut {
            storage: &mut storage,
            api: deps.api,
            querier: QuerierWrapper::new(&querier),
        };
        let msg: T2 = from_json(msg)?;
        (self.instantiate_fn)(deps, env, info, msg).map_err(|err| anyhow!(err))
    }

    fn query(
        &self,
        deps: Deps<Q>,
        env: Env,
        msg: Vec<u8>,
        fork_state: ForkState<C, Q>,
    ) -> AnyResult<Binary> {
        let querier = MockQuerier::new(fork_state.clone());
        let mut storage = DualStorage::new(
            fork_state.remote,
            env.contract.address.to_string(),
            Box::new(ReadonlyStorageWrapper::new(deps.storage)),
        )?;
        let deps = Deps {
            storage: &mut storage,
            api: deps.api,
            querier: QuerierWrapper::new(&querier),
        };
        let msg: T3 = from_json(msg)?;
        (self.query_fn)(deps, env, msg).map_err(|err| anyhow!(err))
    }

    // this returns an error if the contract doesn't implement sudo
    fn sudo(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: Vec<u8>,
        fork_state: ForkState<C, Q>,
    ) -> AnyResult<Response<C>> {
        let querier = MockQuerier::new(fork_state.clone());
        let mut storage = DualStorage::new(
            fork_state.remote,
            env.contract.address.to_string(),
            Box::new(StorageWrapper::new(deps.storage)),
        )?;
        let deps = DepsMut {
            storage: &mut storage,
            api: deps.api,
            querier: QuerierWrapper::new(&querier),
        };
        let msg = from_json(msg)?;
        match &self.sudo_fn {
            Some(sudo) => sudo(deps, env, msg).map_err(|err| anyhow!(err)),
            None => bail!("sudo not implemented for contract"),
        }
    }

    // this returns an error if the contract doesn't implement reply
    fn reply(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        reply_data: Reply,
        fork_state: ForkState<C, Q>,
    ) -> AnyResult<Response<C>> {
        let querier = MockQuerier::new(fork_state.clone());
        let mut storage = DualStorage::new(
            fork_state.remote,
            env.contract.address.to_string(),
            Box::new(StorageWrapper::new(deps.storage)),
        )?;
        let deps = DepsMut {
            storage: &mut storage,
            api: deps.api,
            querier: QuerierWrapper::new(&querier),
        };
        match &self.reply_fn {
            Some(reply) => reply(deps, env, reply_data).map_err(|err| anyhow!(err)),
            None => bail!("reply not implemented for contract"),
        }
    }

    // this returns an error if the contract doesn't implement migrate
    fn migrate(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: Vec<u8>,
        fork_state: ForkState<C, Q>,
    ) -> AnyResult<Response<C>> {
        let querier = MockQuerier::new(fork_state.clone());
        let mut storage = DualStorage::new(
            fork_state.remote,
            env.contract.address.to_string(),
            Box::new(StorageWrapper::new(deps.storage)),
        )?;
        let deps = DepsMut {
            storage: &mut storage,
            api: deps.api,
            querier: QuerierWrapper::new(&querier),
        };
        let msg = from_json(msg)?;
        match &self.migrate_fn {
            Some(migrate) => migrate(deps, env, msg).map_err(|err| anyhow!(err)),
            None => bail!("migrate not implemented for contract"),
        }
    }
}

#[cfg(test)]
pub mod test {

    use cosmwasm_std::{
        testing::{mock_dependencies, mock_env, mock_info},
        to_json_binary, Binary, Deps, DepsMut, Empty, Env, MessageInfo, Response, StdResult,
    };

    use super::ContractWrapper;

    fn execute(deps: DepsMut, env: Env, info: MessageInfo, _msg: Empty) -> StdResult<Response> {
        Ok(Response::new())
    }

    fn query(deps: Deps, env: Env, _msg: Empty) -> StdResult<Binary> {
        to_json_binary("resp")
    }

    fn instantiate(deps: DepsMut, env: Env, info: MessageInfo, _msg: Empty) -> StdResult<Response> {
        Ok(Response::new())
    }

    #[test]
    fn mock_contract() -> anyhow::Result<()> {
        let contract = ContractWrapper::new(execute, instantiate, query);

        let clone = contract.execute_fn;
        let second_clone = clone;

        clone(
            mock_dependencies().as_mut(),
            mock_env(),
            mock_info("sender", &[]),
            Empty {},
        )?;

        second_clone(
            mock_dependencies().as_mut(),
            mock_env(),
            mock_info("sender", &[]),
            Empty {},
        )?;

        Ok(())
    }
}
