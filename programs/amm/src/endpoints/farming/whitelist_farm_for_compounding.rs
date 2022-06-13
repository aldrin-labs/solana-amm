//! This endpoint can be called by the admin of a source farm in order to
//! whitelist the target farms that the source farm can send the harvest tokens
//! to in case of auto-compounding. The whitelisting is performed by creating
//! a program derived address with the source farm and the target farm keys.

use crate::prelude::*;

#[derive(Accounts)]
pub struct WhitelistFarmForCompouding<'info> {
    /// Is the admin of the source_farm
    #[account(mut)]
    pub admin: Signer<'info>,
    /// Represents the farm whitelisting the target farm
    pub source_farm: AccountLoader<'info, Farm>,
    /// Representes the farm to be whitelisted by source farm
    pub target_farm: AccountLoader<'info, Farm>,
    /// CHECK: UNSAFE_CODES.md#signer
    /// The whitelist is signaled by the existance of the following PDA
    #[account(
        init,
        payer = admin,
        space = 8,
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

pub fn handle(ctx: Context<WhitelistFarmForCompouding>) -> Result<()> {
    let accounts = ctx.accounts;
    let source_farm = accounts.source_farm.load()?;

    if source_farm.admin != accounts.admin.key() {
        return Err(error!(AmmError::FarmAdminMismatch));
    }

    Ok(())
}
