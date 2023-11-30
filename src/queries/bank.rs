use std::str::FromStr;

use anyhow::Result as AnyResult;
use cosmwasm_std::{Addr, Coin, Order, Storage, Uint128};
use cw_orch_daemon::queriers::DaemonQuerier;

use crate::{
    bank::{BALANCES, NAMESPACE_BANK},
    prefixed_storage::prefixed_read,
    wasm_emulation::{channel::RemoteChannel, input::BankStorage},
    BankKeeper,
};

pub struct BankRemoteQuerier;

impl BankRemoteQuerier {
    pub fn get_balance(remote: RemoteChannel, account: &Addr) -> AnyResult<Vec<Coin>> {
        let channel = remote.channel;
        let querier = cw_orch_daemon::queriers::Bank::new(channel);
        let distant_amounts: Vec<Coin> = remote
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
    }
}
