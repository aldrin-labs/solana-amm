//! TODO: docs

use crate::prelude::*;
use std::mem;

#[account]
pub struct Pool {
    pub admin: Pubkey,
    pub signer: Pubkey,
    pub mint: Pubkey,
    pub program_toll_wallet: Pubkey,
    pub dimension: u64,
    pub reserves: [Reserve; 4],
    pub curve: Curve,
    pub fee: Permillion,
}

#[derive(
    AnchorDeserialize, AnchorSerialize, Copy, Clone, Debug, Eq, PartialEq,
)]
pub enum Curve {
    ConstProd,
    Stable { amplifier: u64, invariant: SDecimal },
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
pub struct Reserve {
    pub tokens: TokenAmount,
    pub mint: Pubkey,
    pub vault: Pubkey,
}

impl Pool {
    pub const SIGNER_PDA_PREFIX: &'static [u8; 6] = b"signer";

    pub fn space() -> usize {
        let discriminant = 8;
        let initializer = 32;
        let signer = 32;
        let lp_token_program_fee_wallet = 32;
        let mint = 32;
        let dimension = 8;
        let reserves = mem::size_of::<Reserve>() * 4;
        let curve = mem::size_of::<Curve>();
        let fee = mem::size_of::<Permillion>();

        discriminant
            + initializer
            + signer
            + lp_token_program_fee_wallet
            + mint
            + dimension
            + reserves
            + curve
            + fee
    }
}
