// use abstract_core::ans_host::AssetsResponse;
// use abstract_core::objects::pool_id::PoolAddressBase;
// use abstract_core::objects::PoolMetadata;
// use cosmwasm_std::coins;
// use cosmwasm_std::Timestamp;
// use cosmwasm_std::Uint128;
// use cw_orch::daemon::queriers::Node;
// use cw_orch_daemon::DaemonQuerier;
// use tokio::runtime::Runtime;

// use abstract_core::adapter::AuthorizedAddressesResponse;
// use abstract_core::adapter::BaseQueryMsg;
// use cw_multi_test::wasm_emulation::storage::analyzer::StorageAnalyzer;

// use abstract_core::adapter::AdapterRequestMsg;
// use abstract_core::adapter::BaseInstantiateMsg;
// use abstract_core::objects::module::Module;
// use abstract_core::objects::module_reference::ModuleReference;
// use abstract_core::objects::AnsAsset;
// use abstract_core::objects::AssetEntry;
// use abstract_dex_adapter::msg::InstantiateMsg;
// use abstract_dex_adapter::msg::{DexAction, DexExecuteMsg, DexInstantiateMsg};
// use cosmwasm_std::Decimal;
// use cw_multi_test::wasm_emulation::contract::WasmContract;
// use cw_orch_daemon::CosmWasm;
// use std::path::Path;

// use cw_asset::AssetInfo;

// use abstract_core::version_control::{self};

// use abstract_core::objects::module::ModuleInfo;
// use abstract_core::objects::module::ModuleVersion;
// use cosmwasm_std::{to_binary, Addr};
// use cw_multi_test::AppBuilder;
// use cw_multi_test::BankKeeper;
// use cw_multi_test::Executor;
// use cw_multi_test::FailingModule;

// use abstract_interface::get_account_contracts;
// use abstract_interface::Abstract;
// use abstract_interface::ManagerQueryFns;
// use cosmwasm_std::Empty;
// use cw_multi_test::WasmKeeper;
// use cw_orch::daemon::Daemon;
// use cw_orch::deploy::Deploy;

// use cw_orch_daemon::prelude::ContractInstance;
// use dotenv::dotenv;

// use abstract_core::manager::ExecuteMsg;

// // Abstract patch

// #[cosmwasm_schema::cw_serde]
// pub struct ModulesResponse {
//     pub modules: Vec<Module>,
// }

// fn main() {
//     dotenv().ok();
//     let runtime = tokio::runtime::Runtime::new().unwrap();

//     let mut chain = cw_orch::daemon::networks::JUNO_1;
//     chain.grpc_urls = &["http://juno-grpc.polkachu.com:12690"];

//     let daemon = Daemon::builder()
//         .chain(chain.clone())
//         .handle(runtime.handle())
//         .build()
//         .unwrap();

//     let abstract_ = Abstract::load_from(daemon.clone()).unwrap();

//     // Query an account, its owner and install a module for

//     let (manager, proxy) = get_account_contracts(&abstract_.version_control, Some(1));

//     let ownership = manager.ownership().unwrap();

//     let owner = ownership.owner.unwrap();
//     // We use this owner to install and uninstall a module
//     let owner_addr = Addr::unchecked(owner.clone());

//     env_logger::init();
//     let mut wasm = WasmKeeper::<Empty, Empty>::new();
//     wasm.set_chain(chain.clone().into());

//     let mut bank = BankKeeper::new();
//     bank.set_chain(chain.clone().into());

//     let node_querier = daemon.query_client::<Node>();
//     let block = runtime.block_on(node_querier.latest_block()).unwrap();

//     // First we instantiate a new app
//     let app = AppBuilder::default()
//         .with_wasm::<FailingModule<Empty, Empty, Empty>, _>(wasm)
//         .with_bank(bank)
//         .with_block(cosmwasm_std::BlockInfo {
//             height: block.header.height.into(),
//             time: Timestamp::from_seconds(block.header.time.unix_timestamp().try_into().unwrap()),
//             chain_id: block.header.chain_id.to_string(),
//         });
//     let mut app = app.build(|_, _, _| {});

//     log::info!("Built App Environment");

