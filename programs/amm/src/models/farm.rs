use crate::prelude::*;

// TODO: docs
#[account]
pub struct Farm {
    pub admin: Pubkey,
    pub lp_mint: Pubkey,
    /// This can be derived from LP mint
    pub snapshots: Pubkey,
    /// This can be derived from LP mint
    pub lp_vault: Pubkey,
    /// This can be derived from farm pubkey
    pub harvest_vault: Pubkey,
    pub tokens_per_slot: TokenAmount,
    /// This resets with each snapshot
    pub tokens_harvested_for_current_snapshot_window: TokenAmount,
    /// This resets with each snapshot
    pub harvested_lp_token_share_for_current_snapshot_window: TokenAmount,
}
