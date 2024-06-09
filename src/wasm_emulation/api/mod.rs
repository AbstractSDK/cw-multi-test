use crate::wasm_emulation::query::gas::{GAS_COST_CANONICALIZE, GAS_COST_HUMANIZE};
use bech32::{FromBase32, ToBase32, Variant};
use cosmwasm_std::Addr;
use cosmwasm_vm::{BackendApi, BackendError, BackendResult, GasInfo};

const SHORT_CANON_LEN: usize = 20;
const LONG_CANON_LEN: usize = 32;

pub fn bytes_from_bech32(address: &str, prefix: &str) -> Result<Vec<u8>, BackendError> {
    if address.is_empty() {
        return Err(BackendError::Unknown {
            msg: "empty address string is not allowed".to_string(),
        });
    }

    let (hrp, data, _variant) = bech32::decode(address).map_err(|e| BackendError::Unknown {
        msg: format!("Invalid Bech32 address : Err {}", e),
    })?;
    if hrp != prefix {
        return Err(BackendError::Unknown {
            msg: format!("invalid Bech32 prefix; expected {}, got {}", prefix, hrp),
        });
    }

    Ok(Vec::<u8>::from_base32(&data).unwrap())
}

pub const MAX_PREFIX_CHARS: usize = 10;
// Prefixes are limited to MAX_PREFIX_CHARS chars
// This allows one to specify a string prefix and still implement Copy
#[derive(Clone, Copy)]
pub struct RealApi {
    pub prefix: [char; MAX_PREFIX_CHARS],
}

impl RealApi {
    pub fn new(prefix: &str) -> RealApi {
        if prefix.len() > MAX_PREFIX_CHARS {
            panic!("More chars in the prefix than {}", MAX_PREFIX_CHARS);
        }

        let mut api_prefix = ['\0'; 10];
        for (i, c) in prefix.chars().enumerate() {
            api_prefix[i] = c;
        }
        Self { prefix: api_prefix }
    }

    pub fn get_prefix(&self) -> String {
        let mut prefix = Vec::new();

        for &c in self.prefix.iter() {
            if c != '\0' {
                prefix.push(c);
            }
        }
        prefix.iter().collect()
    }

    pub fn next_address(&self, count: usize) -> Addr {
        let mut canon = format!("ADDRESS_{}", count).as_bytes().to_vec();
        canon.resize(SHORT_CANON_LEN, 0);
        Addr::unchecked(self.addr_humanize(&canon).0.unwrap())
    }

    pub fn next_contract_address(&self, count: usize) -> Addr {
        let mut canon = format!("CONTRACT_{}", count).as_bytes().to_vec();
        canon.resize(LONG_CANON_LEN, 0);
        Addr::unchecked(self.addr_humanize(&canon).0.unwrap())
    }
}
macro_rules! unwrap_or_return_with_gas {
    ($result: expr $(,)?, $gas_total: expr $(,)?) => {{
        let result: core::result::Result<_, _> = $result; // just a type check
        let gas: GasInfo = $gas_total; // just a type check
        match result {
            Ok(v) => v,
            Err(e) => return (Err(e), gas),
        }
    }};
}

impl BackendApi for RealApi {
    fn addr_validate(&self, input: &str) -> BackendResult<()> {
        let mut gas_total = GasInfo {
            cost: 0,
            externally_used: 0,
        };

        let (canonicalize_res, gas_info) = self.addr_canonicalize(input);
        gas_total += gas_info;
        let canonical = unwrap_or_return_with_gas!(canonicalize_res, gas_total);

        let (humanize_res, gas_info) = self.addr_humanize(&canonical);
        gas_total += gas_info;
        let normalized = unwrap_or_return_with_gas!(humanize_res, gas_total);
        if input != normalized.as_str() {
            return (
                Err(BackendError::user_err(
                    "Invalid input: address not normalized",
                )),
                gas_total,
            );
        }
        (Ok(()), gas_total)
    }

    fn addr_canonicalize(&self, human: &str) -> BackendResult<Vec<u8>> {
        let gas_cost = GasInfo::with_externally_used(GAS_COST_CANONICALIZE);
        if human.trim().is_empty() {
            return (
                Err(BackendError::Unknown {
                    msg: "empty address string is not allowed".to_string(),
                }),
                gas_cost,
            );
        }

        (bytes_from_bech32(human, &self.get_prefix()), gas_cost)
    }
    fn addr_humanize(&self, canonical: &[u8]) -> BackendResult<String> {
        let gas_cost = GasInfo::with_externally_used(GAS_COST_HUMANIZE);

        if canonical.len() != SHORT_CANON_LEN && canonical.len() != LONG_CANON_LEN {
            return (
                Err(BackendError::Unknown {
                    msg: "Canon address doesn't have the right length".to_string(),
                }),
                gas_cost,
            );
        }

        if canonical.is_empty() {
            return (Ok("".to_string()), gas_cost);
        }

        let human = bech32::encode(&self.get_prefix(), canonical.to_base32(), Variant::Bech32)
            .map_err(|e| BackendError::Unknown { msg: e.to_string() });

        (human, gas_cost)
    }
}

#[cfg(test)]
mod test {
    use super::RealApi;

    #[test]
    fn prefix() {
        let prefix = "migaloo";

        let api = RealApi::new(prefix);

        let final_prefix = api.get_prefix();
        assert_eq!(prefix, final_prefix);
    }

    #[test]
    #[should_panic]
    fn too_long_prefix() {
        let prefix = "migaloowithotherchars";
        RealApi::new(prefix);
    }
}
