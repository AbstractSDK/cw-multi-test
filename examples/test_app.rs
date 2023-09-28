use cosmwasm_std::Addr;
use cw20::AllAccountsResponse;
use cw20::Cw20ExecuteMsg;
use cw_multi_test::Executor;

use cw20::Cw20QueryMsg;
use cw_multi_test::AppBuilder;
use cw_multi_test::BankKeeper;
use cw_multi_test::FailingModule;
use cw_orch_daemon::networks::PHOENIX_1;

use cw_multi_test::WasmKeeper;

use cosmwasm_std::Empty;

pub fn main() {
    env_logger::init();
    let mut wasm = WasmKeeper::<Empty, Empty>::new();
    wasm.set_chain(PHOENIX_1.into());

    let mut bank = BankKeeper::new();
    bank.set_chain(PHOENIX_1.into());

    // First we instantiate a new app
    let app = AppBuilder::default()
        .with_wasm::<FailingModule<Empty, Empty, Empty>, _>(wasm)
        .with_bank(bank);
    let mut app = app.build(|_, _, _| {});

    // Then we send a message to the blockchain through the app
    let sender = "terra17c6ts8grcfrgquhj3haclg44le8s7qkx6l2yx33acguxhpf000xqhnl3je";
    let recipient = "terra1e9lqmv3egtgps9nux04vw8gd4pr3qp9h00y8um";
    let contract_addr = "terra1lxx40s29qvkrcj8fsa3yzyehy7w50umdvvnls2r830rys6lu2zns63eelv";
    let query = "terra1e8lqmv3egtgps9nux04vw8gd4pr3qp9h00y7um";

    let response: AllAccountsResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr,
            &Cw20QueryMsg::AllAccounts {
                start_after: Some(query.to_string()),
                limit: Some(30),
            },
        )
        .unwrap();
    log::info!("Before transfer : {:?}", response);

    // We execute a transfer
    app.execute_contract(
        Addr::unchecked(sender),
        Addr::unchecked(contract_addr),
        &Cw20ExecuteMsg::Transfer {
            recipient: recipient.to_string(),
            amount: 1_000_000u128.into(),
        },
        &[],
    )
    .unwrap();

    // We query to verify the state changed
    let response: AllAccountsResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr,
            &Cw20QueryMsg::AllAccounts {
                start_after: Some(query.to_string()),
                limit: Some(30),
            },
        )
        .unwrap();
    log::info!("After transfer : {:?}", response);
}
