//! Periodically, a bot invokes this endpoint. It writes the latest state of
//! stake vault to the ring buffer on [`Farm`]. This endpoint is
//! permission-less, but it asserts that some minimum amount of time has passed.

use crate::prelude::*;
use anchor_spl::token::TokenAccount;

#[derive(Accounts)]
pub struct TakeSnapshot<'info> {
    #[account(mut)]
    pub farm: AccountLoader<'info, Farm>,
    /// The link to the farm is checked in the [`handle`] function.
    pub stake_vault: Account<'info, TokenAccount>,
}

pub fn handle(ctx: Context<TakeSnapshot>) -> Result<()> {
    let accounts = ctx.accounts;
    let mut farm = accounts.farm.load_mut()?;

    // Checks that the stake_vault account input corresponds to the
    // farm.stake_vault
    if accounts.stake_vault.key() != farm.stake_vault {
        return Err(error!(err::acc(
            "The provided stake vault does \
            not correspond to the Farm stake vault"
        )));
    }

    farm.take_snapshot(
        Slot::current()?,
        TokenAmount::new(accounts.stake_vault.amount),
    )?;

    Ok(())
}
