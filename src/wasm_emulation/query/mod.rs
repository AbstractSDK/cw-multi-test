pub mod bank;
pub mod mock_querier;
pub mod staking;
pub mod wasm;
use cosmwasm_std::Storage;
use serde::de::DeserializeOwned;

pub use mock_querier::MockQuerier;
pub mod gas;

use anyhow::Result as AnyResult;
use serde::Serialize;

pub trait AllQuerier {
    type Output: Serialize + Clone + DeserializeOwned + Default;
    fn query_all(&self, storage: &dyn Storage) -> AnyResult<Self::Output>;
}
