use crate::prefixed_storage::contract_namespace;
use crate::wasm_emulation::api::RealApi;
use cosmwasm_std::CustomMsg;
use cw_orch_daemon::queriers::CosmWasm;
use cw_orch_daemon::queriers::DaemonQuerier;

use ibc_chain_registry::chain::ChainData;

use std::collections::HashMap;
use std::fmt::Debug;

use cosmwasm_std::{
    to_binary, Addr, Api, Attribute, BankMsg, Binary, BlockInfo, Coin, ContractInfo,
    ContractInfoResponse, CustomQuery, Deps, DepsMut, Env, Event, MessageInfo, Order, Querier,
    QuerierWrapper, Record, Reply, ReplyOn, Response, StdResult, Storage, SubMsg, SubMsgResponse,
    SubMsgResult, TransactionInfo, WasmMsg, WasmQuery,
};

use prost::Message;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use cw_storage_plus::Map;

use crate::app::{CosmosRouter, RouterQuerier};
use crate::contracts::Contract;
use crate::error::Error;
use crate::executor::AppResponse;
use crate::prefixed_storage::{prefixed, prefixed_read, PrefixedStorage, ReadonlyPrefixedStorage};
use crate::transactions::transactional;
use cosmwasm_std::testing::mock_wasmd_attr;

use anyhow::{bail, Context, Result as AnyResult};
#[cfg(feature = "cosmwasm_1_2")]
use sha2::{Digest, Sha256};

use crate::wasm_emulation::channel::get_channel;
use crate::wasm_emulation::contract::WasmContract;
use crate::wasm_emulation::input::{SerChainData, WasmStorage};
use crate::wasm_emulation::query::AllQuerier;

// Contract state is kept in Storage, separate from the contracts themselves
const CONTRACTS: Map<&Addr, ContractData> = Map::new("contracts");

pub const NAMESPACE_WASM: &[u8] = b"wasm";
/// See <https://github.com/chipshort/wasmd/blob/d0e3ed19f041e65f112d8e800416b3230d0005a2/x/wasm/types/events.go#L58>
const CONTRACT_ATTR: &str = "_contract_address";
const LOCAL_CODE_OFFFSET: usize = 5_000_000;

#[derive(Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct WasmSudo {
    pub contract_addr: Addr,
    pub msg: Binary,
}

impl WasmSudo {
    pub fn new<T: Serialize>(contract_addr: &Addr, msg: &T) -> StdResult<WasmSudo> {
        Ok(WasmSudo {
            contract_addr: contract_addr.clone(),
            msg: to_binary(msg)?,
        })
    }
}

/// Contract data includes information about contract,
/// equivalent of `ContractInfo` in **wasmd** interface.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ContractData {
    /// Identifier of stored contract code
    pub code_id: u64,
    /// Address of account who initially instantiated the contract
    pub creator: Addr,
    /// Optional address of account who can execute migrations
    pub admin: Option<Addr>,
    /// Metadata passed while contract instantiation
    pub label: String,
    /// Blockchain height in the moment of instantiating the contract
    pub created: u64,
}

/// Contract code base data.
struct CodeData {
    /// Address of an account that initially stored the contract code.
    //FIXME Remove this feature flag when the default flag is `cosmwasm_1_2` or higher.
    #[cfg_attr(feature = "cosmwasm_1_1", allow(dead_code))]
    creator: Addr,
    /// Seed used to generate a checksum for underlying code base.
    //FIXME Remove this feature flag when the default flag is `cosmwasm_1_2` or higher.
    #[cfg_attr(feature = "cosmwasm_1_1", allow(dead_code))]
    seed: usize,
    /// Identifier of the code base where the contract code is stored in memory.
    code_base_id: usize,
}

pub trait Wasm<ExecC, QueryC>: AllQuerier {
    /// Handles all WasmQuery requests
    fn query(
        &self,
        api: &dyn Api,
        storage: &dyn Storage,
        querier: &dyn Querier,
        block: &BlockInfo,
        request: WasmQuery,
    ) -> AnyResult<Binary>;

