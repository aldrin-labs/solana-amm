pub mod farm;

pub use farm::*;

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
