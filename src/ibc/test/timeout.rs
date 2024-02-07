use cosmwasm_std::{
    coin, from_json, to_json_binary, Addr, AllBalanceResponse, BankQuery, CosmosMsg, Empty, IbcMsg,
    IbcOrder, IbcTimeout, IbcTimeoutBlock, Querier, QueryRequest,
};

use crate::{
    ibc::{
        events::TIMEOUT_RECEIVE_PACKET_EVENT,
        relayer::{
            create_channel, create_connection, has_event, relay_packets_in_tx,
            ChannelCreationResult,
        },
        simple_ibc::IbcSimpleModule,
    },
    AppBuilder, Executor,
};

#[test]
fn simple_transfer_timeout() -> anyhow::Result<()> {
    let funds = coin(100_000, "ufund");
    let fund_owner = "owner";
    let fund_recipient = "recipient";

    // We mint some funds to the owner
    let mut app1 = AppBuilder::default()
        .with_ibc(IbcSimpleModule)
        .build(|router, api, storage| {
            router
                .bank
                .init_balance(
                    storage,
                    &api.addr_validate(fund_owner).unwrap(),
                    vec![funds.clone()],
                )
                .unwrap();
        });
    let mut app2 = AppBuilder::default()
        .with_ibc(IbcSimpleModule)
        .build(|_, _, _| {});

    let port1 = "transfer".to_string();
    let port2 = "transfer".to_string();

    let (src_connection_id, _) = create_connection(&mut app1, &mut app2)?;

    // We start by creating channels
    let ChannelCreationResult { src_channel, .. } = create_channel(
        &mut app1,
        &mut app2,
        src_connection_id,
        port1.clone(),
        port2,
        "ics20-1".to_string(),
        IbcOrder::Ordered,
    )?;

    // We send an IBC transfer Cosmos Msg on app 1
    let send_response = app1.execute(
        Addr::unchecked(fund_owner),
        CosmosMsg::Ibc(IbcMsg::Transfer {
            channel_id: src_channel,
            to_address: fund_recipient.to_string(),
            amount: funds.clone(),
            timeout: IbcTimeout::with_block(IbcTimeoutBlock {
                revision: 1,
                height: app2.block_info().height, // this will have the effect of a timeout when relaying the packets
            }),
        }),
    )?;

    // We assert the sender balance is empty !

    // We make sure the balance of the sender hasn't changed in the process
    let balances = app1
        .raw_query(
            to_json_binary(&QueryRequest::<Empty>::Bank(BankQuery::AllBalances {
                address: fund_owner.to_string(),
            }))?
            .as_slice(),
        )
        .into_result()?
        .unwrap();
    let balances: AllBalanceResponse = from_json(balances)?;
    assert!(balances.amount.is_empty());

    // We relaying all packets found in the transaction
    let resp = relay_packets_in_tx(&mut app1, &mut app2, send_response)?;

    // We make sure the response contains a timeout
    assert_eq!(resp.len(), 1);

    assert!(has_event(&resp[0].receive_tx, TIMEOUT_RECEIVE_PACKET_EVENT));

    // We make sure the balance of the recipient has not changed
    let balances = app2
        .raw_query(
            to_json_binary(&QueryRequest::<Empty>::Bank(BankQuery::AllBalances {
                address: fund_recipient.to_string(),
            }))?
            .as_slice(),
        )
        .into_result()?
        .unwrap();
    let balances: AllBalanceResponse = from_json(balances)?;

    // The recipient has exactly no balance, because it has timed out
    assert_eq!(balances.amount.len(), 0);

    // We make sure the balance of the sender hasn't changed in the process
    let balances = app1
        .raw_query(
            to_json_binary(&QueryRequest::<Empty>::Bank(BankQuery::AllBalances {
                address: fund_owner.to_string(),
            }))?
            .as_slice(),
        )
        .into_result()?
        .unwrap();
    let balances: AllBalanceResponse = from_json(balances)?;
    println!("{:?}", balances);
    assert_eq!(balances.amount.len(), 1);
    assert_eq!(balances.amount[0].amount, funds.amount);
    assert_eq!(balances.amount[0].denom, funds.denom);
    Ok(())
}