    /// Handles all WasmMsg messages
    fn execute(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: Addr,
        msg: WasmMsg,
    ) -> AnyResult<AppResponse>;

    /// Admin interface, cannot be called via CosmosMsg
    fn sudo(
        &self,
        api: &dyn Api,
        contract_addr: Addr,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        msg: Binary,
    ) -> AnyResult<AppResponse>;
}

// pub struct WasmKeeper<ExecC: 'static, QueryC: 'static> {
pub struct WasmKeeper<ExecC, QueryC> {
    /// Contract codes that stand for wasm code in real-life blockchain.
    code_base: HashMap<usize, WasmContract>,
    /// Code data with code base identifier and additional attributes.  
    code_data: HashMap<usize, CodeData>,
    /// Just markers to make type elision fork when using it as `Wasm` trait
    _e: std::marker::PhantomData<ExecC>,
    /// Just markers to make type elision fork when using it as `Wasm` trait
    _q: std::marker::PhantomData<QueryC>,
    generator: Box<dyn AddressGenerator>,

    // chain on which the contract should be queried/tested against
    chain: Option<ChainData>,
}

pub trait AddressGenerator {
    fn next_contract_address(&self, storage: &mut dyn Storage, chain_name: Option<String>) -> Addr;
}

struct SimpleAddressGenerator();

impl AddressGenerator for SimpleAddressGenerator {
    fn next_contract_address(&self, storage: &mut dyn Storage, prefix: Option<String>) -> Addr {
        let count = CONTRACTS
            .range_raw(
                &prefixed_read(storage, NAMESPACE_WASM),
                None,
                None,
                Order::Ascending,
            )
            .count();

        let prefix = prefix.unwrap_or("cosmos".to_string());

        RealApi::new(&prefix).next_contract_address(count)
    }
}

impl<ExecC, QueryC> Default for WasmKeeper<ExecC, QueryC> {
    fn default() -> WasmKeeper<ExecC, QueryC> {
        Self {
            code_base: HashMap::new(),
            code_data: HashMap::new(),
            _e: std::marker::PhantomData,
            _q: std::marker::PhantomData,
            generator: Box::new(SimpleAddressGenerator()),
            chain: None,
        }
    }
}

impl<ExecC, QueryC> AllQuerier for WasmKeeper<ExecC, QueryC> {
    type Output = WasmStorage;
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
        })
    }
}

impl<ExecC, QueryC> Wasm<ExecC, QueryC> for WasmKeeper<ExecC, QueryC>
where
    ExecC: CustomMsg + DeserializeOwned + 'static,
    QueryC: CustomQuery + DeserializeOwned + 'static,
{
    fn query(
        &self,
        api: &dyn Api,
        storage: &dyn Storage,
        querier: &dyn Querier,
        block: &BlockInfo,
        request: WasmQuery,
    ) -> AnyResult<Binary> {
        match request {
            WasmQuery::Smart { contract_addr, msg } => {
                let addr = api.addr_validate(&contract_addr)?;
                self.query_smart(addr, api, storage, querier, block, msg.into())
            }
            WasmQuery::Raw { contract_addr, key } => {
                let addr = api.addr_validate(&contract_addr)?;
                Ok(self.query_raw(addr, storage, &key))
            }
            WasmQuery::ContractInfo { contract_addr } => {
                let addr = api.addr_validate(&contract_addr)?;
                let contract = self.load_contract(storage, &addr)?;
                let mut res = ContractInfoResponse::default();
                res.code_id = contract.code_id;
                res.creator = contract.creator.to_string();
                res.admin = contract.admin.map(|x| x.into());
                to_binary(&res).map_err(Into::into)
            }
            #[cfg(feature = "cosmwasm_1_2")]
            WasmQuery::CodeInfo { code_id } => {
                let code_data = self.code_data(code_id)?;
                let mut res = cosmwasm_std::CodeInfoResponse::default();
                res.code_id = code_id;
                res.creator = code_data.creator.to_string();
                res.checksum = cosmwasm_std::HexBinary::from(
                    Sha256::digest(format!("contract code {}", code_data.seed)).to_vec(),
                );
                to_binary(&res).map_err(Into::into)
            }
            other => bail!(Error::UnsupportedWasmQuery(other)),
        }
    }

    fn execute(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: Addr,
        msg: WasmMsg,
    ) -> AnyResult<AppResponse> {
        self.execute_wasm(api, storage, router, block, sender.clone(), msg.clone())
            .context(format!(
                "error executing WasmMsg:\nsender: {}\n{:?}",
                sender, msg
            ))
    }

    fn sudo(
        &self,
        api: &dyn Api,
        contract: Addr,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        msg: Binary,
    ) -> AnyResult<AppResponse> {
        let custom_event = Event::new("sudo").add_attribute(CONTRACT_ATTR, &contract);

        let res = self.call_sudo(contract.clone(), api, storage, router, block, msg.to_vec())?;
        let (res, msgs) = self.build_app_response(&contract, custom_event, res);
        self.process_response(api, router, storage, block, contract, res, msgs)
    }
}