//     // We need to register a pool pairing on the ans host
//     app.execute_contract(
//         Addr::unchecked(owner.clone()),
//         abstract_.ans_host.address().unwrap(),
//         &abstract_core::ans_host::ExecuteMsg::UpdateDexes {
//             to_add: vec!["wyndex".to_string()],
//             to_remove: vec![],
//         },
//         &[],
//     )
//     .unwrap();

//     app.execute_contract(
//         Addr::unchecked(owner.clone()),
//         abstract_.ans_host.address().unwrap(),
//         &abstract_core::ans_host::ExecuteMsg::UpdatePools {
//             to_add: vec![(
//                 PoolAddressBase::contract(
//                     "juno1gqy6rzary8vwnslmdavqre6jdhakcd4n2z4r803ajjmdq08r66hq7zcwrj".to_string(),
//                 ),
//                 PoolMetadata {
//                     dex: "wyndex".to_string(),
//                     pool_type: abstract_core::objects::PoolType::ConstantProduct,
//                     assets: vec!["axelar>usdc".into(), "juno>juno".into()],
//                 },
//             )],
//             to_remove: vec![],
//         },
//         &[],
//     )
//     .unwrap();
//     // End registering the pool pairing

//     // test
//     let rt = Runtime::new().unwrap();
//     let test = rt
//         .block_on(
//             CosmWasm::new(daemon.channel()).contract_raw_state(
//                 "juno13q8rv8w9ew5cn6wecr2p4scegzu9nac0hv2dx807l4vz60h0ldns0ksvz0",
//                 hex::decode(
//                     "0008706f6f6c5f69647300096a756e6f3e6a756e6f00096a756e6f3e77796e6477796e646578",
//                 )
//                 .unwrap(),
//             ),
//         )
//         .unwrap();
//     log::info!("{:x?}", test);

//     // We deploy the adapter :
//     // 1. upload the code
//     let code = std::fs::read(
//         Path::new(env!("CARGO_MANIFEST_DIR"))
//             .join("artifacts")
//             .join("abstract_dex_adapter-juno.wasm"),
//     )
//     .unwrap();
//     let dex_code = WasmContract::new_local(code, chain.clone());
//     let code_id = app.store_code(dex_code);

//     // 2. Instantiate the code
//     let dex_addr = app
//         .instantiate_contract(
//             code_id,
//             owner_addr.clone(),
//             &InstantiateMsg {
//                 module: DexInstantiateMsg {
//                     swap_fee: Decimal::percent(1),
//                     recipient_account: 0,
//                 },
//                 base: BaseInstantiateMsg {
//                     ans_host_address: abstract_.ans_host.address().unwrap().to_string(),
//                     version_control_address: abstract_
//                         .version_control
//                         .address()
//                         .unwrap()
//                         .to_string(),
//                 },
//             },
//             &[],
//             "Dex adapter",
//             None,
//         )
//         .unwrap();

//     log::info!("Instantiated Dex adapter");

//     // 3. Register the adapter in version control
//     let module =
//         ModuleInfo::from_id("abstract:dex", ModuleVersion::Version("0.17.1".to_string())).unwrap();
//     app.execute_contract(
//         owner_addr.clone(),
//         abstract_.version_control.address().unwrap(),
//         &version_control::ExecuteMsg::ProposeModules {
//             modules: vec![(module.clone(), ModuleReference::Adapter(dex_addr.clone()))],
//         },
//         &[],
//     )
//     .unwrap();

//     log::info!("Proposed and registered Dex adapter");
//     // We install the module on the account
//     app.execute_contract(
//         Addr::unchecked(owner.clone()),
//         manager.address().unwrap(),
//         &ExecuteMsg::InstallModule {
//             init_msg: Some(to_binary(&Empty {}).unwrap()),
//             module,
//         },
//         &[],
//     )
//     .unwrap();

//     log::info!("Installed Dex adapter");
//     // Let's see what registered in the ans as assets

//     /*
//         let assets = abstract_.ans_host.asset_list(None, Some(30), Some("juno>future".to_string())).unwrap();
//         log::info!("{:?}", assets);

