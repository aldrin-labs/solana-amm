//! Admin's representation of rewards and history of the system.

use crate::models::{Slot, TokenAmount};
use crate::prelude::*;
use std::cmp::Ordering;
use std::iter;
use std::ops::RangeInclusive;

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
    /// List of different harvest mints with harvest periods, each with its own
    /// configuration of how many tokens are released per slot.
    ///
    /// # Important
    /// Defaults to an array with all harvest mints as default pubkeys. Only
    /// when a pubkey is not the default one is the harvest initialized.
    ///
    /// # Note
    /// Len must match [`consts::MAX_HARVEST_MINTS`].
    pub harvests: [Harvest; 10],
    /// Stores snapshots of the amount of total staked tokens in a ring buffer,
    /// meaning that the history is overwritten after some time.
    pub snapshots: Snapshots,
    /// Enforces a minimum amount of timespan between snapshots, thus ensures
    /// that the ring_buffer in total has a minimum amount of time elapsed.
    /// When a Farm is initiated, min_snapshot_window_slots is defaulted to
    /// zero. When zero, the endpoint take_snapshots will set this constraint
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
    /// Each `ρ` is defined in some time range, ie. we must know at what slot
    /// that particular `ρ` starts being applicable, and at which point it
    /// stops being applicable.
    ///
    /// This array holds history of all periods, so that we can calculate
    /// appropriate harvest for farmers using the snapshot ring buffer history.
    /// Only once the snapshot ring buffer history is beyond some period's end
    /// date can that period be removed from this array.
    ///
    /// # Important
    /// 1. Periods are non-overlapping.
    /// 2. Periods are sorted by start slot DESC.
    /// 3. Gaps between periods are allowed and they should be interpreted as
    /// having `ρ = 0`.
    ///
    /// # Note
    /// This len must match [`consts::HARVEST_PERIODS_LEN`].
    pub periods: [HarvestPeriod; 10],
}

#[derive(Debug, Default, Eq, PartialEq)]
#[zero_copy]
pub struct HarvestPeriod {
    pub tps: TokenAmount,
    pub starts_at: Slot,
    pub ends_at: Slot,
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

/// Struct representing a pda account for whitelisting farms for compounding.
/// The whitelisting of a farm done by calling the endpoint
/// [`crate::endpoints::whitelist_farm_for_compounding`] which will instantiate
/// a new pda account represented by this struct.
///
/// We wrap this struct with an [`Account`] struct instead of using a simple
/// [`AccountInfo`] in order to be able to close the account if needed, by using
/// anchor constraints when calling
/// [`crate::endpoints::dewhitelist_farm_for_compounding`].
#[account]
pub struct WhitelistCompounding {}

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
    pub const WHITELIST_PDA_PREFIX: &'static [u8; 21] =
        b"whitelist_compounding";

    pub fn add_harvest(
        &mut self,
        harvest_mint: Pubkey,
        harvest_vault: Pubkey,
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
            harvest.periods = iter::repeat(HarvestPeriod::default())
                .take(consts::HARVEST_PERIODS_LEN)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| {
                    msg!(
                    "Cannot convert harvest period history vector into array"
                );
                    FarmingError::InvariantViolation
                })?;

