use cosmwasm_std::{Addr, Event, Uint128};

pub fn event_deposit(owner: Addr, amount: Uint128, shares: Uint128) -> Event {
    Event::new(format!("{}/deposit", env!("CARGO_PKG_NAME")))
        .add_attribute("owner", owner)
        .add_attribute("amount", amount)
        .add_attribute("shares", shares)
}

pub fn event_withdraw(owner: Addr, amount: Uint128, shares: Uint128) -> Event {
    Event::new(format!("{}/withdraw", env!("CARGO_PKG_NAME")))
        .add_attribute("owner", owner)
        .add_attribute("amount", amount)
        .add_attribute("shares", shares)
}

pub fn event_borrow(
    borrower: Addr,
    delegate: Option<String>,
    amount: Uint128,
    shares: Uint128,
) -> Event {
    Event::new(format!("{}/borrow", env!("CARGO_PKG_NAME")))
        .add_attribute("borrower", borrower)
        .add_attribute("delegate", delegate.unwrap_or_default())
        .add_attribute("amount", amount)
        .add_attribute("shares", shares)
}

pub fn event_repay(
    borrower: Addr,
    delegate: Option<String>,
    amount: Uint128,
    shares: Uint128,
) -> Event {
    Event::new(format!("{}/repay", env!("CARGO_PKG_NAME")))
        .add_attribute("borrower", borrower)
        .add_attribute("delegate", delegate.unwrap_or_default())
        .add_attribute("amount", amount)
        .add_attribute("shares", shares)
}
