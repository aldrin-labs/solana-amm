//! Admin's representation of rewards and history of the system.

use crate::models::{Slot, TokenAmount};
use crate::prelude::*;
use std::cmp::Ordering;
use std::iter;

/// To create a user incentive for token possession, we distribute time
/// dependent rewards. A farmer stakes tokens of a mint `S`, ie. they lock them
/// with the program, and they become eligible for harvest.
#[derive(Default)]
#[account(zero_copy)]
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
    /// List of different harvest mints with configuration of how many tokens
    /// are released per slot.
    ///
    /// # Important
    /// Defaults to an array with all harvest mints as default pubkeys. Only
    /// when a pubkey is not the default one is the harvest initialized.
    ///
    /// # Note
    /// Len must match [`consts::MAX_HARVEST_MINTS`].
    pub harvests: [Harvest; 10],
    /// Stores snapshots of the amount of total staked tokens and changes to
    /// `ρ`. Note that [`Farm`] is in a many-to-one relationship to a
    /// [`History`].
    pub snapshots: Snapshots,
    /// Enforces a minimum amount of timespan between snapshots, thus ensures
    /// that the ring_buffer in total has a minimum amount of time ellapsed.
    /// When a Farm is initiated, min_snapshot_window_slots is defaulted to
    /// zero. When zero, the endpoint take_snapshots will set this contraint
    /// to the default value [`consts::MIN_SNAPSHOT_WINDOW_SLOTS`].
    /// This field is configurable via the endpoint set_min_snapshot_window
    /// which can be called by the admin.
    pub min_snapshot_window_slots: u64,
}

/// # Important
/// If the `harvest_mint` is equal to [`Pubkey::default`], then the harvest
/// is uninitialized. We don't use an enum to represent uninitialized mints as
/// the anchor FE client has troubles parsing enums in zero copy accounts. And
/// this way we also safe some account space.
#[derive(Debug, Eq, PartialEq, Default)]
#[zero_copy]
pub struct Harvest {
    /// The mint of tokens which are distributed to farmers. This can be the
    /// same mint as `S`.
    pub mint: Pubkey,
    /// Admin deposits the reward tokens which are harvested by farmer into
    /// this vault.
    ///
    /// This is derivable from the farm's pubkey and harvest mint's pubkey.
    pub vault: Pubkey,
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
    /// This array is ordered by the `TokensPerSlotHistory.at.slot` integer
    /// DESC.
    ///
    /// # Note
    /// This len must match [`consts::TOKENS_PER_SLOT_HISTORY_LEN`].
    pub tokens_per_slot: [TokensPerSlotHistory; 10],
}

#[derive(Debug, Default, Eq, PartialEq)]
#[zero_copy]
pub struct TokensPerSlotHistory {
    pub value: TokenAmount,
    /// The new value was updated at this slot. However, it will not be valid
    /// _since_ this slot, only since the first snapshot start slot that's
    /// greater than this slot. That is, the configuration cannot be applied
    /// to currently open snapshot window.
    pub at: Slot,
}

#[derive(Eq, PartialEq)]
#[zero_copy]
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
    ///
    /// # Note
    /// It's [`u64`] and not smaller because otherwise there are issues with
    /// packing of this struct and deserialization.
    pub ring_buffer_tip: u64,
    /// How many tokens were in the staking vault.
    ///
    /// # Note
    /// Len must match [`consts::SNAPSHOTS_LEN`].
    pub ring_buffer: [Snapshot; 1000],
}

/// Defines a snapshot window.
#[derive(Debug, Default, Eq, PartialEq)]
#[zero_copy]
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

impl Farm {
    pub const SIGNER_PDA_PREFIX: &'static [u8; 6] = b"signer";
    pub const STAKE_VAULT_PREFIX: &'static [u8; 11] = b"stake_vault";

