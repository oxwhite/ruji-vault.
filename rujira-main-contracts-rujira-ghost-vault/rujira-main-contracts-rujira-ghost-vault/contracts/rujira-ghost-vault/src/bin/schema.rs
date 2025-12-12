use cosmwasm_schema::write_api;

use rujira_rs::ghost::vault;

fn main() {
    write_api! {
        instantiate: vault::InstantiateMsg,
        execute: vault::ExecuteMsg,
        query: vault::QueryMsg,
        sudo: vault::SudoMsg,
    }
}
