//! TODO: docs

use crate::prelude::*;

#[account]
pub struct Pool {
    pub initializer: Pubkey,
    pub signer: Pubkey,
    pub lp_token_program_fee_wallet: Pubkey,
    pub lp_token_mint: Pubkey,
    pub dimension: u64,
    pub reserves: [Reserve; 4],
    pub curve: Curve,
    pub fee: Fraction,
}

#[derive(
    AnchorDeserialize, AnchorSerialize, Clone, Copy, Debug, Eq, PartialEq,
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