    pub fn add_harvest(
        &mut self,
        harvest_mint: Pubkey,
        harvest_vault: Pubkey,
        tokens_per_slot: TokenAmount,
    ) -> Result<()> {
        // this should also be checked by the PDA seed, that is the harvest
        // vault key will already exist and `init` will fail
        let already_exists =
            self.harvests.iter().any(|h| h.mint == harvest_mint);
        if already_exists {
            return Err(error!(err::acc("Harvest mint already exists")));
        }

        if let Some(harvest) = self
            .harvests
            .iter_mut()
            .find(|h| h.mint == Pubkey::default())
        {
            harvest.mint = harvest_mint;
            harvest.vault = harvest_vault;
            // we could also just write to zeroth index, because the array
            // should be all zeroes, but let's overwrite the whole
            // array anyway
            harvest.tokens_per_slot = iter::once(TokensPerSlotHistory {
                value: tokens_per_slot,
                at: Slot::current()?,
            })
            .chain(iter::repeat(TokensPerSlotHistory::default()))
            .take(consts::TOKENS_PER_SLOT_HISTORY_LEN)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| {
                msg!(
                    "Cannot convert tokens per slot history vector into array"
                );
                AmmError::InvariantViolation
            })?;

            Ok(())
        } else {
            Err(error!(err::acc("Reached maximum harvest mints")))
        }
    }

    pub fn set_tokens_per_slot(
        &mut self,
        oldest_snapshot: Snapshot,
        harvest_mint: Pubkey,
        valid_from_slot: Slot,
        tokens_per_slot: TokenAmount,
    ) -> Result<()> {
        let current_slot = Slot::current()?;

        let harvest = self
            .harvests
            .iter_mut()
            .find(|h| h.mint == harvest_mint)
            .ok_or(AmmError::UnknownHarvestMintPubKey)?;

        let latest_tokens_per_slot_history = &mut harvest.tokens_per_slot[0];
        // if the latest tokens per slot is at a future slot, then we update its
        // value
        if latest_tokens_per_slot_history.at.slot >= current_slot.slot {
            *latest_tokens_per_slot_history = TokensPerSlotHistory {
                at: valid_from_slot,
                value: tokens_per_slot,
            };
            return Ok(());
        }

        // we know a priori that harvest: TokensPerSlotHistory was at least
        // already initialized as a fixed number length array of default
        // elements so unwrap `.last()` is safe
        let oldest_token_per_slot_history =
            harvest.tokens_per_slot.last().unwrap();
        // if the oldest tokens per slot is within the current snapshot history,
        // we are unable to update its value, the admin already passed the
        // allowed max number of possible configuration updates
        if oldest_token_per_slot_history.at.slot != 0
            && oldest_token_per_slot_history.at.slot
                >= oldest_snapshot.started_at.slot
        {
            return Err(error!(AmmError::ConfigurationUpdateLimitExceeded));
        }

        // At this step, we know that the oldest token slot history is strictly
        // less than the oldest snapshot slot. In this case, we update
        // the `harvests` array to have a new harvest with token slot
        // history with slot at `valid_from_slot`
        harvest.tokens_per_slot.rotate_right(1);

        // get new latest token per slot history
        let new_latest_token_per_slot_history = TokensPerSlotHistory {
            value: tokens_per_slot,
            at: valid_from_slot,
        };
        harvest.tokens_per_slot[0] = new_latest_token_per_slot_history;

        Ok(())
    }
    pub fn latest_snapshot(&self) -> Snapshot {
        self.snapshots.ring_buffer[self.snapshots.ring_buffer_tip as usize]
    }

    pub fn oldest_snapshot(&self) -> Snapshot {
        self.snapshots.ring_buffer[self.oldest_snapshot_index()]
    }

    fn oldest_snapshot_index(&self) -> usize {
        if self.snapshots.ring_buffer_tip as usize != consts::SNAPSHOTS_LEN - 1
        {
            self.snapshots.ring_buffer_tip as usize + 1
        } else {
            0
        }
    }

    /// This method returns an iterator that only contains the snapshots that
    /// have not been entirely used to calculate eligible harvest. This
    /// includes the last snapshot, which corresponds to the open window.
    /// The iterator is ready to be consumed from reverse order,
    /// from the newest snapshot to the oldest.
    ///
    /// Given three snapshots starting at 5, 10 and 15, and given a
    /// `calculate_next_harvest_from` = 12, then this method will return a
    /// slice of length 2, containing the snapshots that start at 15 and 10.
    /// However, when there is only the open window left to be harvested it will
    /// retun an empty slice.
    pub fn get_window_snapshots_eligible_to_harvest(
        &self,
        calculate_next_harvest_from: Slot,
    ) -> impl Iterator<Item = &Snapshot> {
        let tip = self.snapshots.ring_buffer_tip as usize;

        // The inverted_tip is the tip of the buffer ring in reverse order.
        // since we ar using `.rev()`, to make sure the iterator start at the
        // open window we have to use `.skip(inverted_tip)` instead of
        // `.skip(tip)`.
        let inverted_tip = consts::SNAPSHOTS_LEN - tip - 1;

        // note that this will not be "inverted" index
        let oldest_unclaimed_snapshot_index = self
            .snapshots
            .ring_buffer
            .iter()
            .enumerate()
            // going through the iterator in reverse order,
            // starting form the ring buffer tip is the most efficient way
            .rev()
            .cycle()
            .skip(inverted_tip)
            // to avoid the risk of looping forever if the skip_while
            // condition is met entirely through the ring buffer.
            .take(consts::SNAPSHOTS_LEN)
            // find first snapshot which was taken before last calculation,
            // we're inclusive with `calculate_next_harvest_from`
            .find(|(_, snapshot)| {
                snapshot.started_at.slot <= calculate_next_harvest_from.slot
            })
            .map(|(index, _)| index)
            .unwrap_or_else(|| self.oldest_snapshot_index());

        let eligible_snapshots_count =
            match oldest_unclaimed_snapshot_index.cmp(&tip) {
                Ordering::Less => {
                    // The addition of one refers to the open window
                    self.snapshots.ring_buffer_tip
                        - (oldest_unclaimed_snapshot_index as u64)
                        + 1
                }
                Ordering::Equal => 1, // Returns only the open window
                Ordering::Greater => {
                    // tip + diff(snapshot len and oldest unclaimed snapshot) +
                    // 1 (for the open window)
                    self.snapshots.ring_buffer_tip
                        + ((consts::SNAPSHOTS_LEN as u64)
                            - (oldest_unclaimed_snapshot_index as u64))
                        + 1
                }
            };

        self.snapshots
            .ring_buffer
            .iter()
            .rev()
            .cycle()
            .skip(inverted_tip)
            .take(eligible_snapshots_count as usize)
    }

    /// This method contains the core logic of the take_snapshot endpoint.
    /// The method is called in the handle function of the endpoint.
    /// It writes current stake_vault amount along with the current slot
    /// to the snapshot positioned in the next ring_buffer_tip.
    pub fn take_snapshot(
        &mut self,
        clock: Slot,
        stake_vault: TokenAmount,
    ) -> Result<()> {
        // When the farm is initialised, farm.min_snapshot_window_slots is set
        // to zero If the admin does not change this value the program
        // defaults the minimum snapshot window slots to the default
        // value
        let min_snapshot_window_slots = if self.min_snapshot_window_slots == 0 {
            consts::MIN_SNAPSHOT_WINDOW_SLOTS
        } else {
            self.min_snapshot_window_slots
        };

        let mut snapshots = &mut self.snapshots;

        // The slot in which the last snapshot was taken
        let last_snapshot_slot = snapshots.ring_buffer
            [(snapshots.ring_buffer_tip as usize)]
            .started_at
            .slot;

        // Assert that sufficient time as passed
        if clock.slot < last_snapshot_slot + min_snapshot_window_slots {
            return Err(error!(
                AmmError::InsufficientSlotTimeSinceLastSnapshot
            ));
        }

        // Set snapshot ring buffer tip to next
        // When the farm is initialised, the ring_buffer_tip is defaulted to
        // zero. This means that the first in the first iteration of the
        // ring_buffer the new snapshot elements are recorded
        // from the index 1 onwards. Only when the tip reaches the max value and
        // it resets to 0 that the snapshot elements start being
        // recorded from  index 0 onwards.
        let is_tip_last_index = snapshots.ring_buffer_tip as usize
            == snapshots.ring_buffer.len() - 1;

        snapshots.ring_buffer_tip = if is_tip_last_index {
            0
        } else {
            snapshots.ring_buffer_tip + 1
        };

        // Write data to the to the buffer
        let tip = snapshots.ring_buffer_tip as usize;

        snapshots.ring_buffer[tip] = Snapshot {
            staked: TokenAmount {
                amount: stake_vault.amount,
            },
            started_at: clock,
        };

        Ok(())
    }
}

