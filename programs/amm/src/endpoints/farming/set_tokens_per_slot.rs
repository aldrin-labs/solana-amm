//! Allows the [`Farm`] admin to update the configurable
//! parameter defining how many tokens are distributed
//! across each slot. This parameter can be changed and
//! upgraded in future slots. The admin is allowed to
//! configure this parameter only a fixed number of times

use crate::prelude::*;

#[derive(Accounts)]
pub struct SetTokensPerSlot<'info> {
    /// The ownership over the farm is checked in the [`handle`] function.
    pub admin: Signer<'info>,
    /// # Important
    /// We must check all constraints in the [`handle`] body because farm needs
    /// to be loaded first
    #[account(mut)]
    pub farm: AccountLoader<'info, Farm>,
}

pub fn handle(
    ctx: Context<SetTokensPerSlot>,
    harvest_mint: Pubkey,
    mut valid_from_slot: Slot,
    tokens_per_slot: TokenAmount,
) -> Result<()> {
    let accounts = ctx.accounts;
    let mut farm = accounts.farm.load_mut()?;

    if farm.admin != accounts.admin.key() {
        return Err(error!(AmmError::FarmAdminMismatch));
    }

    let current_slot = Slot::current()?;

    if valid_from_slot.slot == 0 {
        valid_from_slot = current_slot;
    } else if valid_from_slot.slot < current_slot.slot {
        msg!(
            "Cannot set tokens per slot setting to past, \
            use 0 for default to current slot"
        );
        return Err(error!(AmmError::InvalidSlot));
    }

    let oldest_snapshot = farm.oldest_snapshot();
    farm.set_tokens_per_slot(
        oldest_snapshot,
        harvest_mint,
        valid_from_slot,
        tokens_per_slot,
    )?;

    Ok(())
}
