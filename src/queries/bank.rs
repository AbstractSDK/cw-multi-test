use std::str::FromStr;

use anyhow::Result as AnyResult;
use cosmwasm_std::{Addr, Coin, Uint128, Storage, Order};
use cw_orch_daemon::queriers::DaemonQuerier;

use crate::{
    wasm_emulation::{channel::RemoteChannel, input::BankStorage, query::AllQuerier},
    BankKeeper, prefixed_storage::prefixed_read, bank::{NAMESPACE_BANK, BALANCES},
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
