//! The ring buffer which stores snapshots has a limited history. Therefore, in
//! order not to lose harvest for users, this endpoint must be called before
//! the time of [`Farmer`]'s last harvest becomes greater than the ring buffer's
//! history.
//!
//! Users update their eligible harvest by interacting with the program, eg. by
//! calling [`crate::endpoints::farming::start_farming`] or
//! [`crate::endpoints::farming::stop_farming`]. However, if a user becomes
//! inactive, bots must invoke this endpoint on their behalf.

use crate::prelude::*;

#[derive(Accounts)]
pub struct UpdateEligibleHarvest<'info> {
    pub farm: AccountLoader<'info, Farm>,
    #[account(
        mut,
        constraint = farmer.farm == farm.key()
            @ err::acc("Farmer is set up for a different farm"),
    )]
    pub farmer: Account<'info, Farmer>,
}

pub fn handle(ctx: Context<UpdateEligibleHarvest>) -> Result<()> {
    let accounts = ctx.accounts;

    let farm = accounts.farm.load()?;

    accounts
        .farmer
        .check_vested_period_and_update_harvest(&farm)?;

    Ok(())
}
