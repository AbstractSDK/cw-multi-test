use cosmwasm_schema::cw_serde;
use cosmwasm_schema::QueryResponses;
use cosmwasm_std::coins;
use cosmwasm_std::Addr;
use cosmwasm_std::BlockInfo;
use cosmwasm_std::ContractInfoResponse;
use cosmwasm_std::QueryRequest;
use cosmwasm_std::WasmQuery;
use cw20::BalanceResponse;
use cw_multi_test::addons::MockAddressGenerator;
use cw_multi_test::addons::MockApiBech32;
use cw_multi_test::wasm_emulation::channel::RemoteChannel;
use cw_multi_test::wasm_emulation::contract::WasmContract;
use cw_multi_test::wasm_emulation::storage::analyzer::StorageAnalyzer;
use cw_multi_test::BankKeeper;
use cw_multi_test::Executor;
use cw_orch_daemon::queriers::DaemonQuerier;
use cw_orch_daemon::queriers::Node;
use std::path::Path;
use tokio::runtime::Runtime;

use cw20::Cw20QueryMsg;
use cw_multi_test::AppBuilder;
use cw_orch_networks::networks::PHOENIX_1;

use cosmwasm_std::Empty;
use cw_multi_test::WasmKeeper;
use moneymarket::market::ExecuteMsg;

/// COUNTER CONTRACT MSGs
#[cw_serde]
#[cfg_attr(feature = "interface", derive(cw_orch::QueryFns))] // Function generation
#[derive(QueryResponses)]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    #[returns(GetCountResponse)]
    GetCount {},
    // GetCount returns the current count of the cousin contract
    #[returns(GetCountResponse)]
    GetCousinCount {},
}

// Custom response for the query
#[cw_serde]
pub struct GetCountResponse {
    pub count: i32,
}

#[cw_serde]
pub struct MigrateMsg {
    pub t: String,
}

#[cw_serde]
pub struct InstantiateMsg {
    pub count: i32,
}

/// END CONTRACT MSGs

pub fn test() -> anyhow::Result<()> {
    env_logger::init();

    let sender = "terra1ytj0hhw39j88qsx4yapsr6ker83jv3aj354gmj";
    let market = "terra1zqlcp3aty4p4rjv96h6qdascdn953v6crhwedu5vddxjnp349upscluex6";
    let currency = "ibc/B3504E092456BA618CC28AC671A71FB08C6CA0FD0BE7C8A5B5A3E2DD933CC9E4";
    let a_currency = "terra1gwdxyqtu75es0x5l6cd9flqhh87zjtj7qdankayyr0vtt7s9w4ssm7ds8m";

    let runtime = Runtime::new()?;
    let chain = PHOENIX_1;
    let remote_channel = RemoteChannel::new(&runtime, chain.clone())?;

    let wasm = WasmKeeper::<Empty, Empty>::new()
        .with_remote(remote_channel.clone())
        .with_address_generator(MockAddressGenerator);

    let bank = BankKeeper::new().with_remote(remote_channel.clone());

    let block = runtime.block_on(Node::new(remote_channel.channel.clone()).block_info())?;
    // First we instantiate a new app
    let app = AppBuilder::default()
        .with_wasm(wasm)
        .with_bank(bank)
        .with_remote(remote_channel.clone())
        .with_block(BlockInfo {
            height: block.height,
            time: block.time,
            chain_id: chain.chain_id.to_string(),
        })
        .with_api(MockApiBech32::new(chain.network_info.pub_address_prefix));
    let mut app = app.build(|_, _, _| {})?;
    // Then we send a message to the blockchain through the app

    // We query to verify the state changed
    let response: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            a_currency,
            &Cw20QueryMsg::Balance {
                address: sender.to_string(),
            },
        )
        .unwrap();
    log::info!("Before deposit : {:?}", response);

    app.execute_contract(
        Addr::unchecked(sender),
        Addr::unchecked(market),
        &ExecuteMsg::DepositStable {},
        &coins(10_000, currency),
    )
    .unwrap();

    // We query to verify the state changed
    let response: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            a_currency,
            &Cw20QueryMsg::Balance {
                address: sender.to_string(),
            },
        )
        .unwrap();
    log::info!("After deposit : {:?}", response);

    // Now we try to migrate the contract

    let code = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("artifacts")
            .join("counter_contract.wasm"),
    )
    .unwrap();
    let counter_contract = WasmContract::new_local(code);

    let code_id = app.store_wasm_code(counter_contract);

    // We try to instantiate a new contract. Should work ok !
    let contract_addr = app.instantiate_contract(
        code_id,
        Addr::unchecked(sender),
        &InstantiateMsg { count: 87 },
        &[],
        "label".to_string(),
        None,
    )?;

    log::info!("New contract address {:?}", contract_addr);

    let contract_info: ContractInfoResponse = app
        .wrap()
        .query(&QueryRequest::Wasm(WasmQuery::ContractInfo {
            contract_addr: market.to_string(),
        }))
        .unwrap();

    app.migrate_contract(
        Addr::unchecked(contract_info.admin.clone().unwrap()),
        Addr::unchecked(market),
        &MigrateMsg { t: "t".to_string() },
        code_id,
    )
    .unwrap();

    // The query count message should error with a specific storage error

    let err = app
        .wrap()
        .query_wasm_smart::<GetCountResponse>(market, &QueryMsg::GetCount {})
        .unwrap_err();

    if !err
        .to_string()
        .contains("counter_contract::state::State not found")
    {
        panic!(
            "Error {} should contain counter_contract::state::State not found",
            err
        );
    }

    // Now we migrate back and deposit again
    app.migrate_contract(
        Addr::unchecked(contract_info.admin.unwrap()),
        Addr::unchecked(market),
        &Empty {},
        contract_info.code_id,
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked(sender),
        Addr::unchecked(market),
        &ExecuteMsg::DepositStable {},
        &coins(10_000, currency),
    )
    .unwrap();

    // We query to verify the state changed
    let response: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            a_currency,
            &Cw20QueryMsg::Balance {
                address: sender.to_string(),
            },
        )
        .unwrap();
    log::info!("After migrate and deposit : {:?}", response);

    let analysis = StorageAnalyzer::new(&app).unwrap();
    log::info!(
        "All contracts storage {:?}",
        analysis.all_readable_contract_storage()
    );

    analysis.compare_all_readable_contract_storage();
    Ok(())
}

fn main() {
    test().unwrap();
}
