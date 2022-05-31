//! Sets a new farm owner. A [`Farm`] is administrated by an owner: An user pubkey
//! which has special permissions, like setting certain protocol parameters.
//!

use crate::prelude::*;

#[derive(Accounts)]
pub struct SetFarmOwner<'info> {
    /// THe ownership over the farm is checked in the [`handle`] function.
    pub admin: Signer<'info>,
    pub new_farm_admin: Signer<'info>,
    /// # Important
    /// We must check all constraints in the [`handle`] body because farm needs
    /// to be loaded first.
    #[account(mut)]
    pub farm: AccountLoader<'info, Farm>,
}

pub fn handle(ctx: Context<SetFarmOwner>) -> Result<()> {
    let accounts = ctx.accounts;
    let mut farm = accounts.farm.load_mut()?;
    // we first must check that the current farm admin coincides
    // with accounts current farm admin pub key, otherwise
    // we throw an error
    if farm.admin != accounts.admin.key() {
        return Err(error!(AmmError::FarmAdminMismatch));
    }

    farm.admin = accounts.new_farm_admin.key();
    Ok(())
}