            Ok(())
        } else {
            Err(error!(err::acc("Reached maximum harvest mints")))
        }
    }

    /// The admin always defines how long a farming should last. Once that
    /// farming finishes, they can reuse the same [`Farm`] to start a new
    /// farming period.
    ///
    /// A scheduled launch is when the latest harvest period hasn't started yet.
    /// In that case, instead of creating a new one, we overwrite the existing
    /// one. The return value indicates whether a scheduled launch overwrite
    /// happened by returning the previous scheduled period.
    pub fn new_harvest_period(
        &mut self,
        current_slot: Slot,
        harvest_mint: Pubkey,
        period: (Slot, Slot),
        tps: TokenAmount,
    ) -> Result<Option<HarvestPeriod>> {
        let oldest_snapshot = self.oldest_snapshot();

        let harvest = self
            .harvests
            .iter_mut()
            .find(|h| h.mint == harvest_mint)
            .ok_or(FarmingError::UnknownHarvestMintPubKey)?;

        let (starts_at, ends_at) = period;

        if starts_at >= ends_at {
            msg!("New harvest period must start before it ends");
            return Err(error!(
                FarmingError::HarvestPeriodCannotHaveNegativeLength
            ));
        }

        // the admin can schedule a new launch as long as it starts after the
        // following period ends
        //
        // latest started != currently active, it might have finished already
        let latest_started_period = harvest
            .periods
            .iter()
            .find(|p| p.starts_at <= current_slot)
            .ok_or_else(|| {
                // currently we don't allow all periods have `started_at` in
                // future
                msg!("All harvest periods cannot be scheduled");
                FarmingError::InvariantViolation
            })?;
        if latest_started_period.ends_at >= starts_at {
            msg!(
                "Latest started harvest period ends at slot {}, \
                new period must start later than that",
                latest_started_period.ends_at.slot
            );
            return Err(error!(FarmingError::CannotOverwriteOpenHarvestPeriod));
        }

        // if the latest period is at a future slot, then we update its
        // value
        //
        // this enables editing of scheduled launches
        let latest_period = &mut harvest.periods[0];
        if latest_period.starts_at > current_slot {
            let previous_latest_period = *latest_period;
            *latest_period = HarvestPeriod {
                tps,
                starts_at,
                ends_at,
            };
            return Ok(Some(previous_latest_period));
        }

        // we know a priori that harvest: HarvestPeriod was at least
        // already initialized as a fixed number length array of default
        // elements so unwrap `.last()` is safe
        let oldest_period = harvest.periods.last().unwrap();
        // if the oldest period is within the current snapshot history,
        // we are unable to update its value, the admin already passed the
        // allowed max number of possible configuration updates
        let is_oldest_period_initialized = oldest_period.ends_at.slot != 0;
        if is_oldest_period_initialized
            && oldest_period.ends_at.slot >= oldest_snapshot.started_at.slot
        {
            msg!("Oldest period is still within the ring buffer history");
            return Err(error!(FarmingError::ConfigurationUpdateLimitExceeded));
        }

        // At this step, we know that the oldest period end slot is strictly
        // less than the oldest snapshot slot. In this case, we update
        // the `harvests` array element of the given mint to have
        // a new harvest period starting and ending at the slots
        // given by the `period` parameter.
        harvest.periods.rotate_right(1);
        harvest.periods[0] = HarvestPeriod {
            tps,
            starts_at,
            ends_at,
        };

        Ok(None)
    }

    pub fn latest_snapshot(&self) -> Snapshot {
        self.snapshots.ring_buffer[self.snapshots.ring_buffer_tip as usize]
    }

    pub fn oldest_snapshot(&self) -> Snapshot {
        self.snapshots.ring_buffer[self.oldest_snapshot_index()]
    }

    pub fn first_snapshot_after(&self, slot: Slot) -> Option<Snapshot> {
        if self.latest_snapshot().started_at <= slot {
            // optimization for calculation in open window
            return None;
        }

        let tip = self.snapshots.ring_buffer_tip as usize;
        // The inverted_tip is the tip of the buffer ring in reverse order.
        // since we ar using `.rev()`, to make sure the iterator start at the
        // open window we have to use `.skip(inverted_tip)` instead of
        // `.skip(tip)`.
        let inverted_tip = consts::SNAPSHOTS_LEN - tip - 1;

        let pos = self
            .snapshots
            .ring_buffer
            .iter()
            .enumerate()
            .rev()
            .cycle()
            .skip(inverted_tip)
            .take(consts::SNAPSHOTS_LEN)
            .find(|(_, s)| s.started_at <= slot)
            .map(|(index, _)| index)?;

        Some(if pos == consts::SNAPSHOTS_LEN - 1 {
            self.snapshots.ring_buffer[0]
        } else {
            self.snapshots.ring_buffer[pos + 1]
        })
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
                FarmingError::InsufficientSlotTimeSinceLastSnapshot
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

    fn oldest_snapshot_index(&self) -> usize {
        if self.snapshots.ring_buffer_tip as usize != consts::SNAPSHOTS_LEN - 1
        {
            self.snapshots.ring_buffer_tip as usize + 1
        } else {
            0
        }
    }
}

