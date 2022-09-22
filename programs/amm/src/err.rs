use crate::prelude::*;
use std::fmt::Display;

#[error_code]
pub enum AmmError {
    #[msg("Operation would result in an overflow")]
    MathOverflow,
    /// Use this error via the [`acc`] function to provide more background
    /// about the issue.
    #[msg("Provided account breaks some constraints, see logs for more info")]
    InvalidAccountInput,
    /// Use this error via the [`arg`] function to provide more background
    /// about the issue.
    #[msg("One of the provided input arguments is invalid")]
    InvalidArg,
    #[msg(
        "Given amount of tokens to swap would result in \
        less than minimum requested tokens to receive"
    )]
    SlippageExceeded,
    /// Use this error for program paths which should never be reached if the
    /// program logic works as intended.
    #[msg("There's a bug in the program, see logs for more info")]
    InvariantViolation,
    /// Use this error whenever trying to interact with a pool, but providing
    /// wrong token mints
    #[msg("Provided mints are not available on the pool")]
    InvalidTokenMints,
    #[msg("Invalid lp token amount to burn")]
    InvalidLpTokenAmount,
}

pub fn acc(msg: impl Display) -> AmmError {
    msg!("[InvalidAccountInput] {}", msg);

    AmmError::InvalidAccountInput
}

pub fn arg(msg: impl Display) -> AmmError {
    msg!("[InvalidArg] {}", msg);

    AmmError::InvalidArg
}
