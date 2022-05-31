//! # Additional accounts
//! Pairs of token accounts for each farm harvest where first member of each
//! pair is the harvest vault and second member is the farmer's harvest wallet.
//!
//! ```
//! [
//!   harvest_vault1,
//!   harvest_wallet1,
//!   harvest_vault2,
//!   harvest_wallet2,
//!   ...
//! ]
//! ```

use crate::prelude::*;
use anchor_spl::token::Token;

#[derive(Accounts)]
pub struct ClaimEligibleHarvest<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub farmer: Account<'info, Farmer>,
    /// CHECK: UNSAFE_CODES.md#signer
    pub farm_signer_pda: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

pub fn handle(
    _ctx: Context<ClaimEligibleHarvest>,
    _farm_signer_bump_seed: u8,
) -> Result<()> {
    Ok(())
}
