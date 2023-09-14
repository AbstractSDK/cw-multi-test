use anyhow::Result as AnyResult;
use cosmwasm_std::Empty;
use cw_multi_test::{custom_app, App};

fn ibc_test() -> AnyResult<()> {
    let juno = custom_app::<Empty, Empty, _>(|_, _, _| {});
    let osmosis = custom_app::<Empty, Empty, _>(|_, _, _| {});

    Ok(())
}

fn main() {
    ibc_test().unwrap()
}
