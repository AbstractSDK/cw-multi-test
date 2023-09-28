// Bank
pub const GAS_COST_BALANCE_QUERY: u64 = 1000;
pub const GAS_COST_ALL_BALANCE_QUERY: u64 = 10000;

// Staking
pub const GAS_COST_BONDED_DENOM: u64 = 100;
pub const GAS_COST_ALL_VALIDATORS: u64 = 10000;
pub const GAS_COST_VALIDATOR: u64 = 1000;
pub const GAS_COST_ALL_DELEGATIONS: u64 = 10000;
pub const GAS_COST_DELEGATIONS: u64 = 1000;

// Wasm
pub const GAS_COST_CONTRACT_INFO: u64 = 1000;
pub const GAS_COST_RAW_COSMWASM_QUERY: u64 = 10000;

// ERROR
pub const GAS_COST_QUERY_ERROR: u64 = 1000;

// API (from cosmwasm_vm directly)
pub const GAS_COST_HUMANIZE: u64 = 44;
pub const GAS_COST_CANONICALIZE: u64 = 55;