//         let pools = abstract_.ans_host.pool_list(None, Some(30), None).unwrap();
//         log::info!("{:?}", pools);
//     */
//     // We need to get the authorized addresses on the adapter

//     /*
//         app.execute_contract(Addr::unchecked(owner.clone()), manager.address().unwrap(),&ExecuteMsg::ExecOnModule { module_id: "abstract:dex".to_string(),
//             exec_msg: to_binary(&abstract_dex_adapter::msg::ExecuteMsg::Base(
//                 abstract_core::adapter::BaseExecuteMsg::UpdateAuthorizedAddresses {
//                     to_add: vec![manager.address().unwrap().to_string()],
//                     to_remove: vec![]
//                 }
//             )).unwrap()}, &[]).unwrap();
//     */
//     log::info!("Updated authorized address on the Dex adapter");
//     /* Query to verify that the manager was authorized to execute on the adapter */
//     let addresses: AuthorizedAddressesResponse = app
//         .wrap()
//         .query_wasm_smart(
//             dex_addr,
//             &abstract_dex_adapter::msg::QueryMsg::Base(BaseQueryMsg::AuthorizedAddresses {
//                 proxy_address: proxy.address().unwrap().to_string(),
//             }),
//         )
//         .unwrap();
//     log::info!("AuthorizedAddresses on dex {:?}", addresses);

//     let analysis = StorageAnalyzer::new(&app).unwrap();
//     analysis.compare_all_readable_contract_storage(chain.into());
//     //log::info!("analysis, dex {:x?}", analysis.get_contract_storage(dex_addr));
//     /* End query */
//     log::info!("Some queries to check everything is alright");

//     // We deposit funds on the proxy

//     app.execute(
//         owner_addr,
//         cosmwasm_std::CosmosMsg::Bank(cosmwasm_std::BankMsg::Send {
//             to_address: proxy.address().unwrap().to_string(),
//             amount: coins(100_000u128, "ujuno"),
//         }),
//     )
//     .unwrap();

//     // We get the balances (for asserting)
//     let usdc: AssetsResponse = app
//         .wrap()
//         .query_wasm_smart(
//             abstract_.ans_host.address().unwrap(),
//             &abstract_core::ans_host::QueryMsg::Assets {
//                 names: vec!["axelar>usdc".to_string()],
//             },
//         )
//         .unwrap();
//     let usdc = match &usdc.assets[0].1 {
//         AssetInfo::Native(denom) => denom,
//         _ => panic!("Expected native denom"),
//     };
//     let old_balance = app
//         .wrap()
//         .query_balance(proxy.address().unwrap(), "ujuno")
//         .unwrap();
//     let old_usdc_balance = app
//         .wrap()
//         .query_balance(proxy.address().unwrap(), usdc)
//         .unwrap();

//     // We test a swap interaction
//     app.execute_contract(
//         Addr::unchecked(owner),
//         manager.address().unwrap(),
//         &ExecuteMsg::ExecOnModule {
//             module_id: "abstract:dex".to_string(),
//             exec_msg: to_binary(&abstract_dex_adapter::msg::ExecuteMsg::Module(
//                 AdapterRequestMsg {
//                     proxy_address: None,
//                     request: DexExecuteMsg::Action {
//                         action: DexAction::Swap {
//                             ask_asset: AssetEntry::new("axelar>usdc"),
//                             offer_asset: AnsAsset::new(AssetEntry::new("juno>juno"), 100_000u128),
//                             belief_price: None,
//                             max_spread: None,
//                         },
//                         dex: "wyndex".to_string(),
//                     },
//                 },
//             ))
//             .unwrap(),
//         },
//         &[],
//     )
//     .unwrap();
//     log::info!("Execute the swap");

//     // We get the juno balance (should be lower)
//     let new_balance = app
//         .wrap()
//         .query_balance(proxy.address().unwrap(), "ujuno")
//         .unwrap();
//     assert_eq!(
//         old_balance.amount - new_balance.amount,
//         Uint128::from(100_000u128)
//     );

//     let new_usdc_balance = app
//         .wrap()
//         .query_balance(proxy.address().unwrap(), usdc)
//         .unwrap();
//     assert!(old_usdc_balance.amount < new_usdc_balance.amount);
// }

fn main() {}
