pub mod pool;

use crate::prelude::*;
pub use pool::*;

#[derive(
    AnchorDeserialize,
    AnchorSerialize,
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
)]
pub struct TokenAmount {
    pub amount: u64,
}

#[derive(
    AnchorDeserialize,
    AnchorSerialize,
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
)]
pub struct Fraction {
    /// 1% = 10_000
    pub permillion: u64,
}

impl TokenAmount {
    pub fn new(amount: u64) -> Self {
        Self { amount }
    }
}
