//! User's representation of their position.

use crate::prelude::*;
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::iter;
use std::{cmp, mem};

/// A user has only a single account for farming per `V` (see design docs). This
/// minimizes the number of accounts one needs to provide to transactions and
/// therefore enables single transaction claim. This account tracks everything
/// related to the farmer's stake.
#[account]
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Farmer {
    /// This signer can claim harvest, start/stop farming.
    pub authority: Pubkey,
    /// What farm is this farmer associated with.
    pub farm: Pubkey,
    /// How many tokens are currently _earning_ harvest. Upon stake, tokens are
    /// firstly added to the `vested` amount and only in the next
    /// snapshot window are they added to the `staked_amount`.
    pub staked: TokenAmount,
    /// Upon staking (start farming), tokens are added to this amount. Only on
    /// the next snapshot are these tokens added to the `staked_amount` and are
    /// eligible for harvest.
    pub vested: TokenAmount,
    /// To know whether these tokens are already eligible for harvest, we store
    /// the slot at which were they deposited. This way we know upon the next
    /// action this slot is less than the last snapshot's end slot. See the
    /// [`Farmer::check_vested_period_and_update_harvest`] method.
    ///
    /// # Important
    /// This value is not changed after moving `vested` to
    /// `staked`, only when staking.
    pub vested_at: Slot,
    /// The slot from which the we can start calculating the reamining eligible
    /// harvest from. In other words, upon calculating the available harvest
    /// for the farmer, we set this value to the current slot + 1
    ///
    /// # Important
    /// This value is inclusive.
    /// If calculate_next_harvest_from.slot == 5 and the current slot == 8,
    /// then we need to calculate the harvest for slots 5, 6, 7.
    pub calculate_next_harvest_from: Slot,
    /// Stores how many tokens is the farmer eligible for harvest since
    /// the slot prior to the `calculate_next_harvest_from` slot.
    ///
    /// These values are incremented by
    /// [`crate::endpoints::update_eligible_harvest`]. Its main
    /// purpose is to allow us to claim harvest by 3rd party bots on behalf of
    /// the farmer. The bots increment these integers and when the farmer is
    /// ready to claim their harvest, we perform the actual transfer with token
    /// program.
    ///
    /// # Note
    /// Len must match [`consts::MAX_HARVEST_MINTS`].
    ///
    /// # Important
    /// There's no particular order to the harvest mints below, only guarantee
    /// is a uniqueness of pubkeys unless [`Pubkey::default`].
    pub harvests: [AvailableHarvest; 10],
}

/// Since there are multiple harvestable mints, this must be an array. The
/// mint tells us for which token mint does the associated integer apply.
///
/// If the pubkey is equal to [`Pubkey::default`], then this representation is
/// not initialized yet.
#[derive(
    AnchorDeserialize,
    AnchorSerialize,
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
)]
pub struct AvailableHarvest {
    pub mint: Pubkey,
    pub tokens: TokenAmount,
}

impl Farmer {
    /// farmer account prefix constant
    pub const ACCOUNT_PREFIX: &'static [u8; 6] = b"farmer";

    /// Returns mutable reference to the amount of tokens the farmer is eligible
    /// for, or [`None`] if there doesn't exist the particular mint.
    pub fn get_harvest_mut(
        &mut self,
        mint: Pubkey,
    ) -> Option<&mut TokenAmount> {
        self.harvests
            .iter_mut()
            .find(|h| h.mint == mint)
            .map(|h| &mut h.tokens)
    }

    pub fn add_to_vested(
        &mut self,
        current_slot: Slot,
        tokens: TokenAmount,
    ) -> Result<()> {
        self.vested_at = current_slot;
        self.vested.amount = self
            .vested
            .amount
            .checked_add(tokens.amount)
            .ok_or(FarmingError::MathOverflow)?;

        Ok(())
    }

    pub fn total_deposited(&self) -> Result<TokenAmount> {
        self.staked
            .amount
            .checked_add(self.vested.amount)
            .ok_or_else(|| error!(FarmingError::MathOverflow))
            .map(TokenAmount::new)
    }

    pub fn unstake(&mut self, max: TokenAmount) -> Result<TokenAmount> {
        if self.vested >= max {
            self.vested.amount -= max.amount;
            Ok(max)
        } else {
            let total = self.total_deposited()?;
            if total > max {
                self.staked.amount -= max.amount - self.vested.amount;
                self.vested.amount = 0;
                Ok(max)
            } else {
                self.staked.amount = 0;
                self.vested.amount = 0;
                Ok(total)
            }
        }
    }

    /// Moves funds from vested to staked if possible and then calculates
    /// harvest since last call.
    pub fn check_vested_period_and_update_harvest(
        &mut self,
        farm: &Farm,
        current_slot: Slot,
    ) -> Result<()> {
        if self.vested.amount != 0 {
            // if there was a new snapshot, vested funds are ready to be
            // moved to staked funds
            //
            // the snapshot can never have started at slot = 0, because we're
            // looking for a snapshot with started_at _greater_ than vested_at
            if let Some(snapshot) = farm.first_snapshot_after(self.vested_at) {
                debug_assert!(
                    self.vested_at <= self.calculate_next_harvest_from
                );
                // calculate harvest for which the vested tokens should not be
                // counted yet, ie. all harvest until first snapshot after
                // vested at slot
                let farmer_harvests = self.eligible_harvest_until(
                    farm,
                    Slot::new(snapshot.started_at.slot - 1),
                )?;
                self.set_harvests(farmer_harvests)?;
                // and take a note that we calculated harvest until this
                // point
                self.calculate_next_harvest_from = snapshot.started_at;

                // mark funds which are beyond vesting period as staked
                self.staked.amount = self
                    .staked
                    .amount
                    .checked_add(self.vested.amount)
                    .ok_or(FarmingError::MathOverflow)?;
                self.vested = TokenAmount { amount: 0 };
            }
        }

        // and then use the staked funds to calculate harvest until this slot
        self.update_eligible_harvest(farm, current_slot)?;

        Ok(())
    }

    /// Withdraw all tokens of given stake mint and return how much that was.
    pub fn claim_harvest(&mut self, stake_mint: Pubkey) -> Result<TokenAmount> {
        let harvest =
            self.harvests
                .iter_mut()
                .find(|h| h.mint == stake_mint)
                .ok_or(FarmingError::CannotCompoundIfStakeMintIsNotHarvest)?;

        let stake = harvest.tokens;

        harvest.tokens.amount = 0;

        Ok(stake)
    }