impl<ExecC, QueryC> WasmKeeper<ExecC, QueryC> {
    /// Stores contract code in the in-memory lookup table.
    /// Returns an identifier of the stored contract code.
    pub fn store_code(&mut self, creator: Addr, code: WasmContract) -> u64 {
        let code_id = self.code_base.len() + 1 + LOCAL_CODE_OFFFSET;
        self.code_base.insert(code_id, code);
        self.code_data.insert(
            code_id,
            CodeData {
                creator,
                seed: code_id,
                code_base_id: code_id,
            },
        );
        code_id as u64
    }

    /// Duplicates contract code with specified identifier.
    pub fn duplicate_code(&mut self, code_id: u64) -> AnyResult<u64> {
        let code_data = self.code_data(code_id)?;
        let code = self
            .code_base
            .get(&(code_id as usize))
            .ok_or(Error::UnregisteredCodeId(code_id))?;
        let code_id = self.code_base.len() + 1 + LOCAL_CODE_OFFFSET;
        self.code_data.insert(
            code_id,
            CodeData {
                creator: code_data.creator.clone(),
                seed: code_data.seed,
                code_base_id: code_id,
            },
        );
        self.code_base.insert(code_id, code.clone());

        Ok(code_id as u64)
    }

    /// Returns a handler to code of the contract with specified code id.
    pub fn contract_code(&self, code_id: u64) -> AnyResult<WasmContract> {
        let code_data = self.code_data(code_id)?;
        Ok(self
            .code_base
            .get(&code_data.code_base_id)
            .ok_or(Error::UnregisteredCodeId(code_id))?
            .clone())
    }

    /// Returns code data of the contract with specified code id.
    fn code_data(&self, code_id: u64) -> AnyResult<&CodeData> {
        if code_id < 1 {
            bail!(Error::InvalidCodeId);
        }
        Ok(self
            .code_data
            .get(&(code_id as usize))
            .ok_or(Error::UnregisteredCodeId(code_id))?)
    }

    /// This function won't error if it doesn't find the contract
    /// This allows for querying an online contract struct without having to check if it exists just there
    /// Upon execution, it will fail if the contract doesn't exist
    fn get_code(&self, storage: &dyn Storage, address: &Addr) -> WasmContract {
        if let Ok(handler) = self.load_contract(storage, address) {
            let code = self.code_base.get(&(handler.code_id as usize));
            if let Some(code) = code {
                return code.clone();
            } else {
                return WasmContract::new_distant_code_id(
                    handler.code_id.try_into().unwrap(),
                    self.chain.clone().unwrap(),
                );
            }
        }

        WasmContract::new_distant_contract(address.to_string(), self.chain.clone().unwrap())
    }

