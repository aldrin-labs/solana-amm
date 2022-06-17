pub mod farm;
pub mod farmer;
#[cfg(test)]
mod tests;

pub use farm::*;
pub use farmer::*;

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

impl TokenAmount {
    pub fn new(amount: u64) -> Self {
        Self { amount }
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