impl Harvest {
    pub const VAULT_PREFIX: &'static [u8; 13] = b"harvest_vault";

    /// Returns a vec of all periods and their corresponding `ρ` ordered by
    /// the period's start slot _ASC_. That is, you can pop from this vec to get
    /// the most recent period.
    ///
    /// The range is slot when the period starts, slot when the period ends,
    /// inclusive. There are no gaps, that is two subsequent periods will fill
    /// all timeline.
    ///
    /// # Example
    /// Say there are two farming in the farm `periods` array:
    /// 1. from slot 1 to slot 10 with `ρ = 1000`
    /// 2. from slot 25 to slot 100 with `ρ = 5000`
    ///
    /// This method fills in the gaps between them with periods of `ρ = 0` and
    /// if called on slot 500, it also appends one more period from end of 2nd
    /// to the current slot, again with `ρ = 0`.
    ///
    /// ```text
    /// farm.tps_history(Slot::new(500))
    ///
    /// =>
    ///
    /// [
    ///   ((1..10), 1000),
    ///   ((11..25), 0),
    ///   ((25..100), 5000),
    ///   ((101..500), 0),
    /// ]
    /// ```
    pub fn tps_history(
        &self,
        current: Slot,
    ) -> Vec<(RangeInclusive<Slot>, TokenAmount)> {
        // because we use `windows` method, the last period will not be included
        // in the iterator, so we pad the iterator with it
        let padding = [
            self.periods[self.periods.len() - 1],
            HarvestPeriod {
                tps: TokenAmount::new(0),
                starts_at: Slot::new(0),
                ends_at: Slot::new(0),
            },
        ];
        let history = self
            .periods
            .windows(2)
            .chain(iter::once(padding.as_slice()))
            .filter_map(|periods| {
                // period we care for
                let curr = periods[0];
                // previous period in time so that we know how to fill the gaps
                // between them
                let prev = periods[1];

                // uninitialized periods are filtered out
                if curr.starts_at.slot == 0 && prev.starts_at.slot == 0 {
                    return None;
                }

                // the actual period
                let mut ranges =
                    vec![(curr.starts_at..=curr.ends_at, curr.tps)];

                // In some cases we want to pad the gap between this period and
                // the previous one with a range of `tps = 0`.
                //
                // 1. if the period starts at 0, then there's no previous
                // history, so don't pad
                //
                // 2. if the previous period ends a single slot before the
                // current starts, then no need to pad because there's no gap
                if curr.starts_at.slot != 0
                    && prev.ends_at.slot + 1 != curr.starts_at.slot
                {
                    ranges.push((
                        Slot::new(prev.ends_at.slot + 1)
                            ..=Slot::new(curr.starts_at.slot - 1),
                        TokenAmount::new(0),
                    ));
                }

                Some(ranges.into_iter())
            })
            .flatten();

        if self.periods[0].ends_at < current {
            // and pad the end as well if the latest period ended before the
            // current slot, ie. in past
            iter::once((
                Slot::new(self.periods[0].ends_at.slot + 1)..=current,
                TokenAmount::new(0),
            ))
            .chain(history)
            .rev()
            .collect()
        } else {
            history.rev().collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::utils;
    use std::iter;

    impl Farm {
        fn get_harvest(&self, mint: Pubkey) -> Harvest {
            self.harvests
                .iter()
                .find(|h| h.mint.eq(&mint))
                .copied()
                .expect("farm has no harvest of such mint")
        }
    }

    #[test]
    fn it_matches_harvest_tokens_per_slot_with_const() {
        let harvest = Harvest::default();

        assert_eq!(harvest.periods.len(), consts::HARVEST_PERIODS_LEN);
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

        assert_eq!(8 + std::mem::size_of_val(&farm), 19_160);
    }

    #[test]
    fn it_returns_first_snapshot_after_some_slot() -> Result<()> {
        let mut farm = Farm::default();
        farm.min_snapshot_window_slots = 1;
        farm.take_snapshot(Slot::new(2), TokenAmount::new(1))?;
        farm.take_snapshot(Slot::new(4), TokenAmount::new(2))?;
        farm.take_snapshot(Slot::new(6), TokenAmount::new(3))?;

        assert_eq!(farm.first_snapshot_after(Slot::new(10)), None);

        assert_eq!(
            farm.first_snapshot_after(Slot::new(0)),
            Some(Snapshot {
                staked: TokenAmount::new(1),
                started_at: Slot::new(2),
            })
        );
        assert_eq!(
            farm.first_snapshot_after(Slot::new(1)),
            Some(Snapshot {
                staked: TokenAmount::new(1),
                started_at: Slot::new(2),
            })
        );

        assert_eq!(
            farm.first_snapshot_after(Slot::new(2)),
            Some(Snapshot {
                staked: TokenAmount::new(2),
                started_at: Slot::new(4),
            })
        );

        assert_eq!(
            farm.first_snapshot_after(Slot::new(5)),
            Some(Snapshot {
                staked: TokenAmount::new(3),
                started_at: Slot::new(6)
            })
        );

        Ok(())
    }

    #[test]
    fn it_wraps_when_returning_first_snapshot_after_some_slot() -> Result<()> {
        let mut farm = Farm::default();
        farm.min_snapshot_window_slots = 1;
        for i in 5..consts::SNAPSHOTS_LEN * 2 {
            farm.take_snapshot(Slot::new(i as u64 * 2), TokenAmount::new(1))?;
        }

        assert_eq!(
            farm.first_snapshot_after(Slot::new(
                farm.snapshots.ring_buffer[consts::SNAPSHOTS_LEN - 1]
                    .started_at
                    .slot
                    + 1
            )),
            Some(farm.snapshots.ring_buffer[0])
        );

        Ok(())
    }

    #[test]
    fn it_calculates_tps_with_empty_setting() {
        let harvest = Harvest::default();
        let history = harvest.tps_history(Slot::new(30));
        assert_eq!(
            history,
            vec![(Slot::new(1)..=Slot::new(30), TokenAmount::new(0)),]
        );
    }

    #[test]
    fn it_calculates_tps() {
        let mut harvest = Harvest::default();
        harvest.periods[0] = HarvestPeriod {
            tps: TokenAmount::new(1),
            starts_at: Slot::new(20),
            ends_at: Slot::new(25),
        };
        harvest.periods[1] = HarvestPeriod {
            tps: TokenAmount::new(2),
            starts_at: Slot::new(10),
            ends_at: Slot::new(19),
        };
        harvest.periods[2] = HarvestPeriod {
            tps: TokenAmount::new(3),
            starts_at: Slot::new(5),
            ends_at: Slot::new(8),
        };

        assert_eq!(
            harvest.tps_history(Slot::new(20)),
            vec![
                (Slot::new(1)..=Slot::new(4), TokenAmount::new(0)),
                (Slot::new(5)..=Slot::new(8), TokenAmount::new(3)),
                (Slot::new(9)..=Slot::new(9), TokenAmount::new(0)),
                (Slot::new(10)..=Slot::new(19), TokenAmount::new(2)),
                (Slot::new(20)..=Slot::new(25), TokenAmount::new(1)),
            ]
        );

        // pads it with a dummy period if latest already ended
        assert_eq!(
            harvest.tps_history(Slot::new(30)),
            vec![
                (Slot::new(1)..=Slot::new(4), TokenAmount::new(0)),
                (Slot::new(5)..=Slot::new(8), TokenAmount::new(3)),
                (Slot::new(9)..=Slot::new(9), TokenAmount::new(0)),
                (Slot::new(10)..=Slot::new(19), TokenAmount::new(2)),
                (Slot::new(20)..=Slot::new(25), TokenAmount::new(1)),
                (Slot::new(26)..=Slot::new(30), TokenAmount::new(0)),
            ]
        );
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
    fn it_cannot_add_harvest_which_already_exists() -> Result<()> {
        let mint = Pubkey::new_unique();

        let mut farm = Farm::default();
        farm.add_harvest(mint, Pubkey::new_unique())?;
        assert!(farm.add_harvest(mint, Pubkey::new_unique()).is_err());

        Ok(())
    }

    #[test]
    fn it_returns_err_if_max_harvests_reached() -> Result<()> {
        let mint = Pubkey::new_unique();

        let mut farm = Farm::default();

        for _ in 0..consts::MAX_HARVEST_MINTS {
            farm.add_harvest(Pubkey::new_unique(), Pubkey::new_unique())?;
        }

        assert!(farm.add_harvest(mint, Pubkey::new_unique()).is_err());

        Ok(())
    }

    #[test]
    fn it_adds_harvest() -> Result<()> {
        let mint = Pubkey::new_unique();
        let vault = Pubkey::new_unique();
        let mut farm = Farm::default();

        farm.add_harvest(mint, vault)?;

        assert_eq!(farm.harvests[0].mint, mint);
        assert_eq!(farm.harvests[0].vault, vault);
        assert_eq!(
            farm.harvests[0].periods,
            [HarvestPeriod::default(); consts::HARVEST_PERIODS_LEN]
        );
        Ok(())
    }

    #[test]
    fn it_errs_if_harvest_period_starts_before_it_ends() -> Result<()> {
        let harvest_mint = Pubkey::new_unique();
        let mut farm = Farm::default();

        farm.add_harvest(harvest_mint, Pubkey::new_unique())?;
        assert!(farm
            .new_harvest_period(
                Slot::new(5),
                harvest_mint,
                (Slot::new(30), Slot::new(25)),
                TokenAmount::new(20),
            )
            .is_err());

        Ok(())
    }

    #[test]
    fn it_updates_scheduled_launch_during_active_harvest_period() -> Result<()>
    {
        let harvest_mint = Pubkey::new_unique();
        let mut farm = Farm::default();

        farm.add_harvest(harvest_mint, Pubkey::new_unique())?;
        farm.new_harvest_period(
            Slot::new(5),
            harvest_mint,
            (Slot::new(5), Slot::new(25)),
            TokenAmount::new(20),
        )?;
        farm.new_harvest_period(
            Slot::new(10),
            harvest_mint,
            (Slot::new(30), Slot::new(50)),
            TokenAmount::new(20),
        )?;
        farm.new_harvest_period(
            Slot::new(10),
            harvest_mint,
            (Slot::new(40), Slot::new(50)),
            TokenAmount::new(20),
        )?;

        assert_eq!(
            farm.get_harvest(harvest_mint).periods[0],
            HarvestPeriod {
                starts_at: Slot::new(40),
                ends_at: Slot::new(50),
                tps: TokenAmount::new(20),
            }
        );
        assert_eq!(
            farm.get_harvest(harvest_mint).periods[1],
            HarvestPeriod {
                starts_at: Slot::new(5),
                ends_at: Slot::new(25),
                tps: TokenAmount::new(20),
            }
        );

        Ok(())
    }

    #[test]
    fn it_errs_if_latest_started_period_ends_after_scheduled_launch(
    ) -> Result<()> {
        let harvest_mint = Pubkey::new_unique();
        let mut farm = Farm::default();

        farm.add_harvest(harvest_mint, Pubkey::new_unique())?;
        farm.new_harvest_period(
            Slot::new(5),
            harvest_mint,
            (Slot::new(5), Slot::new(25)),
            TokenAmount::new(20),
        )?;
        farm.new_harvest_period(
            Slot::new(10),
            harvest_mint,
            (Slot::new(30), Slot::new(50)),
            TokenAmount::new(20),
        )?;
        assert!(farm
            .new_harvest_period(
                Slot::new(10),
                harvest_mint,
                (Slot::new(20), Slot::new(50)),
                TokenAmount::new(20),
            )
            .is_err());

        Ok(())
    }

    #[test]
    fn it_changes_farming_period_if_not_started_yet() -> Result<()> {
        // As it stands now, the method `new_harvest_period` will not allow
        // for there to be two scheduled periods in the future. If we call
        // the method with a second period, p2, in the future the method will
        // substitute the future period, p1, that was already in the struct.
        let mut farm = Farm::default();

        let harvest_mint = farm.harvests[0].mint;

        // update first entry to be in the future
        farm.harvests[0].periods[0] = HarvestPeriod {
            starts_at: Slot::new(10),
            ends_at: Slot::new(20),
            tps: TokenAmount::new(10),
        };
        // call new_harvest_period method which should overwrite it
        farm.new_harvest_period(
            Slot::new(5),
            harvest_mint,
            (Slot::new(15), Slot::new(25)),
            TokenAmount::new(20),
        )?;
        assert_eq!(
            farm.harvests[0].periods[0],
            HarvestPeriod {
                starts_at: Slot::new(15),
                ends_at: Slot::new(25),
                tps: TokenAmount::new(20)
            }
        );

        for i in 1..9 {
            assert_eq!(
                farm.harvests[0].periods[i],
                HarvestPeriod {
                    starts_at: Slot::new(0),
                    ends_at: Slot::new(0),
                    tps: TokenAmount::new(0)
                }
            );
        }

        Ok(())
    }

    #[test]
    fn it_can_add_new_period_only_if_history_not_full() {
        let mut farm = Farm::default();

        let harvest_mint = farm.harvests[0].mint;
        assert_eq!(farm.oldest_snapshot().started_at, Slot::new(0));

        farm.harvests[0].periods =
            [10, 9, 8, 7, 6, 5, 4, 3, 2, 1].map(|u| HarvestPeriod {
                starts_at: Slot::new(u * 10),
                ends_at: Slot::new(u * 10 + 5),
                tps: TokenAmount::new(100 * u),
            });

        let output = farm.new_harvest_period(
            Slot::new(150),
            harvest_mint,
            (Slot::new(160), Slot::new(165)),
            TokenAmount::new(10),
        );
        assert!(output.is_err());

        farm.snapshots.ring_buffer[farm.oldest_snapshot_index()] = Snapshot {
            staked: TokenAmount::new(0),
            started_at: Slot::new(140),
        };
        let output = farm.new_harvest_period(
            Slot::new(150),
            harvest_mint,
            (Slot::new(160), Slot::new(165)),
            TokenAmount::new(10),
        );
        assert!(output.is_ok());
        assert_eq!(
            farm.harvests[0].periods[0],
            HarvestPeriod {
                starts_at: Slot::new(160),
                ends_at: Slot::new(165),
                tps: TokenAmount::new(10),
            }
        );
        assert_eq!(
            farm.harvests[0].periods[9],
            HarvestPeriod {
                starts_at: Slot::new(20),
                ends_at: Slot::new(25),
                tps: TokenAmount::new(200),
            }
        );
    }

    #[test]
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

        let snapshot_iter = farm.get_window_snapshots_eligible_to_harvest(
            calculate_next_harvest_from,
        );

        assert_eq!(snapshot_iter.count(), consts::SNAPSHOTS_LEN);

        Ok(())
    }

    #[test]
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

        let snapshot_iter = farm.get_window_snapshots_eligible_to_harvest(
            calculate_next_harvest_from,
        );

        assert_eq!(snapshot_iter.count(), 501);

        Ok(())
    }

    #[test]
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

        let snapshot_iter = farm.get_window_snapshots_eligible_to_harvest(
            calculate_next_harvest_from,
        );

        assert_eq!(snapshot_iter.count(), consts::SNAPSHOTS_LEN);

        Ok(())
    }

    #[test]
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