    pub fn load_distant_contract(
        chain: impl Into<SerChainData>,
        address: &Addr,
    ) -> AnyResult<ContractData> {
        let (rt, channel) = get_channel(chain)?;

        let wasm_querier = CosmWasm::new(channel);

        let code_info = rt.block_on(wasm_querier.contract_info(address.clone()))?;

        Ok(ContractData {
            admin: {
                match code_info.admin.as_str() {
                    "" => None,
                    a => Some(Addr::unchecked(a)),
                }
            },
            code_id: code_info.code_id.try_into()?,
            created: code_info.created.unwrap().block_height,
            creator: Addr::unchecked(code_info.creator),
            label: code_info.label,
        })
    }

    pub fn load_contract(&self, storage: &dyn Storage, address: &Addr) -> AnyResult<ContractData> {
        if let Ok(local_contract) = CONTRACTS.load(&prefixed_read(storage, NAMESPACE_WASM), address)
        {
            return Ok(local_contract);
        }

        Self::load_distant_contract(self.chain.clone().unwrap(), address)
    }

    pub fn dump_wasm_raw(&self, storage: &dyn Storage, address: &Addr) -> Vec<Record> {
        let storage = self.contract_storage_readonly(storage, address);
        storage.range(None, None, Order::Ascending).collect()
    }

    fn contract_storage<'a>(
        &self,
        storage: &'a mut dyn Storage,
        address: &Addr,
    ) -> Box<dyn Storage + 'a> {
        // We double-namespace this, once from global storage -> wasm_storage
        // then from wasm_storage -> the contracts subspace
        let namespace = contract_namespace(address);
        let storage = PrefixedStorage::multilevel(storage, &[NAMESPACE_WASM, &namespace]);
        Box::new(storage)
    }

    // fails RUNTIME if you try to write. please don't
    fn contract_storage_readonly<'a>(
        &self,
        storage: &'a dyn Storage,
        address: &Addr,
    ) -> Box<dyn Storage + 'a> {
        // We double-namespace this, once from global storage -> wasm_storage
        // then from wasm_storage -> the contracts subspace
        let namespace = contract_namespace(address);
        let storage = ReadonlyPrefixedStorage::multilevel(storage, &[NAMESPACE_WASM, &namespace]);
        Box::new(storage)
    }

    fn verify_attributes(attributes: &[Attribute]) -> AnyResult<()> {
        for attr in attributes {
            let key = attr.key.trim();
            let val = attr.value.trim();

            if key.is_empty() {
                bail!(Error::empty_attribute_key(val));
            }

            if val.is_empty() {
                bail!(Error::empty_attribute_value(key));
            }

            if key.starts_with('_') {
                bail!(Error::reserved_attribute_key(key));
            }
        }

        Ok(())
    }

    fn verify_response<T>(response: Response<T>) -> AnyResult<Response<T>>
    where
        T: Clone + Debug + PartialEq + JsonSchema,
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

