use anyhow::Result as AnyResult;
use cosmwasm_std::Addr;
use cw20::AllAccountsResponse;
use cw20::Cw20ExecuteMsg;
use cw_multi_test::Executor;

use cw20::Cw20QueryMsg;
use cw_multi_test::wasm_emulation::channel::RemoteChannel;
use cw_multi_test::AppBuilder;
use cw_multi_test::BankKeeper;
use cw_orch::daemon::networks::PHOENIX_1;

use cw_multi_test::WasmKeeper;

use cosmwasm_std::Empty;
use cw_orch::daemon::GrpcChannel;
use cw_orch::environment::ChainInfoOwned;
use tokio::runtime::Handle;
use tokio::runtime::Runtime;
use tonic::transport::Channel;

pub fn main() {
    test().unwrap()
}
fn get_channel(chain: impl Into<ChainInfoOwned>, rt: Handle) -> AnyResult<Channel> {
    let chain = chain.into();
    let channel = rt.block_on(GrpcChannel::connect(&chain.grpc_urls, &chain.chain_id))?;
    Ok(channel)
}

pub fn test() -> anyhow::Result<()> {
    env_logger::init();

    let runtime = Runtime::new()?;
    let chain = PHOENIX_1;
    let remote_channel = RemoteChannel::new(
        &runtime,
        get_channel(chain.clone(), runtime.handle().clone())?,
        chain.network_info.pub_address_prefix,
    )?;
    let wasm = WasmKeeper::<Empty, Empty>::new().with_remote(remote_channel.clone());

    let bank = BankKeeper::new().with_remote(remote_channel.clone());

    // First we instantiate a new app
    let mut app = AppBuilder::default()
        .with_wasm(wasm)
        .with_bank(bank)
        .with_remote(remote_channel)
        .build(|_, _, _| {})?;

    // Then we send a message to the blockchain through the app
    let sender = "terra17c6ts8grcfrgquhj3haclg44le8s7qkx6l2yx33acguxhpf000xqhnl3je";
    let recipient = "terra1e9lqmv3egtgps9nux04vw8gd4pr3qp9h00y8um";
    let contract_addr = "terra1lxx40s29qvkrcj8fsa3yzyehy7w50umdvvnls2r830rys6lu2zns63eelv";
    let query = "terra1e8lqmv3egtgps9nux04vw8gd4pr3qp9h00y7um";

    let response: AllAccountsResponse = app.wrap().query_wasm_smart(
        contract_addr,
        &Cw20QueryMsg::AllAccounts {
            start_after: Some(query.to_string()),
            limit: Some(30),
        },
    )?;
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
    )?;

    // We query to verify the state changed
    let response: AllAccountsResponse = app.wrap().query_wasm_smart(
        contract_addr,
        &Cw20QueryMsg::AllAccounts {
            start_after: Some(query.to_string()),
            limit: Some(30),
        },
    )?;
    log::info!("After transfer : {:?}", response);

    Ok(())
}
