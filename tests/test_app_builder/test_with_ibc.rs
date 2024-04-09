use crate::test_app_builder::{MyKeeper, NO_MESSAGE};
use anyhow::Result as AnyResult;
use cosmwasm_std::{Empty, IbcMsg, IbcQuery, QueryRequest};
use cw_multi_test::ibc::app::IbcApp;
use cw_multi_test::ibc::relayer::{create_channel, create_connection};
use cw_multi_test::{no_init, AppBuilder, Executor, Ibc};

type MyIbcKeeper = MyKeeper<IbcMsg, IbcQuery, Empty>;

impl Ibc for MyIbcKeeper {}

const EXECUTE_MSG: &str = "ibc execute called";
const QUERY_MSG: &str = "ibc query called";

#[test]
fn building_app_with_custom_ibc_should_work() {
    // build custom ibc keeper (no sudo handling for ibc)
    let ibc_keeper = MyIbcKeeper::new(EXECUTE_MSG, QUERY_MSG, NO_MESSAGE);

    // build the application with custom ibc keeper
    let app_builder = AppBuilder::default();
    let mut app = app_builder.with_ibc(ibc_keeper).build(no_init);

    // executing ibc message should return an error defined in custom keeper
    assert_eq!(
        EXECUTE_MSG,
        app.execute(
            app.api().addr_make("sender"),
            IbcMsg::CloseChannel {
                channel_id: "my-channel".to_string()
            }
            .into(),
        )
        .unwrap_err()
        .to_string()
    );

    // executing ibc query should return an error defined in custom keeper
    assert_eq!(
        format!("Generic error: Querier contract error: {}", QUERY_MSG),
        app.wrap()
            .query::<IbcQuery>(&QueryRequest::Ibc(IbcQuery::ListChannels {
                port_id: Some("my-port".to_string())
            }))
            .unwrap_err()
            .to_string()
    );
}

#[test]
fn create_channel_should_work_with_basic_app() -> AnyResult<()> {
    let mut app1 = IbcApp::new_ibc(no_init);
    let mut app2 = IbcApp::new_ibc(no_init);

    let (src_connection_id, _dst_connection) = create_connection(&mut app1, &mut app2)?;

    create_channel(
        &mut app1,
        &mut app2,
        src_connection_id,
        "transfer".to_string(),
        "transfer".to_string(),
        "ics20-1".to_string(),
        cosmwasm_std::IbcOrder::Unordered,
    )?;

    Ok(())
}
