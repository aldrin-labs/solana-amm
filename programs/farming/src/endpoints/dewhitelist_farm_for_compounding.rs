//! This endpoint can be called by the admin of a source farm in order to
//! remove a target farm from the source farm's auto-compounding whitelist.
//! Note that this endpoint should only be called on target_farms that have
//! already been whitelisted via endpoint [`whitelist_farm_for_compounding`].
//! The endpoint will proceed with the closing of the pda account that signals
//! the whitelisting of the target farm by the source farm.

use crate::prelude::*;

#[derive(Accounts)]
pub struct DewhitelistFarmForCompounding<'info> {
    /// Is the admin of the source_farm
    #[account(mut)]
    pub admin: Signer<'info>,
    /// Represents the farm that has target farm whitelisted
    pub source_farm: AccountLoader<'info, Farm>,
    /// Representes the farm to be removed from source farm's whitelist
    pub target_farm: AccountLoader<'info, Farm>,
    /// CHECK: UNSAFE_CODES.md#signer
    /// The whitelist is signaled by the existance of the following PDA
    /// We therefore close this account to signal the removal from the
    /// whitelist
    #[account(
        mut,
        close = admin,
        seeds = [
            Farm::WHITELIST_PDA_PREFIX,
            source_farm.key().as_ref(),
            target_farm.key().as_ref()
        ],
        bump,
    )]
    pub whitelist_compounding: Account<'info, WhitelistCompounding>,
    pub system_program: Program<'info, System>,
}

pub fn handle(ctx: Context<DewhitelistFarmForCompounding>) -> Result<()> {
    let accounts = ctx.accounts;
    let source_farm = accounts.source_farm.load()?;

    if source_farm.admin != accounts.admin.key() {
        return Err(error!(FarmingError::FarmAdminMismatch));
    }
    Ok(())
}