impl Harvest {
    pub const VAULT_PREFIX: &'static [u8; 13] = b"harvest_vault";

    /// Returns the last change to ρ before or at a given slot.
    ///
    /// # Important
    /// If the admin changes ρ during an open snapshot window, it should only be
    /// considered from the next snapshot. This method _does not account_ for
    /// that invariant.
    ///
    /// # Returns
    /// First tuple member is the ρ itself, second tuple member returns the slot
    /// of the _next_ ρ change if any ([`None`] if latest.)
    pub fn tokens_per_slot(&self, at: Slot) -> (TokenAmount, Option<Slot>) {
        match self
            .tokens_per_slot
            .iter()
            .position(|tps| tps.at.slot <= at.slot)
        {
            Some(0) => (self.tokens_per_slot[0].value, None),
            Some(i) => (
                self.tokens_per_slot[i].value,
                Some(self.tokens_per_slot[i - 1].at),
            ),
            None => {
                msg!("There is no ρ history for the farm at {}", at.slot);
                (
                    // no history = harvest lost
                    TokenAmount { amount: 0 },
                    // find the oldest (hence rev) change to the setting
                    self.tokens_per_slot
                        .iter()
                        .rev()
                        .find(|tps| tps.value.amount != 0)
                        .map(|tps| tps.at)
                        .or(Some(self.tokens_per_slot[0].at)),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::utils;
    use serial_test::serial;
    use std::iter;

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

    #[test]
    fn it_has_stable_size() {
        let farm = Farm::default();

        assert_eq!(8 + std::mem::size_of_val(&farm), 18_360);
    }

    #[test]
    fn it_calculates_tps_with_empty_setting() {
        let harvest = Harvest::default();
        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 0 });
        assert_eq!(tps.amount, 0);
        assert!(next_change.is_none());
    }

    #[test]
    fn it_calculates_tps_with_one_setting() {
        let mut harvest = Harvest::default();
        harvest.tokens_per_slot[0] = TokensPerSlotHistory {
            value: TokenAmount { amount: 10 },
            at: Slot { slot: 100 },
        };

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 0 });
        assert_eq!(tps.amount, 0);
        assert_eq!(next_change, Some(Slot { slot: 100 }));

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 50 });
        assert_eq!(tps.amount, 0);
        assert_eq!(next_change, Some(Slot { slot: 100 }));

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 100 });
        assert_eq!(tps.amount, 10);
        assert!(next_change.is_none());

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 101 });
        assert_eq!(tps.amount, 10);
        assert!(next_change.is_none());

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 200 });
        assert_eq!(tps.amount, 10);
        assert!(next_change.is_none());
    }

    #[test]
    fn it_calculates_tps_with_five_settings() {
        let mut harvest = Harvest::default();
        harvest.tokens_per_slot[0] = TokensPerSlotHistory {
            value: TokenAmount { amount: 10 },
            at: Slot { slot: 100 },
        };
        harvest.tokens_per_slot[1] = TokensPerSlotHistory {
            value: TokenAmount { amount: 5 },
            at: Slot { slot: 90 },
        };
        harvest.tokens_per_slot[2] = TokensPerSlotHistory {
            value: TokenAmount { amount: 8 },
            at: Slot { slot: 80 },
        };
        harvest.tokens_per_slot[3] = TokensPerSlotHistory {
            value: TokenAmount { amount: 0 },
            at: Slot { slot: 70 },
        };
        harvest.tokens_per_slot[4] = TokensPerSlotHistory {
            value: TokenAmount { amount: 20 },
            at: Slot { slot: 60 },
        };

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 101 });
        assert_eq!(tps.amount, 10);
        assert!(next_change.is_none());

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 100 });
        assert_eq!(tps.amount, 10);
        assert!(next_change.is_none());

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 91 });
        assert_eq!(tps.amount, 5);
        assert_eq!(next_change, Some(Slot { slot: 100 }));

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 90 });
        assert_eq!(tps.amount, 5);
        assert_eq!(next_change, Some(Slot { slot: 100 }));

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 89 });
        assert_eq!(tps.amount, 8);
        assert_eq!(next_change, Some(Slot { slot: 90 }));

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 81 });
        assert_eq!(tps.amount, 8);
        assert_eq!(next_change, Some(Slot { slot: 90 }));

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 80 });
        assert_eq!(tps.amount, 8);
        assert_eq!(next_change, Some(Slot { slot: 90 }));

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 71 });
        assert_eq!(tps.amount, 0);
        assert_eq!(next_change, Some(Slot { slot: 80 }));

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 61 });
        assert_eq!(tps.amount, 20);
        assert_eq!(next_change, Some(Slot { slot: 70 }));

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 60 });
        assert_eq!(tps.amount, 20);
        assert_eq!(next_change, Some(Slot { slot: 70 }));

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 59 });
        assert_eq!(tps.amount, 0);
        assert_eq!(next_change, Some(Slot { slot: 60 }));

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 0 });
        assert_eq!(tps.amount, 0);
        assert_eq!(next_change, Some(Slot { slot: 60 }));
    }

    #[test]
    fn it_calculates_tps_with_max_settings() {
        let mut harvest = Harvest::default();
        harvest.tokens_per_slot[0] = TokensPerSlotHistory {
            value: TokenAmount { amount: 10 },
            at: Slot { slot: 100 },
        };
        for i in 1..(consts::MAX_HARVEST_MINTS - 2) {
            harvest.tokens_per_slot[i] = harvest.tokens_per_slot[0];
        }
        harvest.tokens_per_slot[8] = TokensPerSlotHistory {
            value: TokenAmount { amount: 1 },
            at: Slot { slot: 10 },
        };
        harvest.tokens_per_slot[9] = TokensPerSlotHistory {
            value: TokenAmount { amount: 5 },
            at: Slot { slot: 5 },
        };

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 9 });
        assert_eq!(tps.amount, 5);
        assert_eq!(next_change, Some(Slot { slot: 10 }));

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 5 });
        assert_eq!(tps.amount, 5);
        assert_eq!(next_change, Some(Slot { slot: 10 }));

        let (tps, next_change) = harvest.tokens_per_slot(Slot { slot: 0 });
        assert_eq!(tps.amount, 0);
        assert_eq!(next_change, Some(Slot { slot: 5 }));
    }

    #[test]
    fn it_returns_farm_latest_snapshot() {
        let farm = Farm::default();
        assert_eq!(farm.latest_snapshot(), Snapshot::default());

        let mut farm = Farm::default();
        farm.snapshots.ring_buffer_tip = 10;
        farm.snapshots.ring_buffer[10] = Snapshot {
            staked: TokenAmount::new(10),
            started_at: Slot::new(20),
        };
        assert_eq!(
            farm.latest_snapshot(),
            Snapshot {
                staked: TokenAmount::new(10),
                started_at: Slot::new(20),
            }
        );
    }

    #[test]
    fn it_returns_farm_oldest_snapshot() {
        let farm = Farm::default();
        assert_eq!(farm.oldest_snapshot(), Snapshot::default());

        let mut farm = Farm::default();
        let mut current_tip: usize;

        for oldest_snapshot_tip in 1..(consts::SNAPSHOTS_LEN as u64) {
            farm.snapshots.ring_buffer_tip = oldest_snapshot_tip - 1;

            current_tip = oldest_snapshot_tip as usize;

            // oldest snapshot tip passes the last position of the array,
            // we set it to 0
            if oldest_snapshot_tip == consts::SNAPSHOTS_LEN as u64 {
                current_tip = 0;
            }

            farm.snapshots.ring_buffer[current_tip] = Snapshot {
                staked: TokenAmount::new(10),
                started_at: Slot::new(20 + oldest_snapshot_tip),
            };

            assert_eq!(
                farm.oldest_snapshot(),
                Snapshot {
                    staked: TokenAmount::new(10),
                    started_at: Slot::new(20 + oldest_snapshot_tip),
                }
            );
        }
    }

    #[test]
    fn it_takes_snapshot() {
        let mut farm = Farm::default();
        farm.min_snapshot_window_slots = 1;

        let stake_vault_amount = 10;
        let current_slot = 5;

        assert_eq!(farm.snapshots.ring_buffer_tip, 0);

        let _result = farm.take_snapshot(
            Slot::new(current_slot),
            TokenAmount::new(stake_vault_amount),
        );

        // After take_snapshot is called the tip should
        // move from 0 to 1
        assert_eq!(farm.snapshots.ring_buffer_tip, 1);

        assert_eq!(
            farm.snapshots.ring_buffer[1].staked,
            TokenAmount { amount: 10 }
        );

        assert_eq!(farm.snapshots.ring_buffer[1].started_at, Slot { slot: 5 });
    }

    #[test]
    #[serial]
    fn set_tokens_per_slot_when_harvest_schedule_in_future() -> Result<()> {
        // asserts that every schedule configuration to tokens per slot
        // parameter is sucessful and does not change previous harvests

        let mut farm = Farm::default();

        let tokens_per_slot = TokenAmount { amount: 100 };
        let valid_from_slot = Slot { slot: 10 }; // minimum possible
        utils::set_clock(valid_from_slot);

        let harvest = farm.harvests[0];
        let harvest_mint = harvest.mint;
        let oldest_snapshot = farm.oldest_snapshot();

        // update harvest.tokens_per_slot first entry to be in the future
        farm.harvests[0].tokens_per_slot[0] = TokensPerSlotHistory {
            at: Slot { slot: 10 },
            value: TokenAmount { amount: 10 },
        };

        // call set_tokens_per_slot method
        farm.set_tokens_per_slot(
            oldest_snapshot,
            harvest_mint,
            valid_from_slot,
            tokens_per_slot,
        )?;

        assert_eq!(
            farm.harvests[0].tokens_per_slot[0],
            TokensPerSlotHistory {
                at: valid_from_slot,
                value: tokens_per_slot
            }
        );

        for i in 1..9_usize {
            assert_eq!(
                harvest.tokens_per_slot[i],
                TokensPerSlotHistory {
                    at: Slot { slot: 0 },
                    value: TokenAmount { amount: 0 }
                }
            );
        }

        let tokens_per_slot = TokenAmount { amount: 200 };

        // call set_tokens_per_slot method
        farm.set_tokens_per_slot(
            oldest_snapshot,
            harvest_mint,
            valid_from_slot,
            tokens_per_slot,
        )?;

        assert_eq!(
            farm.harvests[0].tokens_per_slot[0],
            TokensPerSlotHistory {
                at: valid_from_slot,
                value: tokens_per_slot
            }
        );

        for i in 1..9_usize {
            assert_eq!(
                harvest.tokens_per_slot[i],
                TokensPerSlotHistory {
                    at: Slot { slot: 0 },
                    value: TokenAmount { amount: 0 }
                }
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn set_tokens_per_slot_when_past_harvest() -> Result<()> {
        // asserts that past harvest configuration updates
        // are successful if limit has not exceeded

        let mut farm = Farm::default();

        let harvest = farm.harvests[0];
        let harvest_mint = harvest.mint;
        let oldest_snapshot = farm.oldest_snapshot();

        let mut correct_tokens_per_slot_history: [TokensPerSlotHistory; 10] =
            [TokensPerSlotHistory::default(); 10];

        for i in 1..10 {
            let valid_from_slot = Slot { slot: i };
            utils::set_clock(valid_from_slot);

            let tokens_per_slot = TokenAmount { amount: 100 * i };

            farm.set_tokens_per_slot(
                oldest_snapshot,
                harvest_mint,
                valid_from_slot,
                tokens_per_slot,
            )?;

            correct_tokens_per_slot_history.rotate_right(1);
            correct_tokens_per_slot_history[0] = TokensPerSlotHistory {
                at: valid_from_slot,
                value: tokens_per_slot,
            };
        }

        assert_eq!(
            correct_tokens_per_slot_history,
            farm.harvests[0].tokens_per_slot
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn set_tokens_per_slot_limit_configurations_exceeded() {
        // asserts that in the case that tokens per slot changes
        // has exceeded the limit, logic fails with error

        let mut farm = Farm::default();

        let harvest = farm.harvests[0];
        let harvest_mint = harvest.mint;
        let oldest_snapshot = farm.oldest_snapshot();

        let valid_from_slot = Slot { slot: 15 };
        let tokens_per_slot = TokenAmount { amount: 10 };

        utils::set_clock(valid_from_slot);

        farm.harvests[0].tokens_per_slot =
            [10, 9, 8, 7, 6, 5, 4, 3, 2, 1].map(|u| TokensPerSlotHistory {
                at: Slot { slot: u },
                value: TokenAmount { amount: 100 * u },
            });

        let output = farm.set_tokens_per_slot(
            oldest_snapshot,
            harvest_mint,
            valid_from_slot,
            tokens_per_slot,
        );

        assert!(output.is_err());
    }

    #[test]
    #[serial]
    fn set_tokens_per_slot_configuration_limit_exceeded_second() {
        // asserts that in the case that tokens per slot changes
        // has exceeded the limit, logic fails with error

        let mut farm = Farm::default();

        let harvest = farm.harvests[0];
        let harvest_mint = harvest.mint;
        let oldest_snapshot = farm.oldest_snapshot();

        let valid_from_slot = Slot { slot: 120 };
        let tokens_per_slot = TokenAmount { amount: 10 };

        utils::set_clock(valid_from_slot);

        farm.harvests[0].tokens_per_slot =
            [100, 90, 80, 70, 60, 50, 40, 30, 20, 10].map(|u| {
                TokensPerSlotHistory {
                    at: Slot { slot: u },
                    value: TokenAmount { amount: 100 * u },
                }
            });

        farm.snapshots.ring_buffer
            [farm.snapshots.ring_buffer_tip as usize + 1] = Snapshot {
            staked: TokenAmount { amount: 50 },
            started_at: Slot { slot: 5 },
        };

        let output = farm.set_tokens_per_slot(
            oldest_snapshot,
            harvest_mint,
            valid_from_slot,
            tokens_per_slot,
        );

        assert!(output.is_err());
    }

    #[test]
    #[serial]
    fn set_tokens_per_slot_successfull_when_oldest_snapshot_after_oldest_token_slot(
    ) {
        // asserts that changes in tokens per slot configuration is sucessful if
        // last harvest parameter was not in future and the limit has
        // not be exceeded

        let mut farm = Farm::default();

        let harvest = farm.harvests[0];
        let harvest_mint = harvest.mint;

        let valid_from_slot = Slot { slot: 120 };
        let tokens_per_slot = TokenAmount { amount: 10 };

        utils::set_clock(valid_from_slot);

        farm.harvests[0].tokens_per_slot =
            [100, 90, 80, 70, 60, 50, 40, 30, 20, 10].map(|u| {
                TokensPerSlotHistory {
                    at: Slot { slot: u },
                    value: TokenAmount { amount: 100 * u },
                }
            });

        farm.snapshots.ring_buffer
            [farm.snapshots.ring_buffer_tip as usize + 1] = Snapshot {
            staked: TokenAmount { amount: 50 },
            started_at: Slot { slot: 15 },
        };

        let oldest_snapshot = farm.oldest_snapshot();

        farm.set_tokens_per_slot(
            oldest_snapshot,
            harvest_mint,
            valid_from_slot,
            tokens_per_slot,
        )
        .unwrap();

        // we assert that the first element of the array has been updated
        assert_eq!(
            farm.harvests[0].tokens_per_slot[0],
            TokensPerSlotHistory {
                at: Slot { slot: 120 },
                value: TokenAmount { amount: 10 }
            }
        );

        // we assert that the remaining elements were shifted
        for i in 1..10_usize {
            assert_eq!(
                farm.harvests[0].tokens_per_slot[i],
                TokensPerSlotHistory {
                    at: Slot {
                        slot: 10 * (10 - i as u64 + 1)
                    },
                    value: TokenAmount {
                        amount: 1000 * (10 - i as u64 + 1)
                    },
                }
            );
        }
    }

    #[test]
    #[serial]
    fn it_gets_window_snapshots_eligible_to_harvest() -> Result<()> {
        // This test asserts that the associated function
        // `gets_window_snapshots_eligible` returns the expected
        // eligible snapshots. For this example we have snapshots ranging
        // from slots 0, 10, 20, ..., 50, whereas snapshot 50 is the current
        // open window and the curent slot is 55.
        let farm = Farm {
            snapshots: Snapshots {
                ring_buffer_tip: 5,
                ring_buffer: utils::generate_snapshots(&mut vec![
                    (0, 2_000),
                    (10, 10_000),
                    (20, 10_000),
                    (30, 10_000),
                    (40, 10_000),
                    (50, 20_000),
                ])
                .try_into()
                .unwrap(),
            },
            ..Default::default()
        };

        let calculate_next_harvest_from = Slot { slot: 35 };
        utils::set_clock(Slot { slot: 55 });

        let mut snapshot_iter = farm.get_window_snapshots_eligible_to_harvest(
            calculate_next_harvest_from,
        );

        assert_eq!(
            snapshot_iter.next(),
            Some(&Snapshot {
                staked: TokenAmount { amount: 20_000 },
                started_at: Slot { slot: 50 },
            })
        );

        assert_eq!(
            snapshot_iter.next(),
            Some(&Snapshot {
                staked: TokenAmount { amount: 10_000 },
                started_at: Slot { slot: 40 },
            })
        );

        assert_eq!(
            snapshot_iter.next(),
            Some(&Snapshot {
                staked: TokenAmount { amount: 10_000 },
                started_at: Slot { slot: 30 },
            })
        );

        assert_eq!(snapshot_iter.next(), None);

        Ok(())
    }

    #[test]
    #[serial]
    fn it_gets_only_open_if_calculate_next_harvest_from_eq_or_gt_last_snapshot_slot(
    ) -> Result<()> {
        // The intuition behind this test is that the method
        // `get_window_snapshots_eligible_to_harvest` should return
        // an iterator with only the open window if the
        // `calculate_next_harvest_from` is equal to the last snapshot slot.

        let farm = Farm {
            snapshots: Snapshots {
                ring_buffer_tip: 1,
                ring_buffer: utils::generate_snapshots(&mut vec![
                    (0, 2_000),
                    (10, 10_000),
                ])
                .try_into()
                .unwrap(),
            },
            ..Default::default()
        };

        let calculate_next_harvest_from = Slot { slot: 11 };
        utils::set_clock(Slot { slot: 12 });

        let mut snapshot_iter = farm.get_window_snapshots_eligible_to_harvest(
            calculate_next_harvest_from,
        );

        assert_eq!(
            snapshot_iter.next(),
            Some(&Snapshot {
                staked: TokenAmount { amount: 10_000 },
                started_at: Slot { slot: 10 },
            })
        );

        assert_eq!(snapshot_iter.next(), None);

        Ok(())
    }

    #[test]
    #[serial]
    fn it_correctly_gets_snapshots_from_last_ring_buffer_cycle() -> Result<()> {
        let mut snapshots_buffer: Vec<Snapshot> =
            utils::generate_snapshots(&mut vec![(50, 2_000), (60, 10_000)]);

        snapshots_buffer[999] = Snapshot {
            staked: TokenAmount { amount: 100 },
            started_at: Slot { slot: 40 },
        };

        snapshots_buffer[998] = Snapshot {
            staked: TokenAmount { amount: 100 },
            started_at: Slot { slot: 30 },
        };

        snapshots_buffer[997] = Snapshot {
            staked: TokenAmount { amount: 100 },
            started_at: Slot { slot: 20 },
        };

        let farm = Farm {
            snapshots: Snapshots {
                ring_buffer_tip: 1,
                ring_buffer: snapshots_buffer.try_into().unwrap(),
            },
            ..Default::default()
        };

        let calculate_next_harvest_from = Slot { slot: 45 };
        utils::set_clock(Slot { slot: 65 });

        let mut snapshot_iter = farm.get_window_snapshots_eligible_to_harvest(
            calculate_next_harvest_from,
        );

        // Expect first 3 results to be snapshot and last one to be None
        assert_eq!(
            snapshot_iter.next(),
            Some(&Snapshot {
                staked: TokenAmount { amount: 10_000 },
                started_at: Slot { slot: 60 },
            })
        );

        assert_eq!(
            snapshot_iter.next(),
            Some(&Snapshot {
                staked: TokenAmount { amount: 2_000 },
                started_at: Slot { slot: 50 },
            })
        );

        assert_eq!(
            snapshot_iter.next(),
            Some(&Snapshot {
                staked: TokenAmount { amount: 100 },
                started_at: Slot { slot: 40 },
            })
        );

        assert_eq!(snapshot_iter.next(), None);

        Ok(())
    }

    #[test]
    #[serial]
    fn it_correctly_gets_snapshots_eligible_for_harvest_if_history_is_lost(
    ) -> Result<()> {
        let snapshots_buffer: Vec<Snapshot> =
            utils::generate_snapshots(&mut vec![(70, 2_000), (80, 10_000)]);

        let farm = Farm {
            snapshots: Snapshots {
                ring_buffer_tip: 1,
                ring_buffer: snapshots_buffer.try_into().unwrap(),
            },
            ..Default::default()
        };

        let calculate_next_harvest_from = Slot { slot: 55 };
        utils::set_clock(Slot { slot: 82 });

        let mut snapshot_iter = farm.get_window_snapshots_eligible_to_harvest(
            calculate_next_harvest_from,
        );

        // Expect first 3 results to be snapshot and last one to be None
        assert_eq!(
            snapshot_iter.next(),
            Some(&Snapshot {
                staked: TokenAmount { amount: 10_000 },
                started_at: Slot { slot: 80 },
            })
        );

        assert_eq!(
            snapshot_iter.next(),
            Some(&Snapshot {
                staked: TokenAmount { amount: 2_000 },
                started_at: Slot { slot: 70 },
            })
        );

        assert_eq!(
            snapshot_iter.next(),
            Some(&Snapshot {
                staked: TokenAmount { amount: 0 },
                started_at: Slot { slot: 0 },
            })
        );

        assert_eq!(snapshot_iter.next(), None);

        Ok(())
    }

    #[test]
    #[serial]
    fn it_correctly_gets_snapshots_eligible_for_harvest_if_history_is_lost_and_ring_buffer_completed_full_cycle(
    ) -> Result<()> {
        let mut snapshots_buffer: Vec<Snapshot> =
            utils::generate_snapshots(&mut vec![]);

        for i in 1000..2000 {
            snapshots_buffer[i - 1000] = Snapshot {
                staked: TokenAmount { amount: 100 },
                started_at: Slot { slot: i as u64 },
            };
        }

        let farm = Farm {
            snapshots: Snapshots {
                ring_buffer_tip: 999,
                ring_buffer: snapshots_buffer.try_into().unwrap(),
            },
            ..Default::default()
        };

        let calculate_next_harvest_from = Slot { slot: 900 };

        let mut snapshot_iter = farm.get_window_snapshots_eligible_to_harvest(
            calculate_next_harvest_from,
        );

        for i in (1000..2000).rev() {
            assert_eq!(
                snapshot_iter.next(),
                Some(&Snapshot {
                    staked: TokenAmount { amount: 100 },
                    started_at: Slot { slot: i as u64 },
                })
            );
        }

        assert_eq!(snapshot_iter.next(), None);

        Ok(())
    }

    #[test]
    #[serial]
    fn it_gets_snapshots_if_starting_from_slot_zero_and_tip_at_the_end(
    ) -> Result<()> {
        let snapshots_buffer: Vec<Snapshot> = utils::generate_snapshots(
            &mut iter::repeat(1)
                .enumerate()
                .take(consts::SNAPSHOTS_LEN)
                .map(|(i, a)| (i as u64 * 10 + 1, a))
                .collect::<Vec<_>>(),
        );

        let farm = Farm {
            snapshots: Snapshots {
                ring_buffer_tip: consts::SNAPSHOTS_LEN as u64 - 1,
                ring_buffer: snapshots_buffer.try_into().unwrap(),
            },
            ..Default::default()
        };

        let calculate_next_harvest_from = Slot { slot: 0 };
        utils::set_clock(Slot { slot: 100 });

        let snapshot_iter = farm.get_window_snapshots_eligible_to_harvest(
            calculate_next_harvest_from,
        );

        assert_eq!(snapshot_iter.count(), consts::SNAPSHOTS_LEN);

        Ok(())
    }

    #[test]
    #[serial]
    fn it_gets_snapshots_if_starting_from_slot_zero_and_buffer_not_rotated(
    ) -> Result<()> {
        let snapshots_buffer: Vec<Snapshot> = utils::generate_snapshots(
            &mut iter::repeat(1)
                .enumerate()
                .take(consts::SNAPSHOTS_LEN - 500)
                .map(|(i, a)| (i as u64 * 10 + 1, a))
                .collect::<Vec<_>>(),
        );

        let farm = Farm {
            snapshots: Snapshots {
                ring_buffer_tip: consts::SNAPSHOTS_LEN as u64 - 500 - 1,
                ring_buffer: snapshots_buffer.try_into().unwrap(),
            },
            ..Default::default()
        };

        let calculate_next_harvest_from = Slot { slot: 0 };
        utils::set_clock(Slot { slot: 100 });

        let snapshot_iter = farm.get_window_snapshots_eligible_to_harvest(
            calculate_next_harvest_from,
        );

        assert_eq!(snapshot_iter.count(), 501);

        Ok(())
    }

    #[test]
    #[serial]
    fn it_gets_snapshots_if_starting_from_slot_zero_and_buffer_rotated(
    ) -> Result<()> {
        let snapshots_buffer: Vec<Snapshot> =
            utils::generate_snapshots(&mut vec![(1, 1); consts::SNAPSHOTS_LEN]);

        let mut farm = Farm {
            snapshots: Snapshots {
                ring_buffer_tip: consts::SNAPSHOTS_LEN as u64 - 1,
                ring_buffer: snapshots_buffer.try_into().unwrap(),
            },
            min_snapshot_window_slots: 1,
            ..Default::default()
        };

        farm.take_snapshot(Slot::new(4), TokenAmount::new(1))?;
        farm.take_snapshot(Slot::new(7), TokenAmount::new(1))?;
        farm.take_snapshot(Slot::new(10), TokenAmount::new(1))?;

        let calculate_next_harvest_from = Slot { slot: 0 };
        utils::set_clock(Slot { slot: 100 });

        let snapshot_iter = farm.get_window_snapshots_eligible_to_harvest(
            calculate_next_harvest_from,
        );

        assert_eq!(snapshot_iter.count(), consts::SNAPSHOTS_LEN);

        Ok(())
    }

    #[test]
    #[serial]
    fn it_gets_snapshots_after_buffer_rotated() -> Result<()> {
        let snapshots_buffer: Vec<Snapshot> =
            utils::generate_snapshots(&mut vec![(1, 1); consts::SNAPSHOTS_LEN]);

        let mut farm = Farm {
            snapshots: Snapshots {
                ring_buffer_tip: consts::SNAPSHOTS_LEN as u64 - 1,
                ring_buffer: snapshots_buffer.try_into().unwrap(),
            },
            min_snapshot_window_slots: 1,
            ..Default::default()
        };

        farm.take_snapshot(Slot::new(4), TokenAmount::new(1))?;
        farm.take_snapshot(Slot::new(7), TokenAmount::new(1))?;
        farm.take_snapshot(Slot::new(10), TokenAmount::new(1))?;

        let calculate_next_harvest_from = Slot { slot: 4 };
        utils::set_clock(Slot { slot: 100 });

        let mut snapshot_iter = farm.get_window_snapshots_eligible_to_harvest(
            calculate_next_harvest_from,
        );

        assert_eq!(
            snapshot_iter.next(),
            Some(&Snapshot {
                staked: TokenAmount { amount: 1 },
                started_at: Slot { slot: 10 },
            })
        );
        assert_eq!(
            snapshot_iter.next(),
            Some(&Snapshot {
                staked: TokenAmount { amount: 1 },
                started_at: Slot { slot: 7 },
            })
        );
        assert_eq!(
            snapshot_iter.next(),
            Some(&Snapshot {
                staked: TokenAmount { amount: 1 },
                started_at: Slot { slot: 4 },
            })
        );
        assert_eq!(snapshot_iter.next(), None);

        Ok(())
    }
}