    /// Calculates how many tokens for each harvest mint is the farmer eligible
    /// for by iterating over the snapshot history (if the farmer last harvest
    /// was before last snapshot) and then calculating it in the open window
    /// too.
    fn update_eligible_harvest(
        &mut self,
        farm: &Farm,
        current_slot: Slot,
    ) -> Result<()> {
        // there is no eligible harvest available if the last calculation
        // has happened on the current slot or after, therefore we skip
        if self.calculate_next_harvest_from >= current_slot {
            return Ok(());
        }

        let farmer_harvests =
            self.eligible_harvest_until(farm, current_slot)?;

        // convert the map back into an array
        self.set_harvests(farmer_harvests)?;

        // plus one because calculation is _inclusive_ of the current slot
        self.calculate_next_harvest_from.slot = current_slot.slot + 1;

        Ok(())
    }

    fn eligible_harvest_until(
        &self,
        farm: &Farm,
        until: Slot,
    ) -> Result<BTreeMap<Pubkey, TokenAmount>> {
        let farm_harvests: BTreeMap<_, _> =
            farm.harvests.iter().map(|h| (h.mint, h)).collect();
        let mut farmer_harvests: BTreeMap<_, _> =
            self.harvests.iter().map(|h| (h.mint, h.tokens)).collect();

        sync_harvest_mints(&farm_harvests, &mut farmer_harvests);

        let snapshots = farm
            .get_window_snapshots_eligible_to_harvest(
                self.calculate_next_harvest_from,
            )
            // the snapshots are in DESC order, skip those which started after
            // the "until" slot, ie. the max slot we're interested in
            .skip_while(|snapshot| snapshot.started_at > until);

        eligible_harvest_until(
            &farm_harvests,
            snapshots,
            &mut farmer_harvests,
            (self.calculate_next_harvest_from, until),
            self.staked,
        )?;

        Ok(farmer_harvests)
    }

    /// Sets given map of harvest mint pubkeys keys and corresponding earned
    /// token amounts in that harvest as the array of harvests on the farmer's
    /// account.
    ///
    /// The map mustn't contain more entries than the
    /// [`consts::MAX_HARVEST_MINTS`]. That's a global program invariant
    /// enforced by [`sync_harvest_mints`].
    ///
    /// If the map contains less entries, we pad the rest with
    /// `(Pubkey::default(), TokenAmount::new(0))`.
    pub fn set_harvests(
        &mut self,
        harvests: impl IntoIterator<Item = (Pubkey, TokenAmount)>,
    ) -> Result<()> {
        self.harvests = harvests
            .into_iter()
            .map(|(mint, tokens)| AvailableHarvest { mint, tokens })
            // pad with uninitialized harvests
            .chain(iter::repeat_with(|| AvailableHarvest {
                mint: Pubkey::default(),
                tokens: TokenAmount::default(),
            }))
            .take(consts::MAX_HARVEST_MINTS)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| {
                msg!("Cannot convert farmer harvest vector into array");
                FarmingError::InvariantViolation
            })?;

        Ok(())
    }

    /// Calculates a farmer's bytes space
    pub fn space() -> usize {
        const DISCRIMINANT: usize = 8;
        const PUBKEY: usize = mem::size_of::<Pubkey>();

        let authority = PUBKEY;
        let farm = PUBKEY;
        let staked = 8;
        let vested = 8;
        let vested_at = 8;
        let harvest_calculated_until = 8;
        let harvests = consts::MAX_HARVEST_MINTS * (PUBKEY + 8);

        DISCRIMINANT
            + authority
            + farm
            + vested_at
            + harvest_calculated_until
            + staked
            + vested
            + harvests
    }
}

