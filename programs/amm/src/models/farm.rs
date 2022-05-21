//! Admin's representation of rewards and history of the system.

use crate::prelude::*;

/// To create a user incentive for token possession, we distribute time
/// dependent rewards. A farmer stakes tokens of a mint `S`, ie. they lock them
/// with the program, and they become eligible for harvest.
#[zero_copy]
#[derive(Default)]
pub struct Farm {
    /// Can change settings on this farm.
    pub admin: Pubkey,
    /// The mint of tokens which are staked by farmers. Also referred to as
    /// `S`.
    ///
    /// Created e.g. in the core part of the AMM logic and here
    /// serves as a natural boundary between the two features: _(1)_ depositing
    /// liquidity and swapping; _(2)_ farming with which this document is
    /// concerned
    pub stake_mint: Pubkey,
    /// Staked tokens are stored in this program's vault (token account.)
    ///
    /// This is derivable from the farm's pubkey as a seed.
    pub stake_vault: Pubkey,
    /// Is also the same mint as `stake_vault`, ie. `stake_mint`, but we use
    /// this vault to store tokens which have been staked in the present
    /// snapshot window. The tokens from this vault are transferred to
    /// `stake_vault` on [`crate::endpoints::farming::take_snapshot`].
    ///
    /// This is derivable from the farm's pubkey as a seed.
    pub vesting_vault: Pubkey,
    /// List of different harvest mints with configuration of how many tokens
    /// are released per slot.
    ///
    /// # Note
    /// Len must match [`consts::MAX_HARVEST_MINTS`].
    pub harvests: [Harvest; 10],
    /// Stores snapshots of the amount of total staked tokens and changes to
    /// `ρ`. Note that [`Farm`] is in a many-to-one relationship to a
    /// [`History`].
    pub ring_buffer: Snapshots,
}

/// # Important
/// If the `harvest_mint` is equal to [`PublicKey::default`], then the harvest
/// is uninitialized. We don't use an enum to represent uninitialized mints as
/// the anchor FE client has troubles parsing enums in zero copy accounts.
///
/// TODO: link an issue for this bug
#[derive(Debug, Eq, PartialEq, Copy, Clone, Default)]
#[repr(packed)]
pub struct Harvest {
    /// The mint of tokens which are distributed to farmers. This can be the
    /// same mint as `S`.
    pub harvest_mint: Pubkey,
    /// Admin deposits the reward tokens which are harvested by farmer into
    /// this vault.
    ///
    /// This is derivable from the farm's pubkey and harvest mint's pubkey.
    pub harvest_vault: Pubkey,
    /// The harvest is distributed using a configurable _tokens per slot_
    /// (`ρ`.) This value represents how many tokens should be divided
    /// between all farmers per slot (~0.4s.)
    ///
    /// This stores an ordered list of changes to this setting. We need to keep
    /// the history because changes to this value should never apply
    /// retroactively. The history is limited by the snapshots ring buffer full
    /// rotation period. See the design docs for more info.
    ///
    /// # Important
    /// This array is ordered by the `TokensPerSlotHistory.since.slot` integer
    /// DESC.
    ///
    /// # Note
    /// This len must match [`consts::TOKENS_PER_SLOT_HISTORY_LEN`].
    pub tokens_per_slot: [TokensPerSlotHistory; 10],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(packed)]
pub struct TokensPerSlotHistory {
    pub value: TokenAmount,
    /// The new value was updated at this slot. However, it will not be valid
    /// _since_ this slot, only since the first snapshot start slot that's
    /// greater than this slot. That is, the configuration cannot be applied
    /// to currently open snapshot window.
    pub at: Slot,
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(packed)]
pub struct Snapshots {
    /// What's the last snapshot index to consider valid. When the buffer tip
    /// reaches [`consts::SNAPSHOTS_LEN`], it is set to 0 again and now the
    /// queue of snapshots starts at index 1. With next call, the tip is set to
    /// 1 and queue starts at index 2.
    ///
    /// There's a special scenario to consider which is the first population of
    /// the ring buffer. We check the slot at the last index of the buffer and
    /// if the slot is equal to zero, that means that we haven't done the first
    /// rotation around the buffer yet. And therefore if the tip is at N, in
    /// this special case the beginning is on index 0 and not N + 1.
    pub ring_buffer_tip: u32,
    /// How many tokens were in the staking vault.
    ///
    /// # Note
    /// Len must match [`consts::SNAPSHOTS_LEN`].
    pub ring_buffer: [Snapshot; 1_000],
}

/// Defines a snapshot window.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(packed)]
pub struct Snapshot {
    pub staked: TokenAmount,
    pub started_at: Slot,
}

impl Default for Snapshots {
    fn default() -> Self {
        Self {
            ring_buffer_tip: 0,
            ring_buffer: [Snapshot::default(); consts::SNAPSHOTS_LEN],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_matches_harvest_tokens_per_slot_with_const() {
        let harvest = Harvest::default();

        assert_eq!(
            harvest.tokens_per_slot.len(),
            consts::TOKENS_PER_SLOT_HISTORY_LEN
        );
    }

    #[test]
    fn it_matches_snapshots_with_const() {
        let snapshots = Snapshots::default();

        assert_eq!(snapshots.ring_buffer.len(), consts::SNAPSHOTS_LEN);
    }

    #[test]
    fn it_matches_harvests_with_const() {
        let farm = Farm::default();

        assert_eq!(farm.harvests.len(), consts::MAX_HARVEST_MINTS);
    }
}
