// use ibc_chain_registry::chain::ChainData;
// use std::path::Path;

// use cosmwasm_schema::{cw_serde, QueryResponses};
// use cosmwasm_std::Empty;

// use cw_multi_test::wasm_emulation::contract::WasmContract;
// use cw_multi_test::wasm_emulation::storage::analyzer::StorageAnalyzer;
// use cw_multi_test::AppBuilder;
// use cw_multi_test::Executor;
// use cw_multi_test::FailingModule;
// use cw_multi_test::WasmKeeper;

// use cw_orch_daemon::networks::PHOENIX_1;

// #[cw_serde]
// pub struct InstantiateMsg {
//     pub count: i32,
// }

// // ANCHOR: exec_msg
// #[cw_serde]
// #[cfg_attr(feature = "interface", derive(cw_orch::ExecuteFns))] // Function generation
// pub enum ExecuteMsg {
//     Increment {},
//     IncrementAndQuery {},
//     SetCousin { addr: String },
//     Reset { count: i32 },
// }
// // ANCHOR_END: exec_msg

// // ANCHOR: query_msg
// #[cw_serde]
// #[cfg_attr(feature = "interface", derive(cw_orch::QueryFns))] // Function generation
// #[derive(QueryResponses)]
// pub enum QueryMsg {
//     // GetCount returns the current count as a json-encoded number
//     #[returns(GetCountResponse)]
//     GetCount {},
//     // GetCount returns the current count of the cousin contract
//     #[returns(GetCountResponse)]
//     GetCousinCount {},
// }

// // Custom response for the query
// #[cw_serde]
// pub struct GetCountResponse {
//     pub count: i32,
// }
// #[cw_serde]
// pub struct GetCousinCountResponse {
//     pub raw: i32,
//     pub smart: i32,
// }
// // ANCHOR_END: query_msg

// #[cw_serde]
// pub struct MigrateMsg {
//     pub t: String,
// }

// pub fn main() {
//     env_logger::init();

//     let chain: ChainData = PHOENIX_1.into();

//     let mut wasm = WasmKeeper::<Empty, Empty>::new();
//     wasm.set_chain(chain.clone());

//     // First we instantiate a new app
//     let app = AppBuilder::default()
//         .with_chain(chain.clone())
//         .with_wasm::<FailingModule<Empty, Empty, Empty>, _>(wasm);
//     let mut app = app.build(|_, _, _| {});

//     // Then we send a message to the blockchain through the app
//     let sender = app.next_address();

//     let code = std::fs::read(
//         Path::new(env!("CARGO_MANIFEST_DIR"))
//             .join("artifacts")
//             .join("counter_contract.wasm"),
//     )
//     .unwrap();
//     let counter_contract = WasmContract::new_local(code, chain.clone());

//     let code_id = app.store_code(counter_contract);

//     let counter1 = app
//         .instantiate_contract(
//             code_id,
//             sender.clone(),
//             &InstantiateMsg { count: 1 },
//             &[],
//             "cousin-counter",
//             Some(sender.to_string()),
//         )
//         .unwrap();
//     let counter2 = app
//         .instantiate_contract(
//             code_id,
//             sender.clone(),
//             &InstantiateMsg { count: 1 },
//             &[],
//             "cousin-counter",
//             Some(sender.to_string()),
//         )
//         .unwrap();

//     app.execute_contract(
//         sender.clone(),
//         counter1.clone(),
//         &ExecuteMsg::Increment {},
//         &[],
//     )
//     .unwrap();
//     app.execute_contract(
//         sender.clone(),
//         counter1.clone(),
//         &ExecuteMsg::Increment {},
//         &[],
//     )
//     .unwrap();
//     app.execute_contract(
//         sender.clone(),
//         counter2.clone(),
//         &ExecuteMsg::Increment {},
//         &[],
//     )
//     .unwrap();

//     app.execute_contract(
//         sender.clone(),
//         counter1.clone(),
//         &ExecuteMsg::SetCousin {
//             addr: counter2.to_string(),
//         },
//         &[],
//     )
//     .unwrap();
//     app.execute_contract(
//         sender,
//         counter2.clone(),
//         &ExecuteMsg::SetCousin {
//             addr: counter1.to_string(),
//         },
//         &[],
//     )
//     .unwrap();

//     let cousin_count: GetCousinCountResponse = app
//         .wrap()
//         .query_wasm_smart(counter2.clone(), &QueryMsg::GetCousinCount {})
//         .unwrap();
//     assert_eq!(cousin_count.raw, cousin_count.smart);
//     assert_eq!(cousin_count.raw, 3);

//     let cousin_count: GetCousinCountResponse = app
//         .wrap()
//         .query_wasm_smart(counter1.clone(), &QueryMsg::GetCousinCount {})
//         .unwrap();
//     assert_eq!(cousin_count.raw, cousin_count.smart);
//     assert_eq!(cousin_count.raw, 2);

//     // Analyze the storage

//     let analysis = StorageAnalyzer::new(&app, chain).unwrap();

//     log::info!(
//         "analysis, wasm1 {:?}",
//         analysis.get_contract_storage(counter1.clone())
//     );
//     log::info!("analysis, wasm1 {:?}", analysis.readable_storage(counter1));
//     log::info!(
//         "analysis, wasm2 {:?}",
//         analysis.get_contract_storage(counter2.clone())
//     );
//     log::info!("analysis, wasm2 {:?}", analysis.readable_storage(counter2));
//     log::info!(
//         "All contracts storage {:?}",
//         analysis.all_readable_contract_storage()
//     );
//     analysis.compare_all_readable_contract_storage();
// }

fn main(){
    
}
