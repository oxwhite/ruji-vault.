use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, Decimal, StdResult, Storage};
use cw_storage_plus::Item;
use rujira_rs::ghost::vault::{InstantiateMsg, Interest};

use crate::ContractError;

static CONFIG: Item<Config> = Item::new("config");

#[cw_serde]
pub struct Config {
    pub denom: String,
    pub interest: Interest,
    pub fee: Decimal,
    pub fee_address: Addr,
}

impl Config {
    pub fn new(api: &dyn Api, value: InstantiateMsg) -> StdResult<Self> {
        Ok(Self {
            denom: value.denom,
            interest: value.interest,
            fee: value.fee,
            fee_address: api.addr_validate(value.fee_address.as_str())?,
        })
    }
}

impl Config {
    pub fn load(storage: &dyn Storage) -> StdResult<Self> {
        CONFIG.load(storage)
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        if self.fee >= Decimal::one() {
            return Err(ContractError::Invalid("config.fee".to_string()));
        }

        Ok(self.interest.validate()?)
    }

    pub fn save(&self, storage: &mut dyn Storage) -> StdResult<()> {
        CONFIG.save(storage, self)
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::Decimal;

    use super::*;

    #[test]
    fn validation() {
        Config {
            denom: "btc".to_string(),
            interest: Interest {
                target_utilization: Decimal::from_ratio(8u128, 10u128),
                base_rate: Decimal::from_ratio(3u128, 10000u128),
                step1: Decimal::from_ratio(8u128, 10u128),
                step2: Decimal::from_ratio(3u128, 1u128),
            },
            fee: Decimal::zero(),
            fee_address: Addr::unchecked("addr0000000000000000000000000000000000000000"),
        }
        .validate()
        .unwrap();
    }
}
