pub mod farm;
pub mod farmer;

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
)]
pub struct MintHash {
    pub hash: u64,
}
