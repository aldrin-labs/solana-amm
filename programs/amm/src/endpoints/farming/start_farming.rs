use crate::prelude::*;
use anchor_spl::token::Token;

#[derive(Accounts)]
pub struct StartFarming<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub farmer: Account<'info, Farmer>,
    /// CHECK: UNSAFE_CODES.md#token
    #[account(mut)]
    pub stake_wallet: AccountInfo<'info>,
    #[account(mut)]
    pub farm: AccountLoader<'info, Farm>,
    /// CHECK: UNSAFE_CODES.md#token
    #[account(mut)]
    pub stake_vault: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

pub fn handle(_ctx: Context<StartFarming>, _stake: TokenAmount) -> Result<()> {
    Ok(())
}