/// Calculates how many tokens for each harvest mint is the farmer eligible for.
///
/// The `period` first member is the oldest slot (inclusive) for which to accrue
/// harvest.
///
/// The `period` second member is the most recent slot (inclusive) for which to
/// accrue the harvest.
///
/// The `snapshots` parameter must be in _reverse_ chronological order, starting
/// with the most recent snapshot that's happened _before_ the `until` slot.
///
/// This method mutates the `farmer_harvests` map and _adds_ the harvest
/// eligible in the period to the amounts already stored in the map.
///
/// ref. eq. (1), ref. eq. (2)
fn eligible_harvest_until<'a>(
    farm_harvests: &BTreeMap<Pubkey, &Harvest>,
    snapshots: impl Iterator<Item = &'a Snapshot>,
    farmer_harvests: &mut BTreeMap<Pubkey, TokenAmount>,
    period: (Slot, Slot),
    farmer_staked: TokenAmount,
) -> Result<()> {
    if farmer_staked.amount == 0 {
        // This method updates farmer's harvest tokens. If the farmer has no
        // staked tokens, there won't be any update as their share is always
        // zero. Therefore, we can return early.
        return Ok(());
    }

    let (since, until) = period;

    // we iterate over the snapshots in DESC order
    //
    // this is updated after each snapshot with the slot at which that snapshot
    // starts, so that in the next iteration we can calculate the length of a
    // snapshot
    let mut oldest_slot_to_skip = Slot::new(until.slot + 1);

    // collect vecs of ranges over which are specific tps valid for each
    // harvestable mint
    //
    // the ranges within the vecs are ordered from the latest in time to the
    // oldest, they don't overlap
    //
    // we pop the last entry every time we iterate over the last snapshot which
    // that entry is relevant for
    let mut harvest_tps_histories: BTreeMap<_, Vec<_>> = farm_harvests
        .iter()
        .map(|(mint, harvest)| (mint, harvest.tps_history(until)))
        .collect();

    // filter out uninitialized snapshots
    for snapshot in snapshots.filter(|s| s.started_at.slot > 0) {
        // we process snapshots in reverse order
        debug_assert!(oldest_slot_to_skip >= snapshot.started_at);
        if snapshot.staked.amount == 0 {
            oldest_slot_to_skip = snapshot.started_at;
            continue;
        }

        // never process history twice
        //
        // Last time the calculation probably happened during a snapshot, so
        // this ensures that only the unprocessed snapshot part is considered.
        // This max is relevant only for the oldest snapshot.
        let starts_at =
            Slot::new(cmp::max(snapshot.started_at.slot, since.slot));
        // we filtered out snapshots which begin on slot 0, so
        // oldest_slot_to_skip cannot be 0
        let ends_at = Slot::new(oldest_slot_to_skip.slot - 1);

        // sum over all farmers' share is 1
        let farmer_share = Decimal::from(farmer_staked.amount)
            .try_div(Decimal::from(snapshot.staked.amount))?;
        debug_assert_ne!(farmer_share, Decimal::zero());

        for farm_harvest in farm_harvests
            .values()
            .filter(|h| h.mint != Pubkey::default())
        {
            // we can unwrap because we created this BTreeMap by mapping _all_
            // keys in the farm_harvests BTreeMap
            let history =
                harvest_tps_histories.get_mut(&farm_harvest.mint).unwrap();

            // Since a single snapshot can contain multiple harvest periods
            // (very unlikely but we must account for this possibility),
            // we keep popping the last entry (latest period) until we hit a
            // period which is going to be relevant also for the next _iterated_
            // snapshot, ie. one snapshot earlier than the currently iterated.
            let mut calculate_until_slot = ends_at.slot;
            let mut eligible_harvest = Decimal::zero();
            while let Some((range, tps)) = history.last() {
                // the period ends before this snapshot starts
                if range.end() < &starts_at {
                    break;
                }
                // the period starts after this snapshot ends, it won't be
                // used by any other snapshot (we iterate in DESC order), so
                // pop it from the queue
                if range.start() > &ends_at {
                    history.pop();
                    continue;
                }

                if tps.amount != 0 {
                    let slots = range
                        .end()
                        .slot
                        .min(calculate_until_slot)
                        .checked_sub(range.start().slot.max(starts_at.slot))
                        // This should never happen, since we skip the
                        // this function call whenever
                        // calculate_next_harvest_from >= current_slot
                        .ok_or(FarmingError::MathOverflow)?
                        + 1; // +1 bcs inclusiveness
                    eligible_harvest = eligible_harvest.try_add(
                        Decimal::from(slots)
                            .try_mul(farmer_share)?
                            .try_mul(Decimal::from(tps.amount))?,
                    )?;
                }

                if range.start() >= &starts_at && range.start().slot != 0 {
                    // -1 bcs inclusiveness
                    calculate_until_slot = range.start().slot - 1;

                    // process all history which will not be relevant for older
                    // snapshots
                    //
                    // IMPORTANT: we iterate over snapshots in reverse order
                    history.pop();
                } else {
                    // the latest range spans snapshots which are yet to be
                    // processed, don't pop it
                    break;
                }
            }

            let farmer_harvest =
                farmer_harvests.entry(farm_harvest.mint).or_default();
            *farmer_harvest = TokenAmount {
                amount: farmer_harvest
                    .amount
                    .checked_add(eligible_harvest.try_floor()?)
                    .ok_or(FarmingError::MathOverflow)?,
            };
        }

        oldest_slot_to_skip = snapshot.started_at;
    }

    Ok(())
}

