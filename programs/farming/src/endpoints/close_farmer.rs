//! If the [`Farmer`] has no more staked/vested tokens and all harvests have
//! been claimed, then the account is empty and can be closed without losing
//! funds.

use crate::prelude::*;

#[derive(Accounts)]
pub struct CloseFarmer<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        constraint = farmer.authority == authority.key()
            @ err::acc("Authority does not own this farmer"),
        close = authority,
    )]
    pub farmer: Account<'info, Farmer>,
}

pub fn handle(ctx: Context<CloseFarmer>) -> Result<()> {
    let farmer = &ctx.accounts.farmer;

    if farmer.total_deposited()?.amount != 0 {
        return Err(error!(err::acc("Unstake all farmer's tokens")));
    }

    if farmer.harvests.iter().any(|h| h.tokens.amount != 0) {
        return Err(error!(err::acc("Claim all farmer's harvest")));
    }

    Ok(())
}
