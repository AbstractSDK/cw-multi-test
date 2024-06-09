use crate::wasm_emulation::query::gas::{
    GAS_COST_ALL_DELEGATIONS, GAS_COST_ALL_VALIDATORS, GAS_COST_BONDED_DENOM, GAS_COST_DELEGATIONS,
    GAS_COST_VALIDATOR,
};
use crate::wasm_emulation::query::mock_querier::QueryResultWithGas;
use cosmwasm_std::Binary;
use cosmwasm_vm::GasInfo;

use cosmwasm_std::to_json_binary;
use cosmwasm_std::{
    AllDelegationsResponse, AllValidatorsResponse, BondedDenomResponse, DelegationResponse,
    FullDelegation, StakingQuery, Validator, ValidatorResponse,
};
use cosmwasm_std::{ContractResult, SystemResult};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct StakingQuerier {
    denom: String,
    validators: Vec<Validator>,
    delegations: Vec<FullDelegation>,
}

impl StakingQuerier {
    pub fn new(denom: &str, validators: &[Validator], delegations: &[FullDelegation]) -> Self {
        StakingQuerier {
            denom: denom.to_string(),
            validators: validators.to_vec(),
            delegations: delegations.to_vec(),
        }
    }

    pub fn query(&self, request: &StakingQuery) -> QueryResultWithGas {
        let contract_result: ContractResult<Binary> = match request {
            StakingQuery::BondedDenom {} => {
                let res = BondedDenomResponse::new(self.denom.clone());
                to_json_binary(&res).into()
            }
            StakingQuery::AllValidators {} => {
                let res = AllValidatorsResponse::new(self.validators.clone());
                to_json_binary(&res).into()
            }
            StakingQuery::Validator { address } => {
                let validator: Option<Validator> = self
                    .validators
                    .iter()
                    .find(|validator| validator.address == *address)
                    .cloned();
                let res = ValidatorResponse::new(validator);
                to_json_binary(&res).into()
            }
            StakingQuery::AllDelegations { delegator } => {
                let delegations: Vec<_> = self
                    .delegations
                    .iter()
                    .filter(|d| d.delegator.as_str() == delegator)
                    .cloned()
                    .map(|d| d.into())
                    .collect();
                let res = AllDelegationsResponse::new(delegations);
                to_json_binary(&res).into()
            }
            StakingQuery::Delegation {
                delegator,
                validator,
            } => {
                let delegation = self
                    .delegations
                    .iter()
                    .find(|d| d.delegator.as_str() == delegator && d.validator == *validator);
                let res = DelegationResponse::new(delegation.cloned());
                to_json_binary(&res).into()
            }
            &_ => panic!("Not implemented {:?}", request),
        };

        // We handle the gas_info
        let gas_info = match request {
            StakingQuery::BondedDenom { .. } => GAS_COST_BONDED_DENOM,
            StakingQuery::AllValidators { .. } => GAS_COST_ALL_VALIDATORS,
            StakingQuery::Validator { .. } => GAS_COST_VALIDATOR,
            StakingQuery::AllDelegations { .. } => GAS_COST_ALL_DELEGATIONS,
            StakingQuery::Delegation { .. } => GAS_COST_DELEGATIONS,
            &_ => panic!("Not implemented {:?}", request),
        };

        // system result is always ok in the mock implementation
        (
            SystemResult::Ok(contract_result),
            GasInfo::with_externally_used(gas_info),
        )
    }
}