// 1. Gets rid of any (admin) deleted harvest mints
// 2. Inserts newly (admin) added harvest mints
fn sync_harvest_mints(
    farm_harvests: &BTreeMap<Pubkey, &Harvest>,
    farmer_harvests: &mut BTreeMap<Pubkey, TokenAmount>,
) {
    // 1.
    farmer_harvests.retain(|farmer_harvest_mint, _| {
        farm_harvests.contains_key(farmer_harvest_mint)
    });

    // 2.
    for farm_harvest_mint in farm_harvests.keys() {
        if !farmer_harvests.contains_key(farm_harvest_mint) {
            farmer_harvests.insert(*farm_harvest_mint, TokenAmount::default());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::utils::*;
    use serial_test::serial;

    impl Farmer {
        fn get_harvest(&self, mint: Pubkey) -> TokenAmount {
            self.harvests
                .iter()
                .find_map(|h| h.mint.eq(&mint).then(|| h.tokens))
                .expect("farmer has no harvest of such mint")
        }
    }

    #[test]
    fn it_has_stable_size() {
        assert_eq!(Farmer::space(), 504);
    }

    #[test]
    fn it_claims_harvest() {
        let mint = Pubkey::new_unique();

        let mut farmer = Farmer {
            staked: TokenAmount { amount: 10 },
            vested: TokenAmount { amount: 10 },
            vested_at: Slot { slot: 0 },
            harvests: generate_farmer_harvests(&mut vec![(mint, 100)])
                .try_into()
                .unwrap(),
            ..Default::default()
        };

        assert_eq!(farmer.harvests[0].mint, mint);
        assert_eq!(farmer.harvests[0].tokens, TokenAmount { amount: 100 });

        let amount_claimed = farmer.claim_harvest(mint).unwrap();

        assert_eq!(farmer.harvests[0].tokens, TokenAmount { amount: 0 });
        assert_eq!(amount_claimed, TokenAmount { amount: 100 });
    }

    #[test]
    fn it_does_not_claim_harvest_if_nothing_to_claim() {
        let mint = Pubkey::new_unique();

        let mut farmer = Farmer {
            staked: TokenAmount { amount: 0 },
            vested: TokenAmount { amount: 0 },
            vested_at: Slot { slot: 0 },
            harvests: generate_farmer_harvests(&mut vec![(mint, 0)])
                .try_into()
                .unwrap(),
            ..Default::default()
        };

        assert_eq!(farmer.harvests[0].mint, mint);
        assert_eq!(farmer.harvests[0].tokens, TokenAmount { amount: 0 });

        let amount_claimed = farmer.claim_harvest(mint).unwrap();

        assert_eq!(farmer.harvests[0].tokens, TokenAmount { amount: 0 });
        assert_eq!(amount_claimed, TokenAmount { amount: 0 });
    }

    #[test]
    fn it_fails_to_claim_harvest_if_mint_mismatch() {
        let mint = Pubkey::new_unique();
        let wrong_mint = Pubkey::new_unique();

        let mut farmer = Farmer {
            staked: TokenAmount { amount: 10 },
            vested: TokenAmount { amount: 10 },
            vested_at: Slot { slot: 0 },
            harvests: generate_farmer_harvests(&mut vec![(mint, 100)])
                .try_into()
                .unwrap(),
            ..Default::default()
        };

        assert!(farmer
            .claim_harvest(wrong_mint)
            .unwrap_err()
            .to_string()
            .contains("CannotCompoundIfStakeMintIsNotHarvest"))
    }

    #[test]
    fn it_matches_available_harvest_with_const() {
        let farmer = Farmer::default();

        assert_eq!(farmer.harvests.len(), consts::MAX_HARVEST_MINTS);
    }

    #[test]
    fn it_ignores_snapshots_earlier_than_calc_next_harvest_from() -> Result<()>
    {
        let (harvest_mint, farm) = dummy_farm_1()?;

        let mut farmer = Farmer {
            staked: TokenAmount::new(100),
            calculate_next_harvest_from: Slot::new(10),
            ..Default::default()
        };
        farmer.update_eligible_harvest(&farm, Slot::new(20))?;
        assert_eq!(
            farmer.get_harvest(harvest_mint),
            // 5 slots in 2nd period with 50% share
            // 6 slots in 3rd period with 25% share
            TokenAmount::new((5 * 20) / 2 + (6 * 30) / 4)
        );

        // calculates harvest in the open window only
        let mut farmer = Farmer {
            staked: TokenAmount::new(100),
            calculate_next_harvest_from: Slot::new(17),
            ..Default::default()
        };
        farmer.update_eligible_harvest(&farm, Slot::new(20))?;
        assert_eq!(
            farmer.get_harvest(harvest_mint),
            // 4 slots in 3rd period with 25% share
            TokenAmount::new((4 * 30) / 4)
        );

        Ok(())
    }

    #[test]
    fn it_ignores_snapshots_started_at_slot_0() -> Result<()> {
        let (harvest_mint, mut farm) = dummy_farm_1()?;
        farm.snapshots.ring_buffer[1].started_at = Slot::new(0);

        let mut farmer = Farmer {
            staked: TokenAmount::new(100),
            calculate_next_harvest_from: Slot::new(0),
            ..Default::default()
        };
        farmer.update_eligible_harvest(&farm, Slot::new(50))?;
        assert_eq!(
            farmer.get_harvest(harvest_mint),
            // 2nd period 7-9 slots, 100% share of 10 tps
            // 3rd period 10-14 slots, 50% share of 20 tps
            // 4th period 15-30 slots, 25% share of 30 tps
            TokenAmount::new((3 * 10) + (5 * 20) / 2 + (16 * 30) / 4)
        );

        Ok(())
    }

    #[test]
    fn it_ignores_snapshots_with_total_staked_amount_0() -> Result<()> {
        let (harvest_mint, mut farm) = dummy_farm_1()?;
        farm.snapshots.ring_buffer[4].staked = TokenAmount::new(0);

        let mut farmer = Farmer {
            staked: TokenAmount::new(100),
            calculate_next_harvest_from: Slot::new(7),
            ..Default::default()
        };
        farmer.update_eligible_harvest(&farm, Slot::new(50))?;
        assert_eq!(
            farmer.get_harvest(harvest_mint),
            // 2nd period 7-9 slots, 100% share of 10 tps
            // 3rd period 10-14 slots, 50% share of 20 tps
            // 4th period should be ignored
            TokenAmount::new((3 * 10) + (5 * 20) / 2)
        );

        Ok(())
    }

    #[test]
    fn it_noops_on_farmer_stake_amount_0() -> Result<()> {
        let (harvest_mint, farm) = dummy_farm_1()?;

        let mut farmer = Farmer {
            staked: TokenAmount { amount: 0 },
            calculate_next_harvest_from: Slot::new(0),
            ..Default::default()
        };
        farmer.update_eligible_harvest(&farm, Slot::new(50))?;
        assert_eq!(farmer.get_harvest(harvest_mint), TokenAmount::new(0));

        Ok(())
    }

    #[test]
    fn it_calculates_harvest_from_halfway_claimed_snapshot() -> Result<()> {
        let (harvest_mint, farm) = dummy_farm_1()?;

        let mut farmer = Farmer {
            staked: TokenAmount::new(100),
            calculate_next_harvest_from: Slot::new(12),
            ..Default::default()
        };
        farmer.update_eligible_harvest(&farm, Slot::new(50))?;
        assert_eq!(
            farmer.get_harvest(harvest_mint),
            TokenAmount::new(
                // 3rd period, 12-14, 50% share of 20 tps
                // 4rd period, 15-30, 25% share of 30 tps
                (3 * 20) / 2 + (16 * 30 / 4)
            )
        );

        Ok(())
    }

    #[test]
    fn it_checks_vested_period_and_update_harvest() -> Result<()> {
        let (harvest_mint, farm) = dummy_farm_1()?;

        let mut farmer = Farmer {
            vested: TokenAmount::new(100),
            vested_at: Slot::new(12),
            calculate_next_harvest_from: Slot::new(14),
            ..Default::default()
        };
        farmer.check_vested_period_and_update_harvest(&farm, Slot::new(50))?;
        assert_eq!(
            farmer.get_harvest(harvest_mint),
            TokenAmount::new(
                // 4rd period, 15-30, 25% share of 30 tps
                16 * 30 / 4
            )
        );
        // Checks that vested amount has been moved staked
        assert_eq!(farmer.vested, TokenAmount::new(0));
        assert_eq!(farmer.staked, TokenAmount::new(100));

        Ok(())
    }

    #[test]
    fn it_checks_vested_period_and_does_not_update_harvest_when_still_in_vesting_snapshot(
    ) -> Result<()> {
        let (harvest_mint, farm) = dummy_farm_1()?;

        let mut farmer = Farmer {
            vested: TokenAmount::new(100),
            vested_at: Slot::new(16),
            calculate_next_harvest_from: Slot::new(14),
            ..Default::default()
        };
        farmer.check_vested_period_and_update_harvest(&farm, Slot::new(50))?;
        assert_eq!(farmer.get_harvest(harvest_mint), TokenAmount::new(0));
        assert_eq!(farmer.vested, TokenAmount::new(100));
        assert_eq!(farmer.staked, TokenAmount::new(0));

        Ok(())
    }

    #[test]
    fn it_errs_check_vested_period_and_update_harvest_if_vested_overflows_staked(
    ) -> Result<()> {
        let (_harvest_mint, farm) = dummy_farm_1()?;

        let mut farmer = Farmer {
            staked: TokenAmount { amount: u64::MAX },
            vested: TokenAmount { amount: u64::MAX },
            vested_at: Slot::new(12),
            calculate_next_harvest_from: Slot::new(14),
            ..Default::default()
        };
        assert!(farmer
            .check_vested_period_and_update_harvest(&farm, Slot::new(50))
            .is_err());

        Ok(())
    }

    #[test]
    fn it_calculates_reward_with_vested_funds_only_from_next_snapshot(
    ) -> Result<()> {
        let (harvest_mint, farm) = dummy_farm_1()?;

        let mut farmer = Farmer {
            staked: TokenAmount::new(0),
            vested: TokenAmount::new(100),
            vested_at: Slot::new(12),
            calculate_next_harvest_from: Slot::new(12),
            ..Default::default()
        };
        farmer.check_vested_period_and_update_harvest(&farm, Slot::new(50))?;
        assert_eq!(
            farmer.get_harvest(harvest_mint),
            TokenAmount::new(
                // 4rd period, 15-30, 25% share of 30 tps
                16 * 30 / 4
            )
        );
        assert_eq!(farmer.vested, TokenAmount::new(0));
        assert_eq!(farmer.staked, TokenAmount::new(100));

        Ok(())
    }

    #[test]
    fn it_ignores_uninitialized_harvests() -> Result<()> {
        let (harvest_mint, farm) = dummy_farm_1()?;

        let mut farmer = Farmer {
            staked: TokenAmount::new(100),
            calculate_next_harvest_from: Slot::new(0),
            ..Default::default()
        };
        farmer.check_vested_period_and_update_harvest(&farm, Slot::new(50))?;

        for harvest in farmer.harvests {
            if harvest.mint == harvest_mint {
                assert_ne!(harvest.tokens, TokenAmount::new(0));
            } else {
                assert_eq!(harvest.tokens, TokenAmount::new(0));
            }
        }

        Ok(())
    }

    #[test]
    fn it_works_with_three_periods_in_one_snapshot() -> Result<()> {
        let harvest_mint = Pubkey::new_unique();
        let mut farm = Farm::default();
        farm.min_snapshot_window_slots = 1;
        farm.add_harvest(harvest_mint, Pubkey::new_unique())?;

        farm.take_snapshot(Slot::new(1), TokenAmount::new(100))?;
        farm.new_harvest_period(
            Slot::new(1),
            harvest_mint,
            (Slot::new(1), Slot::new(3)),
            TokenAmount::new(1),
        )?;
        farm.new_harvest_period(
            Slot::new(11),
            harvest_mint,
            (Slot::new(11), Slot::new(13)),
            TokenAmount::new(10),
        )?;
        farm.new_harvest_period(
            Slot::new(31),
            harvest_mint,
            (Slot::new(31), Slot::new(33)),
            TokenAmount::new(100),
        )?;
        farm.take_snapshot(Slot::new(50), TokenAmount::new(100))?;

        let mut farmer = Farmer {
            staked: TokenAmount::new(100),
            calculate_next_harvest_from: Slot::new(0),
            ..Default::default()
        };
        farmer.check_vested_period_and_update_harvest(&farm, Slot::new(50))?;
        assert_eq!(
            farmer.get_harvest(harvest_mint),
            TokenAmount::new(
                // 1st period, 1-3, 100% share of 1 tps
                3 +
                // 2nd period, 11-13, 100% share of 10 tps
                30 +
                // 3nd period, 31-33, 100% share of 100 tps
                300
            )
        );

        Ok(())
    }

    #[test]
    fn it_is_idempotent_when_updating_harvest() -> Result<()> {
        let (harvest_mint, farm) = dummy_farm_1()?;

        let mut farmer = Farmer {
            vested: TokenAmount::new(100),
            vested_at: Slot::new(12),
            calculate_next_harvest_from: Slot::new(14),
            ..Default::default()
        };

        for i in 0..6 {
            farmer.check_vested_period_and_update_harvest(
                &farm,
                // two subsequent calls are made at the same slot due to
                // integer division
                Slot::new(100 + i / 2),
            )?;
            assert_eq!(
                farmer.get_harvest(harvest_mint),
                TokenAmount::new(
                    // never changes because farming period ended at slot 50
                    16 * 30 / 4
                )
            );
        }

        Ok(())
    }

    #[test]
    fn it_works_with_no_harvests() -> Result<()> {
        let mut farm = Farm::default();
        farm.min_snapshot_window_slots = 1;
        farm.add_harvest(Pubkey::new_unique(), Pubkey::new_unique())?;
        farm.take_snapshot(Slot::new(1), TokenAmount::new(100))?;
        farm.take_snapshot(Slot::new(10), TokenAmount::new(100))?;

        let mut farmer = Farmer {
            staked: TokenAmount::new(100),
            calculate_next_harvest_from: Slot::new(1),
            ..Default::default()
        };
        farmer.check_vested_period_and_update_harvest(&farm, Slot::new(20))?;

        for harvest in farmer.harvests {
            assert_eq!(harvest.tokens, TokenAmount::new(0));
        }

        Ok(())
    }

    #[test]
    fn it_skips_update_of_eligible_harvest_if_all_snapshots_empty() -> Result<()>
    {
        let mut farm = Farm::default();
        farm.min_snapshot_window_slots = 1;
        farm.add_harvest(Pubkey::new_unique(), Pubkey::new_unique())?;

        let mut farmer = Farmer {
            staked: TokenAmount { amount: 100 },
            calculate_next_harvest_from: Slot { slot: 0 },
            ..Default::default()
        };
        farmer.check_vested_period_and_update_harvest(&farm, Slot::new(20))?;

        for harvest in farmer.harvests {
            assert_eq!(harvest.tokens, TokenAmount::new(0));
        }

        Ok(())
    }

    #[test]
    fn it_deleted_mints_from_farmer_harvests() {
        let h = Harvest::default();

        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();
        let mint4 = Pubkey::new_unique();

        let farm_harvests: BTreeMap<_, _> =
            vec![(mint1, &h), (mint2, &h)].into_iter().collect();

        let mut farmer_harvests: BTreeMap<_, _> = vec![
            (mint1, TokenAmount::default()),
            (mint2, TokenAmount::default()),
            (mint3, TokenAmount::default()),
            (mint4, TokenAmount::default()),
        ]
        .into_iter()
        .collect();

        sync_harvest_mints(&farm_harvests, &mut farmer_harvests);

        assert_eq!(
            farmer_harvests,
            vec![
                (mint1, TokenAmount::default()),
                (mint2, TokenAmount::default()),
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn it_inserts_added_mints_into_farmer_harvests() {
        let h = Harvest::default();

        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();

        let farm_harvests: BTreeMap<_, _> =
            vec![(mint1, &h), (mint2, &h)].into_iter().collect();

        let mut farmer_harvests: BTreeMap<_, _> =
            vec![(mint1, TokenAmount::new(10))].into_iter().collect();

        sync_harvest_mints(&farm_harvests, &mut farmer_harvests);

        assert_eq!(
            farmer_harvests,
            vec![
                (mint1, TokenAmount::new(10)),
                (mint2, TokenAmount::default()),
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn it_adds_to_vested() -> Result<()> {
        let mut farmer = Farmer::default();

        farmer.add_to_vested(Slot::new(15), TokenAmount::new(10))?;

        assert_eq!(farmer.vested_at, Slot::new(15));
        assert_eq!(farmer.vested, TokenAmount::new(10));

        farmer.add_to_vested(Slot::new(20), TokenAmount::new(10))?;

        assert_eq!(farmer.vested_at, Slot::new(20));
        assert_eq!(farmer.vested, TokenAmount::new(20));

        assert!(farmer
            .add_to_vested(Slot::new(25), TokenAmount::new(u64::MAX))
            .is_err());

        Ok(())
    }

    #[test]
    fn it_unstakes_when_unstake_max_is_gt_vested() -> Result<()> {
        let mut farmer = Farmer::default();

        farmer.staked.amount = 30;
        farmer.add_to_vested(Slot::new(15), TokenAmount::new(10))?;

        farmer.unstake(TokenAmount::new(20))?;

        assert_eq!(farmer.vested, TokenAmount::new(0));
        assert_eq!(farmer.staked, TokenAmount::new(20));

        Ok(())
    }

    #[test]
    fn it_unstakes_when_unstake_max_is_lt_vested() -> Result<()> {
        let mut farmer = Farmer::default();

        farmer.vested.amount = 15;

        farmer.unstake(TokenAmount::new(10))?;

        assert_eq!(farmer.vested, TokenAmount::new(5));
        assert_eq!(farmer.staked, TokenAmount::new(0));

        Ok(())
    }

    #[test]
    #[serial]
    fn it_unstakes_when_unstake_max_is_eq_vested() -> Result<()> {
        let mut farmer = Farmer::default();
        set_clock(Slot::new(15));

        farmer.staked.amount = 30;
        farmer.vested.amount = 10;

        farmer.unstake(TokenAmount::new(10))?;

        assert_eq!(farmer.vested, TokenAmount::new(0));
        assert_eq!(farmer.staked, TokenAmount::new(30));

        Ok(())
    }

    #[test]
    fn it_unstakes_when_unstake_max_is_gt_vested_and_staked() -> Result<()> {
        let mut farmer = Farmer::default();

        farmer.staked.amount = 10;
        farmer.vested.amount = 10;

        farmer.unstake(TokenAmount::new(100))?;

        assert_eq!(farmer.vested, TokenAmount::new(0));
        assert_eq!(farmer.staked, TokenAmount::new(0));

        Ok(())
    }

    #[test]
    fn it_computes_total_deposited() -> Result<()> {
        let mut farmer = Farmer::default();

        let mut total_deposited = farmer.total_deposited()?;
        assert_eq!(total_deposited, TokenAmount::new(0));

        farmer.staked.amount = 30;
        farmer.vested.amount = 70;

        total_deposited = farmer.total_deposited()?;

        assert_eq!(total_deposited, TokenAmount::new(100));

        Ok(())
    }

    #[test]
    fn it_skips_eligible_harvest_if_starts_in_the_future() -> Result<()> {
        let mint = Pubkey::new_unique();

        let periods = generate_harvest_periods(&mut vec![(10, 1, 100)]);

        let farm = Farm {
            harvests: generate_farm_harvests(&mut vec![(
                mint,
                Pubkey::new_unique(),
                periods.try_into().unwrap(),
            )])
            .try_into()
            .unwrap(),
            snapshots: Snapshots {
                ring_buffer_tip: 2,
                ring_buffer: generate_snapshots(&mut vec![
                    (10, 5_000),
                    (20, 10_000),
                    (30, 10_000),
                ])
                .try_into()
                .unwrap(),
            },
            ..Default::default()
        };

        let mut farmer = Farmer {
            staked: TokenAmount::new(5_000),
            vested: TokenAmount::new(0),
            vested_at: Slot::new(0),
            calculate_next_harvest_from: Slot::new(55),
            harvests: generate_farmer_harvests(&mut vec![(mint, 0)])
                .try_into()
                .unwrap(),
            ..Default::default()
        };

        let current_slot = 35;
        farmer.update_eligible_harvest(&farm, Slot::new(current_slot))?;

        assert_eq!(farmer.calculate_next_harvest_from, Slot::new(55));
        assert_eq!(farmer.get_harvest(mint), TokenAmount::new(0));

        Ok(())
    }

    #[test]
    fn it_gets_eligible_harvest_when_starting_from_snapshot_slot() -> Result<()>
    {
        let mint = Pubkey::new_unique();

        let periods = generate_harvest_periods(&mut vec![(10, 1, 100)]);

        let farm = Farm {
            harvests: generate_farm_harvests(&mut vec![(
                mint,
                Pubkey::new_unique(),
                periods.try_into().unwrap(),
            )])
            .try_into()
            .unwrap(),
            snapshots: Snapshots {
                ring_buffer_tip: 2,
                ring_buffer: generate_snapshots(&mut vec![
                    (10, 5_000),
                    (20, 10_000),
                    (30, 10_000),
                ])
                .try_into()
                .unwrap(),
            },
            ..Default::default()
        };

        let mut farmer = Farmer {
            staked: TokenAmount::new(5_000),
            vested: TokenAmount::new(0),
            vested_at: Slot::new(0),
            calculate_next_harvest_from: Slot::new(20),
            harvests: generate_farmer_harvests(&mut vec![(mint, 0)])
                .try_into()
                .unwrap(),
            ..Default::default()
        };

        let current_slot = 35;
        farmer.update_eligible_harvest(&farm, Slot::new(current_slot))?;

        assert_eq!(
            farmer.calculate_next_harvest_from,
            Slot::new(current_slot + 1)
        );
        assert_eq!(farmer.get_harvest(mint), TokenAmount::new(50 + 30));

        Ok(())
    }

    #[test]
    fn it_gets_eligible_harvest_when_current_slot_in_a_snapshot_slot(
    ) -> Result<()> {
        let mint = Pubkey::new_unique();

        let periods = generate_harvest_periods(&mut vec![(10, 1, 100)]);

        let farm = Farm {
            harvests: generate_farm_harvests(&mut vec![(
                mint,
                Pubkey::new_unique(),
                periods.try_into().unwrap(),
            )])
            .try_into()
            .unwrap(),
            snapshots: Snapshots {
                ring_buffer_tip: 2,
                ring_buffer: generate_snapshots(&mut vec![
                    (10, 5_000),
                    (20, 10_000),
                    (30, 10_000),
                ])
                .try_into()
                .unwrap(),
            },
            ..Default::default()
        };

        let mut farmer = Farmer {
            staked: TokenAmount::new(5_000),
            vested: TokenAmount::new(0),
            vested_at: Slot::new(0),
            calculate_next_harvest_from: Slot::new(15),
            harvests: generate_farmer_harvests(&mut vec![(mint, 0)])
                .try_into()
                .unwrap(),
            ..Default::default()
        };

        let current_slot = 30;
        farmer.update_eligible_harvest(&farm, Slot::new(current_slot))?;

        assert_eq!(
            farmer.calculate_next_harvest_from,
            Slot::new(current_slot + 1)
        );
        assert_eq!(farmer.get_harvest(mint), TokenAmount::new(50 + 50 + 5));

        Ok(())
    }

    #[test]
    fn it_skips_eligible_harvest_if_all_snapshots_are_uninitialized(
    ) -> Result<()> {
        let mint = Pubkey::new_unique();

        let periods = generate_harvest_periods(&mut vec![(10, 1, 100)]);

        let farm = Farm {
            harvests: generate_farm_harvests(&mut vec![(
                mint,
                Pubkey::new_unique(),
                periods.try_into().unwrap(),
            )])
            .try_into()
            .unwrap(),
            snapshots: Snapshots {
                ring_buffer_tip: 0,
                ring_buffer: [Default::default(); consts::SNAPSHOTS_LEN],
            },
            ..Default::default()
        };

        let mut farmer = Farmer {
            staked: TokenAmount::new(5_000),
            vested: TokenAmount::new(0),
            vested_at: Slot::new(0),
            calculate_next_harvest_from: Slot::new(0),
            harvests: generate_farmer_harvests(&mut vec![(mint, 0)])
                .try_into()
                .unwrap(),
            ..Default::default()
        };

        let current_slot = 1;
        farmer.update_eligible_harvest(&farm, Slot::new(current_slot))?;

        assert_eq!(
            farmer.calculate_next_harvest_from,
            Slot::new(current_slot + 1)
        );
        assert_eq!(farmer.get_harvest(mint), TokenAmount::new(0));

        Ok(())
    }

    #[test]
    fn it_is_idempotent_in_calc_eligible_harvest() -> Result<()> {
        let mint = Pubkey::new_unique();

        let periods = generate_harvest_periods(&mut vec![(10, 1, 100)]);

        let farm = Farm {
            harvests: generate_farm_harvests(&mut vec![(
                mint,
                Pubkey::new_unique(),
                periods.try_into().unwrap(),
            )])
            .try_into()
            .unwrap(),
            snapshots: Snapshots {
                ring_buffer_tip: 2,
                ring_buffer: generate_snapshots(&mut vec![
                    (10, 5_000),
                    (20, 10_000),
                    (30, 10_000),
                ])
                .try_into()
                .unwrap(),
            },
            ..Default::default()
        };

        let mut farmer = Farmer {
            staked: TokenAmount::new(5_000),
            vested: TokenAmount::new(0),
            vested_at: Slot::new(0),
            calculate_next_harvest_from: Slot::new(15),
            harvests: generate_farmer_harvests(&mut vec![(mint, 0)])
                .try_into()
                .unwrap(),
            ..Default::default()
        };

        let current_slot = 35;
        farmer.update_eligible_harvest(&farm, Slot::new(current_slot))?;

        assert_eq!(
            farmer.calculate_next_harvest_from,
            Slot::new(current_slot + 1)
        );
        assert_eq!(farmer.get_harvest(mint), TokenAmount::new(50 + 50 + 30));

        farmer.update_eligible_harvest(&farm, Slot::new(current_slot))?;

        assert_eq!(
            farmer.calculate_next_harvest_from,
            Slot::new(current_slot + 1)
        );
        assert_eq!(farmer.get_harvest(mint), TokenAmount::new(50 + 50 + 30));

        Ok(())
    }

    #[test]
    fn it_gets_eligible_harvest_twice_over_time() -> Result<()> {
        let mint = Pubkey::new_unique();

        let periods = generate_harvest_periods(&mut vec![(10, 1, 100)]);

        let farm = Farm {
            harvests: generate_farm_harvests(&mut vec![(
                mint,
                Pubkey::new_unique(),
                periods.try_into().unwrap(),
            )])
            .try_into()
            .unwrap(),
            snapshots: Snapshots {
                ring_buffer_tip: 2,
                ring_buffer: generate_snapshots(&mut vec![
                    (10, 5_000),
                    (20, 10_000),
                    (30, 10_000),
                ])
                .try_into()
                .unwrap(),
            },
            ..Default::default()
        };

        let mut farmer = Farmer {
            staked: TokenAmount::new(5_000),
            vested: TokenAmount::new(0),
            vested_at: Slot::new(0),
            calculate_next_harvest_from: Slot::new(15),
            harvests: generate_farmer_harvests(&mut vec![(mint, 0)])
                .try_into()
                .unwrap(),
            ..Default::default()
        };

        let current_slot = 35;
        farmer.update_eligible_harvest(&farm, Slot::new(current_slot))?;

        assert_eq!(
            farmer.calculate_next_harvest_from,
            Slot::new(current_slot + 1)
        );
        assert_eq!(farmer.get_harvest(mint), TokenAmount::new(50 + 50 + 30));

        let current_slot = 39;
        farmer.update_eligible_harvest(&farm, Slot::new(current_slot))?;

        assert_eq!(
            farmer.calculate_next_harvest_from,
            Slot::new(current_slot + 1)
        );
        assert_eq!(farmer.get_harvest(mint), TokenAmount::new(50 + 50 + 50));

        Ok(())
    }

    #[test]
    fn it_handles_eligible_harvest_when_zero_tps() -> Result<()> {
        let mint = Pubkey::new_unique();

        let periods = generate_harvest_periods(&mut vec![(0, 1, 100)]);

        let farm = Farm {
            harvests: generate_farm_harvests(&mut vec![(
                mint,
                Pubkey::new_unique(),
                periods.try_into().unwrap(),
            )])
            .try_into()
            .unwrap(),
            snapshots: Snapshots {
                ring_buffer_tip: 2,
                ring_buffer: generate_snapshots(&mut vec![
                    (30, 20_000),
                    (40, 20_000),
                    (50, 20_000),
                ])
                .try_into()
                .unwrap(),
            },
            ..Default::default()
        };

        let mut farmer = Farmer {
            staked: TokenAmount::new(5_000),
            vested: TokenAmount::new(0),
            vested_at: Slot::new(0),
            calculate_next_harvest_from: Slot::new(30),
            harvests: generate_farmer_harvests(&mut vec![(mint, 0)])
                .try_into()
                .unwrap(),
            ..Default::default()
        };

        let current_slot = 51;
        farmer.update_eligible_harvest(&farm, Slot::new(current_slot))?;

        assert_eq!(
            farmer.calculate_next_harvest_from,
            Slot::new(current_slot + 1)
        );
        assert_eq!(farmer.get_harvest(mint), TokenAmount::new(0));

        Ok(())
    }

    #[test]
    fn it_gets_eligible_harvest_in_a_single_period() -> Result<()> {
        let mint = Pubkey::new_unique();

        let periods = generate_harvest_periods(&mut vec![(10, 1, 100)]);

        let farm = Farm {
            harvests: generate_farm_harvests(&mut vec![(
                mint,
                Pubkey::new_unique(),
                periods.try_into().unwrap(),
            )])
            .try_into()
            .unwrap(),
            snapshots: Snapshots {
                ring_buffer_tip: 2,
                ring_buffer: generate_snapshots(&mut vec![
                    (10, 5_000),
                    (20, 10_000),
                    (30, 10_000),
                ])
                .try_into()
                .unwrap(),
            },
            ..Default::default()
        };

        let mut farmer = Farmer {
            staked: TokenAmount::new(5_000),
            vested: TokenAmount::new(0),
            vested_at: Slot::new(0),
            calculate_next_harvest_from: Slot::new(15),
            harvests: generate_farmer_harvests(&mut vec![(mint, 0)])
                .try_into()
                .unwrap(),
            ..Default::default()
        };

        let current_slot = 35;
        farmer.update_eligible_harvest(&farm, Slot::new(current_slot))?;

        assert_eq!(
            farmer.calculate_next_harvest_from,
            Slot::new(current_slot + 1)
        );
        assert_eq!(farmer.get_harvest(mint), TokenAmount::new(50 + 50 + 30));

        Ok(())
    }

    #[test]
    fn it_gets_eligible_harvest_in_a_multiple_periods() -> Result<()> {
        let mint = Pubkey::new_unique();

        let periods =
            generate_harvest_periods(&mut vec![(20, 20, 100), (10, 1, 20)]);

        let farm = Farm {
            harvests: generate_farm_harvests(&mut vec![(
                mint,
                Pubkey::new_unique(),
                periods.try_into().unwrap(),
            )])
            .try_into()
            .unwrap(),
            snapshots: Snapshots {
                ring_buffer_tip: 2,
                ring_buffer: generate_snapshots(&mut vec![
                    (10, 5_000),
                    (20, 10_000),
                    (30, 10_000),
                ])
                .try_into()
                .unwrap(),
            },
            ..Default::default()
        };

        let mut farmer = Farmer {
            staked: TokenAmount::new(5_000),
            vested: TokenAmount::new(0),
            vested_at: Slot::new(0),
            calculate_next_harvest_from: Slot::new(15),
            harvests: generate_farmer_harvests(&mut vec![(mint, 0)])
                .try_into()
                .unwrap(),
            ..Default::default()
        };

        let current_slot = 35;
        farmer.update_eligible_harvest(&farm, Slot::new(current_slot))?;

        assert_eq!(
            farmer.calculate_next_harvest_from,
            Slot::new(current_slot + 1)
        );
        assert_eq!(farmer.get_harvest(mint), TokenAmount::new(50 + 100 + 60));

        Ok(())
    }

    #[test]
    fn it_handles_eligible_harvest_when_user_share_is_zero() -> Result<()> {
        let mint = Pubkey::new_unique();

        let periods =
            generate_harvest_periods(&mut vec![(20, 20, 100), (10, 1, 20)]);

        let farm = Farm {
            harvests: generate_farm_harvests(&mut vec![(
                mint,
                Pubkey::new_unique(),
                periods.try_into().unwrap(),
            )])
            .try_into()
            .unwrap(),
            snapshots: Snapshots {
                ring_buffer_tip: 2,
                ring_buffer: generate_snapshots(&mut vec![
                    (10, 5_000),
                    (20, 10_000),
                    (30, 10_000),
                ])
                .try_into()
                .unwrap(),
            },
            ..Default::default()
        };

        let mut farmer = Farmer {
            staked: TokenAmount::new(0),
            vested: TokenAmount::new(0),
            vested_at: Slot::new(0),
            calculate_next_harvest_from: Slot::new(15),
            harvests: generate_farmer_harvests(&mut vec![(mint, 0)])
                .try_into()
                .unwrap(),
            ..Default::default()
        };

        let current_slot = 35;
        farmer.update_eligible_harvest(&farm, Slot::new(current_slot))?;

        assert_eq!(
            farmer.calculate_next_harvest_from,
            Slot::new(current_slot + 1)
        );
        assert_eq!(farmer.get_harvest(mint), TokenAmount::new(0));

        Ok(())
    }
}
