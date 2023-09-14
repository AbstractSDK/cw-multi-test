use cosmwasm_std::{Binary, IbcMsg};

use crate::{AppResponse, FailingModule, Ibc, Module};

use super::{types::MockIbcQuery, IbcPacketRelayingMsg};

impl Ibc for FailingModule<IbcMsg, MockIbcQuery, IbcPacketRelayingMsg> {}

pub struct IbcAcceptingModule;

impl Module for IbcAcceptingModule {
    type ExecT = IbcMsg;
    type QueryT = MockIbcQuery;
    type SudoT = IbcPacketRelayingMsg;

    fn execute<ExecC, QueryC>(
        &self,
        _api: &dyn cosmwasm_std::Api,
        _storage: &mut dyn cosmwasm_std::Storage,
        _router: &dyn crate::CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &cosmwasm_std::BlockInfo,
        _sender: cosmwasm_std::Addr,
        _msg: Self::ExecT,
    ) -> anyhow::Result<crate::AppResponse>
    where
        ExecC: std::fmt::Debug
            + Clone
            + PartialEq
            + schemars::JsonSchema
            + serde::de::DeserializeOwned
            + 'static,
        QueryC: cosmwasm_std::CustomQuery + serde::de::DeserializeOwned + 'static,
    {
        Ok(AppResponse::default())
    }

    fn sudo<ExecC, QueryC>(
        &self,
        _api: &dyn cosmwasm_std::Api,
        _storage: &mut dyn cosmwasm_std::Storage,
        _router: &dyn crate::CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &cosmwasm_std::BlockInfo,
        _msg: Self::SudoT,
    ) -> anyhow::Result<crate::AppResponse>
    where
        ExecC: std::fmt::Debug
            + Clone
            + PartialEq
            + schemars::JsonSchema
            + serde::de::DeserializeOwned
            + 'static,
        QueryC: cosmwasm_std::CustomQuery + serde::de::DeserializeOwned + 'static,
    {
        Ok(AppResponse::default())
    }

    fn query(
        &self,
        _api: &dyn cosmwasm_std::Api,
        _storage: &dyn cosmwasm_std::Storage,
        _querier: &dyn cosmwasm_std::Querier,
        _block: &cosmwasm_std::BlockInfo,
        _request: Self::QueryT,
    ) -> anyhow::Result<cosmwasm_std::Binary> {
        Ok(Binary::default())
    }
}

impl Ibc for IbcAcceptingModule {}

#[cfg(test)]
mod test {
    use cosmwasm_std::{Addr, Empty};

    use crate::test_helpers::contracts::stargate;
    use crate::test_helpers::contracts::stargate::ExecMsg;
    use crate::{App, AppBuilder, Executor};

    use super::*;

    #[test]
    fn default_ibc() {
        let mut app = App::default();
        #[cfg(not(feature = "multitest_api_1_0"))]
        let code = app.store_code(stargate::contract());
        #[cfg(feature = "multitest_api_1_0")]
        let code = app.store_code(Addr::unchecked("creator"), stargate::contract());
        let contract = app
            .instantiate_contract(
                code,
                Addr::unchecked("owner"),
                &Empty {},
                &[],
                "contract",
                None,
            )
            .unwrap();

        app.execute_contract(Addr::unchecked("owner"), contract, &ExecMsg::Ibc {}, &[])
            .unwrap_err();
    }

    #[test]
    fn substituting_ibc() {
        let mut app = AppBuilder::new()
            .with_ibc(IbcAcceptingModule)
            .build(|_, _, _| ());
        #[cfg(not(feature = "multitest_api_1_0"))]
        let code = app.store_code(stargate::contract());
        #[cfg(feature = "multitest_api_1_0")]
        let code = app.store_code(Addr::unchecked("creator"), stargate::contract());
        let contract = app
            .instantiate_contract(
                code,
                Addr::unchecked("owner"),
                &Empty {},
                &[],
                "contract",
                None,
            )
            .unwrap();

        app.execute_contract(Addr::unchecked("owner"), contract, &ExecMsg::Ibc {}, &[])
            .unwrap();
    }
}
