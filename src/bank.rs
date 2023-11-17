use crate::wasm_emulation::input::BankStorage;
use cw_orch_daemon::queriers::DaemonQuerier;
use std::str::FromStr;

use anyhow::{bail, Result as AnyResult};
use itertools::Itertools;
use schemars::JsonSchema;

use cosmwasm_std::{
    coin, to_binary, Addr, AllBalanceResponse, Api, BalanceResponse, BankMsg, BankQuery, Binary,
    BlockInfo, Coin, Event, Order, Querier, StdResult, Storage, SupplyResponse, Uint128,
};
use cw_storage_plus::Map;
use cw_utils::NativeBalance;

use crate::app::CosmosRouter;
use crate::executor::AppResponse;
use crate::module::Module;
use crate::prefixed_storage::{prefixed, prefixed_read};

use crate::wasm_emulation::channel::RemoteChannel;
use crate::wasm_emulation::query::AllQuerier;

const BALANCES: Map<&Addr, NativeBalance> = Map::new("balances");

pub const NAMESPACE_BANK: &[u8] = b"bank";

#[derive(Clone, std::fmt::Debug, PartialEq, Eq, JsonSchema)]
pub enum BankSudo {
    Mint {
        to_address: String,
        amount: Vec<Coin>,
    },
}

pub trait Bank: Module<ExecT = BankMsg, QueryT = BankQuery, SudoT = BankSudo> + AllQuerier {}

#[derive(Default)]
pub struct BankKeeper {
    remote: Option<RemoteChannel>,
}

impl BankKeeper {
    pub fn new() -> Self {
        BankKeeper::default()
    }

    pub fn set_remote(&mut self, remote: RemoteChannel) {
        self.remote = Some(remote);
    }

    // this is an "admin" function to let us adjust bank accounts in genesis
    pub fn init_balance(
        &self,
        storage: &mut dyn Storage,
        account: &Addr,
        amount: Vec<Coin>,
    ) -> AnyResult<()> {
        let mut bank_storage = prefixed(storage, NAMESPACE_BANK);
        self.set_balance(&mut bank_storage, account, amount)
    }

    // this is an "admin" function to let us adjust bank accounts
    fn set_balance(
        &self,
        bank_storage: &mut dyn Storage,
        account: &Addr,
        amount: Vec<Coin>,
    ) -> AnyResult<()> {
        let mut balance = NativeBalance(amount);
        balance.normalize();
        BALANCES
            .save(bank_storage, account, &balance)
            .map_err(Into::into)
    }

    fn get_balance(&self, bank_storage: &dyn Storage, account: &Addr) -> AnyResult<Vec<Coin>> {
        // If there is no balance present, we query it on the distant chain
        if !BALANCES.has(bank_storage, account) {
            let channel = self.remote.clone().unwrap().channel;
            let querier = cw_orch_daemon::queriers::Bank::new(channel);
            let distant_amounts: Vec<Coin> = self
                .remote
                .clone()
                .unwrap()
                .rt
                .block_on(querier.balance(account, None))
                .map(|result| {
                    result
                        .into_iter()
                        .map(|c| Coin {
                            amount: Uint128::from_str(&c.amount).unwrap(),
                            denom: c.denom,
                        })
                        .collect()
                })
                .unwrap();
            Ok(distant_amounts)
        } else {
            let val = BALANCES.may_load(bank_storage, account)?;
            Ok(val.unwrap_or_default().into_vec())
        }
    }

