pub mod discount;
pub mod pool;
pub mod program_toll;

pub use discount::*;
pub use pool::*;
pub use program_toll::*;

use crate::prelude::*;

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
pub struct Slot {
    pub slot: u64,
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
pub struct Permillion {
    /// 1% = 10_000
    pub permillion: u64,
}

impl TokenAmount {
    pub fn new(amount: u64) -> Self {
        Self { amount }
    }

    pub fn max_value() -> Self {
        Self {
            amount: std::u64::MAX,
        }
    }
}

impl Slot {
    pub fn new(slot: u64) -> Self {
        Self { slot }
    }

    pub fn current() -> Result<Self> {
        Ok(Self {
            slot: Clock::get()?.slot,
        })
    }
}

impl Permillion {
    pub fn from_percent(percent: u64) -> Self {
        Self {
            permillion: percent.checked_mul(10_000).unwrap(),
        }
    }
}

impl From<TokenAmount> for Decimal {
    fn from(tokens: TokenAmount) -> Self {
        Decimal::from(tokens.amount)
    }
}

impl From<Permillion> for Decimal {
    fn from(permillion: Permillion) -> Self {
        Decimal::from_permillion(permillion.permillion)
    }
}

impl From<u64> for TokenAmount {
    fn from(amount: u64) -> Self {
        Self { amount }
    }
}
