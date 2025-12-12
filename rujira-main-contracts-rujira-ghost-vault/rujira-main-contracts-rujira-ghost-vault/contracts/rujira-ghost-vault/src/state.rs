use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, Decimal256, Env, StdResult, Storage, Timestamp, Uint128};
use cw_storage_plus::Item;
use rujira_rs::{ghost::vault::Interest, DecimalScaled, SharePool, SharePoolError};
use std::ops::{Add, Mul, Sub};

use crate::{config::Config, ContractError};

static STATE: Item<State> = Item::new("state");

#[cw_serde]
pub struct State {
    pub last_updated: Timestamp,

    // Pools representing the ownership of debt, and ownership of deposits + interest
    // Calculated interest is charged on the debt_pool, and the same amount allocated
    // to the deposit_pool. As membership of each grows and shrinks (ie through
    // borrows & repays for debt_pool, and deposits & withdraws for deposit_pool),
    // the effective rate earned by depositors will vary
    pub debt_pool: SharePool,
    pub deposit_pool: SharePool,
    #[serde(default)]
    pub pending_interest: DecimalScaled,
    #[serde(default)]
    pub pending_fees: DecimalScaled,
}

impl State {
    pub fn init(storage: &mut dyn Storage, env: &Env) -> StdResult<()> {
        STATE.save(
            storage,
            &Self {
                last_updated: env.block.time,
                debt_pool: SharePool::default(),
                deposit_pool: SharePool::default(),
                pending_interest: DecimalScaled::zero(),
                pending_fees: DecimalScaled::zero(),
            },
        )?;

        Ok(())
    }

    pub fn load(storage: &dyn Storage) -> StdResult<Self> {
        STATE.load(storage)
    }

    pub fn save(&self, storage: &mut dyn Storage) -> StdResult<()> {
        STATE.save(storage, self)
    }

    pub fn deposit(&mut self, amount: Uint128) -> Result<Uint128, ContractError> {
        Ok(self.deposit_pool.join(amount)?)
    }//*called by execute() in contract.rs

    pub fn withdraw(&mut self, amount: Uint128) -> Result<Uint128, ContractError> {
        let withdrawn = self.deposit_pool.leave(amount)?;
        Ok(withdrawn)
    }//*called by execute()

    pub fn borrow(&mut self, amount: Uint128) -> Result<Uint128, ContractError> {
        Ok(self.debt_pool.join(amount)?)
    }//*caled by execute()

    pub fn repay(&mut self, amount: Uint128) -> Result<Uint128, ContractError> {
        if amount.gt(&self.debt_pool.size()) {
            return Err(ContractError::ExcessiveRepay {
                debt: self.debt_pool.size(),
                repaid: amount,
            });
        }
        // Calculate the amount of shares that this repay will burn
        let shares = amount.multiply_ratio(self.debt_pool.shares(), self.debt_pool.size());
        self.debt_pool.leave(shares)?;
        Ok(shares)
    } //*called by execute()

    pub fn utilization(&self) -> Decimal {
        // We consider accrued interest and debt in the utilization rate
        if self.deposit_pool.size().is_zero() {
            Decimal::zero()
        } else {
            Decimal::one()
                - Decimal::from_ratio(
                    // We use the debt pool size to determine utilization
                    self.deposit_pool.size().sub(self.debt_pool.size()),
                    self.deposit_pool.size(),
                )
        }
    }

    pub fn debt_rate(&self, interest: &Interest) -> StdResult<Decimal> {
        interest.rate(self.utilization())
    }

    pub fn lend_rate(&self, interest: &Interest) -> StdResult<Decimal> {
        Ok(interest.rate(self.utilization())? * self.utilization())
    }

    pub fn calculate_interest(
        &mut self,
        interest: &Interest,
        to: Timestamp,
        fee_rate: Decimal,
    ) -> Result<(Uint128, Uint128), ContractError> {
        let rate = Decimal256::from(self.debt_rate(interest)?);
        let seconds = to.seconds().sub(self.last_updated.seconds());
        let part = Decimal256::from_ratio(seconds, 31_536_000u128);

        let interest_decimal = Decimal256::from_ratio(self.debt_pool.size(), 1u128)
            .mul(rate)
            .mul(part);

        // add pending_interest to interest
        let interest_scaled = DecimalScaled::from(interest_decimal);

        // collect the fee for the protocol
        let fee_rate_scaled = DecimalScaled::from(Decimal256::from(fee_rate));
        // add the fee to the pending fees
        let fee_accrued = interest_scaled.mul(fee_rate_scaled);

        // net interest for the users
        let net_interest = interest_scaled.sub(fee_accrued).add(self.pending_interest);

        // add the fee to the pending fees
        let fee_total = fee_accrued.add(self.pending_fees);

        // decompose fee_total and net_interest
        let (fee, fee_frac) = fee_total.decompose();
        let (interest, interest_frac) = net_interest.decompose();

        // persist pendings
        self.pending_fees = fee_frac;
        self.pending_interest = interest_frac;

        Ok((Uint128::try_from(interest)?, Uint128::try_from(fee)?))
    }

