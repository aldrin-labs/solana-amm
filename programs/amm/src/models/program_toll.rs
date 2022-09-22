//! Program toll is the share on swap fees which are given to the program's
//! owner. In swap, we mint LP tokens and transfer them to a wallet owned by the
//! authority set on the [`ProgramToll`] account.

use crate::prelude::*;

#[account]
pub struct ProgramToll {
    pub authority: Pubkey,
}

impl ProgramToll {
    pub const PDA_SEED: &'static [u8; 4] = b"toll";

    pub fn space() -> usize {
        let discriminant = 8;
        let authority = 32;

        discriminant + authority
    }
}