impl<ExecC, QueryC> WasmKeeper<ExecC, QueryC>
where
    ExecC: CustomMsg + DeserializeOwned + 'static,
    QueryC: CustomQuery + DeserializeOwned + 'static,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_custom_address_generator(generator: impl AddressGenerator + 'static) -> Self {
        Self {
            generator: Box::new(generator),
            ..Default::default()
        }
    }

    pub fn set_chain(&mut self, chain: ChainData) {
        self.chain = Some(chain);
    }

    pub fn query_smart(
        &self,
        address: Addr,
        api: &dyn Api,
        storage: &dyn Storage,
        querier: &dyn Querier,
        block: &BlockInfo,
        msg: Vec<u8>,
    ) -> AnyResult<Binary> {
        self.with_storage_readonly(
            api,
            storage,
            querier,
            block,
            address,
            |handler, deps, env| {
                <WasmContract as Contract<ExecC, QueryC>>::query(&handler, deps, env, msg)
            },
        )
    }

    pub fn query_raw(&self, address: Addr, storage: &dyn Storage, key: &[u8]) -> Binary {
        let storage = self.contract_storage_readonly(storage, &address);
        let data = storage.get(key).unwrap_or_default();
        data.into()
    }

    fn send<T>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: T,
        recipient: String,
        amount: &[Coin],
    ) -> AnyResult<AppResponse>
    where
        T: Into<Addr>,
    {
        if !amount.is_empty() {
            let msg: cosmwasm_std::CosmosMsg<ExecC> = BankMsg::Send {
                to_address: recipient,
                amount: amount.to_vec(),
            }
            .into();
            let res = router.execute(api, storage, block, sender.into(), msg)?;
            Ok(res)
        } else {
            Ok(AppResponse::default())
        }
    }

    /// unified logic for UpdateAdmin and ClearAdmin messages
    fn update_admin(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        sender: Addr,
        contract_addr: &str,
        new_admin: Option<String>,
    ) -> AnyResult<AppResponse> {
        let contract_addr = api.addr_validate(contract_addr)?;
        let admin = new_admin.map(|a| api.addr_validate(&a)).transpose()?;

        // check admin status
        let mut data = self.load_contract(storage, &contract_addr)?;
        if data.admin != Some(sender) {
            bail!("Only admin can update the contract admin: {:?}", data.admin);
        }
        // update admin field
        data.admin = admin;
        self.save_contract(storage, &contract_addr, &data)?;

        // no custom event here
        Ok(AppResponse {
            data: None,
            events: vec![],
        })
    }

    // this returns the contract address as well, so we can properly resend the data
    fn execute_wasm(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: Addr,
        wasm_msg: WasmMsg,
    ) -> AnyResult<AppResponse> {
        match wasm_msg {
            WasmMsg::Execute {
                contract_addr,
                msg,
                funds,
            } => {
                let contract_addr = api.addr_validate(&contract_addr)?;
                // first move the cash
                self.send(
                    api,
                    storage,
                    router,
                    block,
                    sender.clone(),
                    contract_addr.clone().into(),
                    &funds,
                )?;

                // then call the contract
                let info = MessageInfo { sender, funds };
                let res = self.call_execute(
                    api,
                    storage,
                    contract_addr.clone(),
                    router,
                    block,
                    info,
                    msg.to_vec(),
                )?;

                let custom_event =
                    Event::new("execute").add_attribute(CONTRACT_ATTR, &contract_addr);

                let (res, msgs) = self.build_app_response(&contract_addr, custom_event, res);
                let mut res =
                    self.process_response(api, router, storage, block, contract_addr, res, msgs)?;
                res.data = execute_response(res.data);
                Ok(res)
            }
            WasmMsg::Instantiate {
                admin,
                code_id,
                msg,
                funds,
                label,
            } => {
                if label.is_empty() {
                    bail!("Label is required on all contracts");
                }

                let contract_addr = self.register_contract(
                    storage,
                    code_id,
                    sender.clone(),
                    admin.map(Addr::unchecked),
                    label,
                    block.height,
                )?;

                // move the cash
                self.send(
                    api,
                    storage,
                    router,
                    block,
                    sender.clone(),
                    contract_addr.clone().into(),
                    &funds,
                )?;

                // then call the contract
                let info = MessageInfo { sender, funds };
                let res = self.call_instantiate(
                    contract_addr.clone(),
                    api,
                    storage,
                    router,
                    block,
                    info,
                    msg.to_vec(),
                )?;

                let custom_event = Event::new("instantiate")
                    .add_attribute(CONTRACT_ATTR, &contract_addr)
                    .add_attribute("code_id", code_id.to_string());

                let (res, msgs) = self.build_app_response(&contract_addr, custom_event, res);
                let mut res = self.process_response(
                    api,
                    router,
                    storage,
                    block,
                    contract_addr.clone(),
                    res,
                    msgs,
                )?;
                res.data = Some(instantiate_response(res.data, &contract_addr));
                Ok(res)
            }
            WasmMsg::Migrate {
                contract_addr,
                new_code_id,
                msg,
            } => {
                let contract_addr = api.addr_validate(&contract_addr)?;

                // We don't check if the code exists here, the call_migrate hook, will take care of that
                // This allows migrating to an on-chain code_id

                let mut data = self.load_contract(storage, &contract_addr)?;
                if data.admin != Some(sender) {
                    bail!("Only admin can migrate contract: {:?}", data.admin);
                }
                data.code_id = new_code_id;
                self.save_contract(storage, &contract_addr, &data)?;

                // then call migrate
                let res = self.call_migrate(
                    contract_addr.clone(),
                    api,
                    storage,
                    router,
                    block,
                    msg.to_vec(),
                )?;

                let custom_event = Event::new("migrate")
                    .add_attribute(CONTRACT_ATTR, &contract_addr)
                    .add_attribute("code_id", new_code_id.to_string());
                let (res, msgs) = self.build_app_response(&contract_addr, custom_event, res);
                let mut res =
                    self.process_response(api, router, storage, block, contract_addr, res, msgs)?;
                res.data = execute_response(res.data);
                Ok(res)
            }
            WasmMsg::UpdateAdmin {
                contract_addr,
                admin,
            } => self.update_admin(api, storage, sender, &contract_addr, Some(admin)),
            WasmMsg::ClearAdmin { contract_addr } => {
                self.update_admin(api, storage, sender, &contract_addr, None)
            }
            msg => bail!(Error::UnsupportedWasmMsg(msg)),
        }
    }

    /// This will execute the given messages, making all changes to the local cache.
    /// This *will* write some data to the cache if the message fails half-way through.
    /// All sequential calls to RouterCache will be one atomic unit (all commit or all fail).
    ///
    /// For normal use cases, you can use Router::execute() or Router::execute_multi().
    /// This is designed to be handled internally as part of larger process flows.
    ///
    /// The `data` on `AppResponse` is data returned from `reply` call, not from execution of
    /// sub-message itself. In case if `reply` is not called, no `data` is set.
    fn execute_submsg(
        &self,
        api: &dyn Api,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        storage: &mut dyn Storage,
        block: &BlockInfo,
        contract: Addr,
        msg: SubMsg<ExecC>,
    ) -> AnyResult<AppResponse> {
        let SubMsg {
            msg, id, reply_on, ..
        } = msg;

        // execute in cache
        let res = transactional(storage, |write_cache, _| {
            router.execute(api, write_cache, block, contract.clone(), msg)
        });

        // call reply if meaningful
        if let Ok(mut r) = res {
            if matches!(reply_on, ReplyOn::Always | ReplyOn::Success) {
                let reply = Reply {
                    id,
                    result: SubMsgResult::Ok(SubMsgResponse {
                        events: r.events.clone(),
                        data: r.data,
                    }),
                };
                // do reply and combine it with the original response
                let reply_res = self._reply(api, router, storage, block, contract, reply)?;
                // override data
                r.data = reply_res.data;
                // append the events
                r.events.extend_from_slice(&reply_res.events);
            } else {
                // reply is not called, no data should be returned
                r.data = None;
            }

            Ok(r)
        } else if let Err(e) = res {
            if matches!(reply_on, ReplyOn::Always | ReplyOn::Error) {
                let reply = Reply {
                    id,
                    result: SubMsgResult::Err(e.to_string()),
                };
                self._reply(api, router, storage, block, contract, reply)
            } else {
                Err(e)
            }
        } else {
            res
        }
    }

    fn _reply(
        &self,
        api: &dyn Api,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        storage: &mut dyn Storage,
        block: &BlockInfo,
        contract: Addr,
        reply: Reply,
    ) -> AnyResult<AppResponse> {
        let ok_attr = if reply.result.is_ok() {
            "handle_success"
        } else {
            "handle_failure"
        };
        let custom_event = Event::new("reply")
            .add_attribute(CONTRACT_ATTR, &contract)
            .add_attribute("mode", ok_attr);

        let res = self.call_reply(contract.clone(), api, storage, router, block, reply)?;
        let (res, msgs) = self.build_app_response(&contract, custom_event, res);
        self.process_response(api, router, storage, block, contract, res, msgs)
    }

    // this captures all the events and data from the contract call.
    // it does not handle the messages
    fn build_app_response(
        &self,
        contract: &Addr,
        custom_event: Event, // entry-point specific custom event added by x/wasm
        response: Response<ExecC>,
    ) -> (AppResponse, Vec<SubMsg<ExecC>>) {
        let Response {
            messages,
            attributes,
            events,
            data,
            ..
        } = response;

        // always add custom event
        let mut app_events = Vec::with_capacity(2 + events.len());
        app_events.push(custom_event);

        // we only emit the `wasm` event if some attributes are specified
        if !attributes.is_empty() {
            // turn attributes into event and place it first
            let wasm_event = Event::new("wasm")
                .add_attribute(CONTRACT_ATTR, contract)
                .add_attributes(attributes);
            app_events.push(wasm_event);
        }

        // These need to get `wasm-` prefix to match the wasmd semantics (custom wasm messages cannot
        // fake system level event types, like transfer from the bank module)
        let wasm_events = events.into_iter().map(|mut ev| {
            ev.ty = format!("wasm-{}", ev.ty);
            ev.attributes
                .insert(0, mock_wasmd_attr(CONTRACT_ATTR, contract));
            ev
        });
        app_events.extend(wasm_events);

        let app = AppResponse {
            events: app_events,
            data,
        };
        (app, messages)
    }

    fn process_response(
        &self,
        api: &dyn Api,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        storage: &mut dyn Storage,
        block: &BlockInfo,
        contract: Addr,
        response: AppResponse,
        messages: Vec<SubMsg<ExecC>>,
    ) -> AnyResult<AppResponse> {
        let AppResponse { mut events, data } = response;

        // recurse in all messages
        let data = messages.into_iter().try_fold(data, |data, resend| {
            let sub_res =
                self.execute_submsg(api, router, storage, block, contract.clone(), resend)?;
            events.extend_from_slice(&sub_res.events);
            Ok::<_, anyhow::Error>(sub_res.data.or(data))
        })?;

        Ok(AppResponse { events, data })
    }

    /// This just creates an address and empty storage instance, returning the new address
    /// You must call init after this to set up the contract properly.
    /// These are separated into two steps to have cleaner return values.
    pub fn register_contract(
        // TODO
        &self,
        storage: &mut dyn Storage,
        code_id: u64,
        creator: Addr,
        admin: impl Into<Option<Addr>>,
        label: String,
        created: u64,
    ) -> AnyResult<Addr> {
        let addr = self
            .generator
            .next_contract_address(storage, self.chain.clone().map(|c| c.bech32_prefix));

        let info = ContractData {
            code_id,
            creator,
            admin: admin.into(),
            label,
            created,
        };
        self.save_contract(storage, &addr, &info)?;
        Ok(addr)
    }

    pub fn call_execute(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        address: Addr,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        info: MessageInfo,
        msg: Vec<u8>,
    ) -> AnyResult<Response<ExecC>> {
        Self::verify_response(self.with_storage(
            api,
            storage,
            router,
            block,
            address,
            |contract, deps, env| contract.execute(deps, env, info, msg),
        )?)
    }

    pub fn call_instantiate(
        &self,
        address: Addr,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        info: MessageInfo,
        msg: Vec<u8>,
    ) -> AnyResult<Response<ExecC>> {
        Self::verify_response(self.with_storage(
            api,
            storage,
            router,
            block,
            address,
            |contract, deps, env| contract.instantiate(deps, env, info, msg),
        )?)
    }

    pub fn call_reply(
        &self,
        address: Addr,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        reply: Reply,
    ) -> AnyResult<Response<ExecC>> {
        Self::verify_response(self.with_storage(
            api,
            storage,
            router,
            block,
            address,
            |contract, deps, env| contract.reply(deps, env, reply),
        )?)
    }

    pub fn call_sudo(
        &self,
        address: Addr,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        msg: Vec<u8>,
    ) -> AnyResult<Response<ExecC>> {
        Self::verify_response(self.with_storage(
            api,
            storage,
            router,
            block,
            address,
            |contract, deps, env| contract.sudo(deps, env, msg),
        )?)
    }

    pub fn call_migrate(
        &self,
        address: Addr,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        msg: Vec<u8>,
    ) -> AnyResult<Response<ExecC>> {
        Self::verify_response(self.with_storage(
            api,
            storage,
            router,
            block,
            address,
            |contract, deps, env| contract.migrate(deps, env, msg),
        )?)
    }

    fn get_env<T: Into<Addr>>(&self, address: T, block: &BlockInfo) -> Env {
        Env {
            block: block.clone(),
            contract: ContractInfo {
                address: address.into(),
            },
            transaction: Some(TransactionInfo { index: 0 }),
        }
    }

    fn with_storage_readonly<F, T>(
        &self,
        api: &dyn Api,
        storage: &dyn Storage,
        querier: &dyn Querier,
        block: &BlockInfo,
        address: Addr,
        action: F,
    ) -> AnyResult<T>
    where
        F: FnOnce(WasmContract, Deps<QueryC>, Env) -> AnyResult<T>,
    {
        let handler = self.get_code(storage, &address);

        let storage = self.contract_storage_readonly(storage, &address);
        let env = self.get_env(address, block);

        let deps = Deps {
            storage: storage.as_ref(),
            api,
            querier: QuerierWrapper::new(querier),
        };
        action(handler, deps, env)
    }

    fn with_storage<F, T>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        address: Addr,
        action: F,
    ) -> AnyResult<T>
    where
        F: FnOnce(WasmContract, DepsMut<QueryC>, Env) -> AnyResult<T>,
        ExecC: DeserializeOwned,
    {
        let handler = self.get_code(storage, &address);

        // We don't actually need a transaction here, as it is already embedded in a transactional.
        // execute_submsg or App.execute_multi.
        // However, we need to get write and read access to the same storage in two different objects,
        // and this is the only way I know how to do so.
        transactional(storage, |write_cache, read_store| {
            let mut contract_storage = self.contract_storage(write_cache, &address);
            let querier = RouterQuerier::new(router, api, read_store, block);
            let env = self.get_env(address, block);

            let deps = DepsMut {
                storage: contract_storage.as_mut(),
                api,
                querier: QuerierWrapper::new(&querier),
            };
            action(handler.clone(), deps, env)
        })
    }

    pub fn save_contract(
        &self,
        storage: &mut dyn Storage,
        address: &Addr,
        contract: &ContractData,
    ) -> AnyResult<()> {
        CONTRACTS
            .save(&mut prefixed(storage, NAMESPACE_WASM), address, contract)
            .map_err(Into::into)
    }
}

// TODO: replace with code in utils

#[derive(Clone, PartialEq, Message)]
struct InstantiateResponse {
    #[prost(string, tag = "1")]
    pub address: ::prost::alloc::string::String,
    #[prost(bytes, tag = "2")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}

// TODO: encode helpers in utils
fn instantiate_response(data: Option<Binary>, contact_address: &Addr) -> Binary {
    let data = data.unwrap_or_default().to_vec();
    let init_data = InstantiateResponse {
        address: contact_address.into(),
        data,
    };
    let mut new_data = Vec::<u8>::with_capacity(init_data.encoded_len());
    // the data must encode successfully
    init_data.encode(&mut new_data).unwrap();
    new_data.into()
}

#[derive(Clone, PartialEq, Message)]
struct ExecuteResponse {
    #[prost(bytes, tag = "1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}

// empty return if no data present in original
fn execute_response(data: Option<Binary>) -> Option<Binary> {
    data.map(|d| {
        let exec_data = ExecuteResponse { data: d.to_vec() };
        let mut new_data = Vec::<u8>::with_capacity(exec_data.encoded_len());
        // the data must encode successfully
        exec_data.encode(&mut new_data).unwrap();
        new_data.into()
    })
}
