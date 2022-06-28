//! TODO: docs

use crate::prelude::*;

#[account]
pub struct ProgramToll {
    pub authority: Pubkey,
}

impl ProgramToll {
    pub const ACCOUNT_SEED: &'static [u8; 4] = b"toll";

    pub fn space() -> usize {
        let discriminant = 8;
        let authority = 32;

        discriminant + authority
    }
}
