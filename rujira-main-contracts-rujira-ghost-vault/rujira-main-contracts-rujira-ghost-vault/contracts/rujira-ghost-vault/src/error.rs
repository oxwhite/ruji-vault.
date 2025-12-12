use cosmwasm_std::{
    CheckedFromRatioError, ConversionOverflowError, OverflowError, StdError, Uint128,
};
use cw_utils::PaymentError;
use rujira_rs::SharePoolError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("{0}")]
    CheckedFromRatio(#[from] CheckedFromRatioError),

    #[error("{0}")]
    ConversionOverflow(#[from] ConversionOverflowError),

    #[error("{0}")]
    SharePool(#[from] SharePoolError),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("UnauthorizedBorrower")]
    UnauthorizedBorrower {},

    #[error("BorrowLimitReached {limit}")]
    BorrowLimitReached { limit: Uint128 },

    #[error("InsufficientRepay debt {debt} value {value} repaid {repaid}")]
    InsufficientRepay {
        debt: Uint128,
        value: Uint128,
        repaid: Uint128,
    },

    #[error("ExcessiveRepay debt {debt} repaid {repaid}")]
    ExcessiveRepay { debt: Uint128, repaid: Uint128 },

    #[error("Invalid: {0}")]
    Invalid(String),
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
