//! Basic app to use the IbcSimpleModule
use cosmwasm_std::{
    testing::{MockApi, MockStorage},
    Api, Empty, Storage,
};

use crate::{
    App, AppBuilder, BankKeeper, DistributionKeeper, FailingModule, GovFailingModule, Router,
    StakeKeeper, StargateFailingModule, WasmKeeper,
};

use super::IbcSimpleModule;

/// A type alias for the default-built App. It simplifies storage and handling in typical scenarios,
/// streamlining the use of the App structure in standard test setups.
pub type IbcApp<ExecC = Empty, QueryC = Empty> = App<
    BankKeeper,
    MockApi,
    MockStorage,
    FailingModule<ExecC, QueryC, Empty>,
    WasmKeeper<ExecC, QueryC>,
    StakeKeeper,
    DistributionKeeper,
    IbcSimpleModule,
    GovFailingModule,
    StargateFailingModule,
>;

impl IbcApp {
    /// Creates new default `App` implementation working with Empty custom messages.
    pub fn new_ibc<F>(init_fn: F) -> Self
    where
        F: FnOnce(
            &mut Router<
                BankKeeper,
                FailingModule<Empty, Empty, Empty>,
                WasmKeeper<Empty, Empty>,
                StakeKeeper,
                DistributionKeeper,
                IbcSimpleModule,
                GovFailingModule,
                StargateFailingModule,
            >,
            &dyn Api,
            &mut dyn Storage,
        ),
    {
        AppBuilder::new().with_ibc(IbcSimpleModule).build(init_fn)
    }
}
