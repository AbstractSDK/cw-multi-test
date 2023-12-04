fn main() {
    test().unwrap()
}
use std::path::Path;

use anyhow::Result as AnyResult;
use cosmwasm_std::{from_json, Addr, Binary, Deps, Empty, Env};
use counter::msg::{ExecuteMsg, GetCountResponse, QueryMsg};
use cw_multi_test::{
    addons::{MockAddressGenerator, MockApiBech32},
    wasm_emulation::{channel::RemoteChannel, contract::WasmContract},
    App, AppBuilder, BankKeeper, ContractWrapper, Executor, WasmKeeper,
};
use cw_orch_networks::networks::PHOENIX_1;
use tokio::runtime::Runtime;

mod counter;

fn query(deps: Deps, env: Env, msg: Vec<u8>) -> AnyResult<Binary> {
    counter::contract::query(deps, env, from_json(msg)?).map_err(Into::into)
}

pub const SENDER: &str = "terra17c6ts8grcfrgquhj3haclg44le8s7qkx6l2yx33acguxhpf000xqhnl3je";
fn increment(app: &mut App<BankKeeper, MockApiBech32>, contract: Addr) -> AnyResult<()> {
    let sender = Addr::unchecked(SENDER);
    app.execute_contract(
        sender.clone(),
        contract.clone(),
        &ExecuteMsg::Increment {},
        &[],
    )?;
    Ok(())
}

fn count(app: &App<BankKeeper, MockApiBech32>, contract: Addr) -> AnyResult<GetCountResponse> {
    Ok(app
        .wrap()
        .query_wasm_smart(contract.clone(), &QueryMsg::GetCount {})?)
}

fn raw_cousin_count(
    app: &App<BankKeeper, MockApiBech32>,
    contract: Addr,
) -> AnyResult<GetCountResponse> {
    Ok(app
        .wrap()
        .query_wasm_smart(contract.clone(), &QueryMsg::GetRawCousinCount {})?)
}

fn cousin_count(
    app: &App<BankKeeper, MockApiBech32>,
    contract: Addr,
) -> AnyResult<GetCountResponse> {
    Ok(app
        .wrap()
        .query_wasm_smart(contract.clone(), &QueryMsg::GetCousinCount {})?)
}

fn test() -> AnyResult<()> {
    env_logger::init();
    let rust_contract = ContractWrapper::new(
        counter::contract::execute,
        counter::contract::instantiate,
        counter::contract::query,
    );

    let code = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("artifacts")
            .join("counter_contract_with_cousin.wasm"),
    )
    .unwrap();
    let wasm_contract = WasmContract::new_local(code);

    let runtime = Runtime::new()?;
    let chain = PHOENIX_1;
    let remote_channel = RemoteChannel::new(&runtime, chain.clone())?;

    let wasm = WasmKeeper::<Empty, Empty>::new()
        .with_remote(remote_channel.clone())
        .with_address_generator(MockAddressGenerator);

    let bank = BankKeeper::new().with_remote(remote_channel.clone());

    // First we instantiate a new app
    let mut app = AppBuilder::default()
        .with_wasm(wasm)
        .with_bank(bank)
        .with_remote(remote_channel)
        .with_api(MockApiBech32::new(chain.network_info.pub_address_prefix))
        .build(|_, _, _| {})?;

    let sender = Addr::unchecked(SENDER);
    let rust_code_id = app.store_code((Box::new(rust_contract), query));
    let wasm_code_id = app.store_wasm_code(wasm_contract);

    let counter_rust = app
        .instantiate_contract(
            rust_code_id,
            sender.clone(),
            &counter::msg::InstantiateMsg { count: 1 },
            &[],
            "cousin-counter",
            Some(sender.to_string()),
        )
        .unwrap();

    let counter_wasm = app
        .instantiate_contract(
            wasm_code_id,
            sender.clone(),
            &counter::msg::InstantiateMsg { count: 1 },
            &[],
            "cousin-counter",
            Some(sender.to_string()),
        )
        .unwrap();

    println!("Rust contract {}", counter_rust);
    println!("Wasm contract {}", counter_wasm);

    app.execute_contract(
        sender.clone(),
        counter_rust.clone(),
        &ExecuteMsg::SetCousin {
            cousin: counter_wasm.to_string(),
        },
        &[],
    )?;

    app.execute_contract(
        sender.clone(),
        counter_wasm.clone(),
        &ExecuteMsg::SetCousin {
            cousin: counter_rust.to_string(),
        },
        &[],
    )?;

    // Increment the count on both and see what's what
    increment(&mut app, counter_rust.clone())?;
    increment(&mut app, counter_rust.clone())?;
    increment(&mut app, counter_wasm.clone())?;

    // Assert the count
    assert_eq!(count(&app, counter_rust.clone())?.count, 3);
    assert_eq!(count(&app, counter_wasm.clone())?.count, 2);

    // Assert the raw cousin count
    assert_eq!(raw_cousin_count(&app, counter_rust.clone())?.count, 2);
    assert_eq!(raw_cousin_count(&app, counter_wasm.clone())?.count, 3);

    // Assert the cousin count
    assert_eq!(cousin_count(&app, counter_rust.clone())?.count, 2);
    assert_eq!(cousin_count(&app, counter_wasm.clone())?.count, 3);

    Ok(())
}
