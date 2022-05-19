use crate::prelude::*;

#[error_code]
pub enum AmmError {
    #[msg("Operation would result in an overflow")]
    MathOverflow,
}
