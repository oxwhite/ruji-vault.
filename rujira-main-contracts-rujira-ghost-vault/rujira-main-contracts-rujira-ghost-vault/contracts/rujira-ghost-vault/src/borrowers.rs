use crate::ContractError;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Order, StdError, StdResult, Storage, Uint128};
use cw_storage_plus::{Bound, Map};
use rujira_rs::SharePool;
use std::{
    cmp::min,
    collections::HashMap,
    ops::{Add, Sub},
};

static BORROWERS: Map<Addr, Borrower> = Map::new("borrowers");
// Delegated shares for a borrower
static DELEGATE_SHARES: Map<(Addr, Addr), Uint128> = Map::new("delegates");

#[cw_serde]
pub struct Borrower {
    pub addr: Addr,
    pub limit: Uint128,
    pub shares: Uint128,
}

impl Borrower {
    pub fn load(storage: &dyn Storage, addr: Addr) -> Result<Self, ContractError> {
        match BORROWERS.load(storage, addr.clone()) {
            Ok(x) => Ok(x),
            Err(StdError::NotFound { .. }) => Err(ContractError::UnauthorizedBorrower {}),
            Err(err) => Err(ContractError::Std(err)),
        }
    }

    pub fn save(&self, storage: &mut dyn Storage) -> StdResult<()> {
        BORROWERS.save(storage, self.addr.clone(), self)
    }

    pub fn delegate_shares(&self, storage: &dyn Storage, delegate: Addr) -> Uint128 {
        DELEGATE_SHARES
            .load(storage, (self.addr.clone(), delegate))
            .unwrap_or_default()
    }

    pub fn delegate_borrow(
        &mut self,
        storage: &mut dyn Storage,
        delegate: Addr,
        pool: &SharePool,
        shares: Uint128,
    ) -> Result<(), ContractError> {
        DELEGATE_SHARES.update(
            storage,
            (self.addr.clone(), delegate),
            |v| -> Result<Uint128, ContractError> { Ok(v.unwrap_or_default().add(shares)) },
        )?;
        self.borrow(storage, pool, shares)
    }

    pub fn borrow(
        &mut self,
        storage: &mut dyn Storage,
        pool: &SharePool,
        shares: Uint128,
    ) -> Result<(), ContractError> {
        if pool.ownership(self.shares.add(shares)).gt(&self.limit) {
            return Err(ContractError::BorrowLimitReached { limit: self.limit });
        }
        self.shares += shares;
        Ok(self.save(storage)?)
    }

    pub fn repay(
        &mut self,
        storage: &mut dyn Storage,
        shares: Uint128,
    ) -> Result<Uint128, ContractError> {
        let repaid = min(shares, self.shares);
        self.shares -= repaid;
        self.save(storage)?;
        Ok(shares.sub(repaid))
    }

    pub fn delegate_repay(
        &mut self,
        storage: &mut dyn Storage,
        delegate: Addr,
        shares: Uint128,
    ) -> Result<Uint128, ContractError> {
        let k = (self.addr.clone(), delegate);
        let delegate = DELEGATE_SHARES.load(storage, k.clone())?;
        let repaid = min(shares, delegate);
        DELEGATE_SHARES.save(storage, k, &delegate.checked_sub(repaid)?)?;
        self.repay(storage, repaid)?;
        Ok(shares.sub(repaid))
    }

    pub fn set(storage: &mut dyn Storage, addr: Addr, limit: Uint128) -> StdResult<()> {
        let mut borrower = BORROWERS.load(storage, addr.clone()).unwrap_or(Borrower {
            addr: addr.clone(),
            limit: Default::default(),
            shares: Default::default(),
        });
        borrower.limit = limit;
        BORROWERS.save(storage, addr, &borrower)
    }

    pub fn list(
        storage: &dyn Storage,
        limit: Option<u8>,
        start_after: Option<Addr>,
    ) -> impl Iterator<Item = StdResult<Self>> + '_ {
        let limit = limit.unwrap_or(100) as usize;
        let min = start_after.map(Bound::exclusive);
        BORROWERS
            .range(storage, min, None, Order::Ascending)
            .take(limit)
            .map(|x| x.map(|(_, v)| v))
    }
}

// ------------ Migration ------------
#[cw_serde]
pub struct OldDelegate {
    pub borrower: Borrower,
    pub addr: Addr,
    pub shares: Uint128,
}

pub fn migrate(storage: &mut dyn Storage) -> StdResult<()> {
    let old: Map<(Addr, Addr), OldDelegate> = Map::new("delegates");

    // Collect all old delegates first to avoid borrow conflicts
    let old_delegates: Vec<((Addr, Addr), OldDelegate)> = old
        .range(storage, None, None, Order::Ascending)
        .collect::<StdResult<Vec<_>>>()?;

    let mut delegate_shares_by_borrower: HashMap<Addr, Uint128> = HashMap::new();

    for ((borrower_addr, delegate_addr), old_delegate) in old_delegates {
        DELEGATE_SHARES.save(
            storage,
            (borrower_addr.clone(), delegate_addr),
            &old_delegate.shares,
        )?;

        delegate_shares_by_borrower
            .entry(borrower_addr)
            .and_modify(|total| *total += old_delegate.shares)
            .or_insert(old_delegate.shares);
    }

    for (borrower_addr, expected_shares) in delegate_shares_by_borrower {
        let mut borrower = BORROWERS.load(storage, borrower_addr.clone())?;
        borrower.shares = expected_shares;
        borrower.save(storage)?;
    }

    Ok(())
}
