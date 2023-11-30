use cosmwasm_std::{to_json_binary, Deps, QueryRequest, StdResult, WasmQuery};

use crate::counter::{
    msg::{GetCountResponse, QueryMsg},
    state::STATE,
};

pub fn count(deps: Deps) -> StdResult<GetCountResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(GetCountResponse { count: state.count })
}

pub fn cousin_count(deps: Deps) -> StdResult<GetCountResponse> {
    let state = STATE.load(deps.storage)?;
    let cousin_count: GetCountResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: state.cousin.unwrap().to_string(),
            msg: to_json_binary(&QueryMsg::GetCount {})?,
        }))?;
    Ok(cousin_count)
}

pub fn raw_cousin_count(deps: Deps) -> StdResult<GetCountResponse> {
    let state = STATE.load(deps.storage)?;
    let cousin_state = STATE.query(&deps.querier, state.cousin.unwrap())?;
    Ok(GetCountResponse {
        count: cousin_state.count,
    })
}
