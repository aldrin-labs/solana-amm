use crate::prelude::*;

#[derive(Accounts)]
pub struct SetMinSnapshotWindow<'info> {
    /// The ownership over the farm is checked in the [`handle`] function.
    pub admin: Signer<'info>,
    #[account(mut)]
    pub farm: AccountLoader<'info, Farm>,
}

pub fn handle(
    ctx: Context<SetMinSnapshotWindow>,
    min_snapshot_window_slots: u64,
) -> Result<()> {
    let accounts = ctx.accounts;

    let mut farm = accounts.farm.load_mut()?;

    if farm.admin != accounts.admin.key() {
        return Err(error!(AmmError::FarmAdminMismatch));
    }

    farm.min_snapshot_window_slots = min_snapshot_window_slots;

    Ok(())
}
