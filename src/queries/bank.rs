use std::str::FromStr;

use anyhow::Result as AnyResult;
use cosmwasm_std::{Addr, Coin, Uint128};

use crate::wasm_emulation::channel::RemoteChannel;

pub struct BankRemoteQuerier;

impl BankRemoteQuerier {
    pub fn get_balance(remote: RemoteChannel, account: &Addr) -> AnyResult<Vec<Coin>> {
        let querier = cw_orch::daemon::queriers::Bank {
            channel: remote.channel,
            rt_handle: Some(remote.rt.clone()),
        };
        let distant_amounts: Vec<Coin> = remote
            .rt
            .block_on(querier._balance(account, None))
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