    fn get_supply(&self, bank_storage: &dyn Storage, denom: String) -> AnyResult<Coin> {
        let supply: Uint128 = BALANCES
            .range(bank_storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<_>>>()?
            .into_iter()
            .map(|a| a.1)
            .fold(Uint128::zero(), |accum, item| {
                let mut subtotal = Uint128::zero();
                for coin in item.into_vec() {
                    if coin.denom == denom {
                        subtotal += coin.amount;
                    }
                }
                accum + subtotal
            });
        Ok(coin(supply.into(), denom))
    }

    fn send(
        &self,
        bank_storage: &mut dyn Storage,
        from_address: Addr,
        to_address: Addr,
        amount: Vec<Coin>,
    ) -> AnyResult<()> {
        self.burn(bank_storage, from_address, amount.clone())?;
        self.mint(bank_storage, to_address, amount)
    }

    fn mint(
        &self,
        bank_storage: &mut dyn Storage,
        to_address: Addr,
        amount: Vec<Coin>,
    ) -> AnyResult<()> {
        let amount = self.normalize_amount(amount)?;
        let b = self.get_balance(bank_storage, &to_address)?;
        let b = NativeBalance(b) + NativeBalance(amount);
        self.set_balance(bank_storage, &to_address, b.into_vec())
    }

    fn burn(
        &self,
        bank_storage: &mut dyn Storage,
        from_address: Addr,
        amount: Vec<Coin>,
    ) -> AnyResult<()> {
        let amount = self.normalize_amount(amount)?;
        let a = self.get_balance(bank_storage, &from_address)?;
        let a = (NativeBalance(a) - amount)?;
        self.set_balance(bank_storage, &from_address, a.into_vec())
    }

    /// Filters out all 0 value coins and returns an error if the resulting Vec is empty
    fn normalize_amount(&self, amount: Vec<Coin>) -> AnyResult<Vec<Coin>> {
        let res: Vec<_> = amount.into_iter().filter(|x| !x.amount.is_zero()).collect();
        if res.is_empty() {
            bail!("Cannot transfer empty coins amount")
        } else {
            Ok(res)
        }
    }
}

fn coins_to_string(coins: &[Coin]) -> String {
    coins
        .iter()
        .map(|c| format!("{}{}", c.amount, c.denom))
        .join(",")
}

impl Bank for BankKeeper {}

impl AllQuerier for BankKeeper {
    type Output = BankStorage;
    fn query_all(&self, storage: &dyn Storage) -> AnyResult<BankStorage> {
        let bank_storage = prefixed_read(storage, NAMESPACE_BANK);
        let balances: Result<Vec<_>, _> = BALANCES
            .range(&bank_storage, None, None, Order::Ascending)
            .collect();
        Ok(BankStorage { storage: balances? })
    }
}

impl Module for BankKeeper {
    type ExecT = BankMsg;
    type QueryT = BankQuery;
    type SudoT = BankSudo;

    fn execute<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        sender: Addr,
        msg: BankMsg,
    ) -> AnyResult<AppResponse> {
        let mut bank_storage = prefixed(storage, NAMESPACE_BANK);
        match msg {
            BankMsg::Send { to_address, amount } => {
                // see https://github.com/cosmos/cosmos-sdk/blob/v0.42.7/x/bank/keeper/send.go#L142-L147
                let events = vec![Event::new("transfer")
                    .add_attribute("recipient", &to_address)
                    .add_attribute("sender", &sender)
                    .add_attribute("amount", coins_to_string(&amount))];
                self.send(
                    &mut bank_storage,
                    sender,
                    Addr::unchecked(to_address),
                    amount,
                )?;
                Ok(AppResponse { events, data: None })
            }
            BankMsg::Burn { amount } => {
                // burn doesn't seem to emit any events
                self.burn(&mut bank_storage, sender, amount)?;
                Ok(AppResponse::default())
            }
            m => bail!("Unsupported bank message: {:?}", m),
        }
    }

    fn sudo<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        msg: BankSudo,
    ) -> AnyResult<AppResponse> {
        let mut bank_storage = prefixed(storage, NAMESPACE_BANK);
        match msg {
            BankSudo::Mint { to_address, amount } => {
                let to_address = api.addr_validate(&to_address)?;
                self.mint(&mut bank_storage, to_address, amount)?;
                Ok(AppResponse::default())
            }
        }
    }

    fn query(
        &self,
        api: &dyn Api,
        storage: &dyn Storage,
        _querier: &dyn Querier,
        _block: &BlockInfo,
        request: BankQuery,
    ) -> AnyResult<Binary> {
        let bank_storage = prefixed_read(storage, NAMESPACE_BANK);
        match request {
            BankQuery::AllBalances { address } => {
                let address = api.addr_validate(&address)?;
                let amount = self.get_balance(&bank_storage, &address)?;
                let res = AllBalanceResponse { amount };
                Ok(to_binary(&res)?)
            }
            BankQuery::Balance { address, denom } => {
                let address = api.addr_validate(&address)?;
                let all_amounts = self.get_balance(&bank_storage, &address)?;
                let amount = all_amounts
                    .into_iter()
                    .find(|c| c.denom == denom)
                    .unwrap_or_else(|| coin(0, denom));
                let res = BalanceResponse { amount };
                Ok(to_binary(&res)?)
            }
            BankQuery::Supply { denom } => {
                let amount = self.get_supply(&bank_storage, denom)?;
                let mut res = SupplyResponse::default();
                res.amount = amount;
                Ok(to_binary(&res)?)
            }
            q => bail!("Unsupported bank query: {:?}", q),
        }
    }
}