    pub fn distribute_interest(
        &mut self,
        env: &Env,
        config: &Config,
    ) -> Result<Uint128, ContractError> {
        // Calculate interest charged on total debt since last update
        let (interest, mut fee) =
            self.calculate_interest(&config.interest, env.block.time, config.fee)?;
        let mut shares = Uint128::zero();

        // deposit the protocol fee to the deposit pool to issue shares
        match self.deposit_pool.join(fee) {
            Ok(amount) => {
                shares = amount;
            }
            // if no shares were issued, add the fee to the pending fees for later distribution
            // set the fee to 0 so that the debt is not charged with the fee yet
            Err(SharePoolError::Zero(_)) => {
                self.pending_fees = self.pending_fees.add(DecimalScaled::from_ratio(fee, 1u128));
                fee = Uint128::zero();
            }
            Err(err) => return Err(err.into()),
        }

        // Allocate the interest to the deposit pool
        self.deposit_pool.deposit(interest)?;
        // Charge the interest to the debt pool, so that outstanding debt tokens are required to
        // pay this interest on return
        self.debt_pool.deposit(interest.add(fee))?;
        self.last_updated = env.block.time;

        Ok(shares)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{testing::mock_env, Decimal};
    use rujira_rs::{ghost::vault::Interest, DecimalScaled};

    #[test]
    fn test_distribute_interest_no_mint_path() {
        let env = mock_env();
        let mut storage = cosmwasm_std::testing::MockStorage::new();
        State::init(&mut storage, &env).unwrap();
        let mut state = State::load(&storage).unwrap();

        let config = Config {
            denom: "test".to_string(),
            interest: Interest {
                target_utilization: Decimal::from_ratio(8u128, 10u128),
                base_rate: Decimal::from_ratio(1u128, 1000000u128), // 0.0001% per year
                step1: Decimal::from_ratio(20u128, 100u128),
                step2: Decimal::from_ratio(100u128, 100u128),
            },
            fee: Decimal::from_ratio(1u128, 10u128), // 10% fee
            fee_address: cosmwasm_std::Addr::unchecked("fee_addr"),
        };

        // Deposit 1000, borrow 800
        state.deposit(Uint128::new(1000)).unwrap();
        state.borrow(Uint128::new(800)).unwrap();

        // Wait 1 second
        let mut env = mock_env();
        env.block.time = state.last_updated.plus_seconds(1);

        // Distribute interest
        let shares = state.distribute_interest(&env, &config).unwrap();

        // No shares minted (fee_int = 0)
        assert_eq!(shares, Uint128::zero());

        // Pool sizes unchanged (net_int = 0)
        assert_eq!(state.deposit_pool.size(), Uint128::new(1000));
        assert_eq!(state.debt_pool.size(), Uint128::new(800));

        // All interest in pending amounts
        assert_eq!(
            state.pending_interest,
            DecimalScaled::from_ratio(45662328766017u128, 10u128.pow(19))
        );
        assert_eq!(
            state.pending_fees,
            DecimalScaled::from_ratio(5073592085113u128, 10u128.pow(19))
        );
    }

    #[test]
    fn test_distribute_interest_mint_path() {
        let env = mock_env();
        let mut storage = cosmwasm_std::testing::MockStorage::new();
        State::init(&mut storage, &env).unwrap();
        let mut state = State::load(&storage).unwrap();

        let config = Config {
            denom: "test".to_string(),
            interest: Interest {
                target_utilization: Decimal::from_ratio(8u128, 10u128),
                base_rate: Decimal::from_ratio(10u128, 100u128), // 10% base rate
                step1: Decimal::from_ratio(20u128, 100u128),
                step2: Decimal::from_ratio(100u128, 100u128),
            },
            fee: Decimal::from_ratio(1u128, 10u128), // 10% fee
            fee_address: cosmwasm_std::Addr::unchecked("fee_addr"),
        };

        // Deposit 1000, borrow 800
        state.deposit(Uint128::new(1000)).unwrap();
        state.borrow(Uint128::new(800)).unwrap();

        // Wait 1 year
        let mut env = mock_env();
        env.block.time = state.last_updated.plus_seconds(31_536_000);

        // Distribute interest
        let shares = state.distribute_interest(&env, &config).unwrap();

        // 24 shares minted for protocol fee
        assert_eq!(shares, Uint128::new(24));

        // Pool sizes: net_interest(216) + fee(24) = 240 total
        assert_eq!(state.deposit_pool.size(), Uint128::new(1240));
        assert_eq!(state.debt_pool.size(), Uint128::new(1040));

        // No pending amounts (exact integer calculations)
        assert_eq!(state.pending_interest, DecimalScaled::zero());
        assert_eq!(state.pending_fees, DecimalScaled::zero());

        // Both pools grow by 240
        assert_eq!(state.deposit_pool.size().u128() - 1000, 240);
        assert_eq!(state.debt_pool.size().u128() - 800, 240);
    }
}
