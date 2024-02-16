use cosmwasm_std::{DepsMut, MessageInfo, Response};

use crate::counter::{error::*, state::*};

pub fn increment(deps: DepsMut) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.count += 1;
        Ok(state)
    })?;

    Ok(Response::new().add_attribute("action", "increment"))
}

pub fn reset(deps: DepsMut, info: MessageInfo, count: i32) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        if info.sender != state.owner {
            return Err(ContractError::Unauthorized {});
        }
        state.count = count;
        Ok(state)
    })?;
    Ok(Response::new().add_attribute("action", "reset"))
}

pub fn set_cousin(
    deps: DepsMut,
    info: MessageInfo,
    cousin: String,
) -> Result<Response, ContractError> {
    let cousin_addr = deps.api.addr_validate(&cousin)?;
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        if info.sender != state.owner {
            return Err(ContractError::Unauthorized {});
        }
        state.cousin = Some(cousin_addr);
        Ok(state)
    })?;
    Ok(Response::new().add_attribute("action", "set_cousin"))
}
