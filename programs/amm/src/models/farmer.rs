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
#[derive(Default, Debug, PartialEq)]
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
    /// [`Farmer::refresh`] method.
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
    /// [`crate::endpoints::farming::calculate_available_harvest`]. Its main
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

    pub fn add_to_vested(&mut self, tokens: TokenAmount) -> Result<()> {
        self.vested_at = Slot::current()?;
        self.vested.amount = self
            .vested
            .amount
            .checked_add(tokens.amount)
            .ok_or(AmmError::MathOverflow)?;

        Ok(())
    }

    pub fn total_deposited(&self) -> Result<TokenAmount> {
        self.staked
            .amount
            .checked_add(self.vested.amount)
            .ok_or_else(|| error!(AmmError::MathOverflow))
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
    ) -> Result<()> {
        // first mark funds which are beyond vesting period as staked
        self.update_vested(farm.latest_snapshot().started_at)?;
        // and then use the staked funds to calculate harvest until this slot
        self.update_eligible_harvest(farm)?;

        Ok(())
    }

    /// Checks if the vested tokens can be moved to staked tokens. This method
    /// must be called before any other action is taken regarding the farmer's
    /// account.
    fn update_vested(&mut self, last_snapshot_window_end: Slot) -> Result<()> {
        // Use "less than" instead of "less or equal than" because the taking
        // a snapshot (the endpoint that determines new window) is always called
        // first by definition (it's after all what determines new window).
        // After this endpoint the total staked amount for the upcoming snapshot
        // window is locked (see the docs), hence the tokens staked after the
        // instruction remain vested.
        if self.vested.amount != 0
            && self.vested_at.slot < last_snapshot_window_end.slot
        {
            self.staked.amount = self
                .staked
                .amount
                .checked_add(self.vested.amount)
                .ok_or(AmmError::MathOverflow)?;

            self.vested = TokenAmount { amount: 0 };
        }

        Ok(())
    }

    /// Calculates how many tokens for each harvest mint is the farmer eligible
    /// for by iterating over the snapshot history (if the farmer last harvest
    /// was before last snapshot) and then calculating it in the open window
    /// too.
    fn update_eligible_harvest(&mut self, farm: &Farm) -> Result<()> {
        let farm_harvests: BTreeMap<_, _> =
            farm.harvests.iter().map(|h| (h.mint, h)).collect();
        let mut farmer_harvests: BTreeMap<_, _> =
            self.harvests.iter().map(|h| (h.mint, h.tokens)).collect();

        sync_harvest_mints(&farm_harvests, &mut farmer_harvests);

        let snapshot_iter = farm.get_window_snapshots_eligible_to_harvest(
            self.calculate_next_harvest_from,
        );
        update_eligible_harvest_in_past_snapshots(
            &farm_harvests,
            snapshot_iter,
            &mut farmer_harvests,
            self.calculate_next_harvest_from,
            self.staked,
        )?;

        // We should only update calculate_next_harvest_from if the last
        // calculated slot is not after the latest snapshot slot. In
        // other words, if the last snapshot slot is 50 but the
        // calculate_next_harvest_from is already 55, then
        // we should keep it as it is. This guarantees that
        // calculate_next_harvest_from can never decrease, or in other
        // words, go back to the past.
        let last_snapshot_slot = farm.latest_snapshot().started_at;

        if last_snapshot_slot.slot > self.calculate_next_harvest_from.slot {
            self.calculate_next_harvest_from = last_snapshot_slot;
        }

        update_eligible_harvest_in_open_window(
            &farm_harvests,
            &farm.latest_snapshot(),
            &mut farmer_harvests,
            self.calculate_next_harvest_from,
            self.staked,
        )?;

        // convert the map back into an array
        self.set_harvests(farmer_harvests)?;

        // plus one because calculation is _inclusive_ of the current slot
        self.calculate_next_harvest_from.slot = Clock::get()?.slot + 1;

        Ok(())
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
                AmmError::InvariantViolation
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

/// Calculates farmer's share of tokens since the last snapshot was taken.
/// This method can be called as often as farmer pleases. The harvest is
/// scaled down by the number of slots passed since the last harvest.
///
/// This method updates the token value of the input `farmer_harvests` map.
///
/// If the time of farmer's last harvest was less than the open window,
/// the farmer would lose tokens, so we error. Firstly,
/// [`update_eligible_harvest_in_past_snapshots`] must be called.
///
/// ref. eq. (1)
fn update_eligible_harvest_in_open_window(
    farm_harvests: &BTreeMap<Pubkey, &Harvest>,
    open_window: &Snapshot,
    farmer_harvests: &mut BTreeMap<Pubkey, TokenAmount>,
    calculate_next_harvest_from: Slot,
    farmer_staked: TokenAmount,
) -> Result<()> {
    let current_slot = Clock::get()?.slot;

    if calculate_next_harvest_from.slot > current_slot {
        return Ok(());
    }

    if calculate_next_harvest_from.slot < open_window.started_at.slot {
        msg!("Calculate harvest of past snapshots first");
        // this would only happen if our logic is composed incorrectly
        return Err(error!(AmmError::InvariantViolation));
    }

    if open_window.staked.amount == 0 {
        return Ok(());
    }

    for farm_harvest in farm_harvests
        .values()
        // we're not interested in uninitialized harvests, although not
        // having this filter still works, it's a needless computation
        .filter(|h| h.mint != Pubkey::default())
    {
        let farmer_harvest_to_date =
            *farmer_harvests.get(&farm_harvest.mint).ok_or_else(|| {
                // should never happen if [`sync_harvest_mints`] is correct
                msg!("Harvests are not in sync");
                AmmError::InvariantViolation
            })?;

        //
        // ref. eq. (1)
        //
        let farmer_share = Decimal::from(farmer_staked.amount)
            .try_div(Decimal::from(open_window.staked.amount))?;
        let (tps, _) = farm_harvest.tokens_per_slot(open_window.started_at);
        // We don't have to check for underflow because of a condition in
        // the beginning of the method.
        let slots = (current_slot + 1) - calculate_next_harvest_from.slot;
        let eligible_harvest = Decimal::from(slots)
            .try_mul(Decimal::from(tps.amount))?
            .try_mul(farmer_share)?
            .try_floor_u64()?;

        farmer_harvests.insert(
            farm_harvest.mint,
            TokenAmount {
                amount: farmer_harvest_to_date
                    .amount
                    .checked_add(eligible_harvest)
                    .ok_or(AmmError::MathOverflow)?,
            },
        );
    }
    Ok(())
}

/// Calculates farmer's share of tokens in the past snapshots (not including
/// the open window).
///
/// This method updates the token value of the input `farmer_harvests` map.
///
/// Before calculating the farmer's harvest in the open window, the harvest
/// needs to be calculated in the past snapshots. It the farmer hasn't harvested
/// the eligible harvest form past snapshots, then this method needs to be
/// called prior to `[update_eligible_harvest_in_open_window]`.
///
/// ref. eq. (2)
fn update_eligible_harvest_in_past_snapshots<'a>(
    farm_harvests: &BTreeMap<Pubkey, &Harvest>,
    snapshots_iter: impl Iterator<Item = &'a Snapshot>,
    farmer_harvests: &mut BTreeMap<Pubkey, TokenAmount>,
    calculate_next_harvest_from: Slot,
    farmer_staked: TokenAmount,
) -> Result<()> {
    let mut snapshots_iter = snapshots_iter.peekable();

    if snapshots_iter.peek().is_none() {
        return Ok(());
    }

    let current_slot = Clock::get()?.slot;

    // This needs to be calculated outside the harvest loop because other it
    // will pop more than one snapshot, and we only want to pop the open window.
    // It's safe to unwrap as we have asserted that the snapshots_iter is not
    // empty
    let initial_next_slot_started_at =
        snapshots_iter.next().unwrap().started_at.slot;

    // Skip calculation if calculate_next_harvest_from is in the future
    if calculate_next_harvest_from.slot >= current_slot {
        return Ok(());
    }

    // We initiliase this variable to the first snapshot.started.slot in the
    // iterator. As the iterator is in reverse order this should
    // correspond to the open_window slot
    let mut next_slot_started_at = initial_next_slot_started_at;

    for snapshot in snapshots_iter.filter(|s| s.staked.amount > 0) {
        // For the oldest snapshot in the snapshots iter it is likely that
        // it started at a slot that is smaller than
        // calculate_next_harvest_from.slot Therefore we chose
        // the maximum between the two values to make sure
        // that we do not account for slots that its harvers has been
        // already calculated
        let start_at_slot = cmp::max(
            snapshot.started_at.slot,
            calculate_next_harvest_from.slot,
        );

        // The only way for this condition not to be met is if there are
        // older snapshots in the iterator, that have finished
        // before calculate_next_harvest_from.slot This should
        // never happen because we control what snapshots are in the
        // iterator by making sure the parameter comes from the
        // output of [`get_window_snapshots_eligible_to_harvest`]
        if next_slot_started_at > start_at_slot {
            let farmer_share = Decimal::from(farmer_staked.amount)
                .try_div(Decimal::from(snapshot.staked.amount))?;

            // We don't have to check for overflow because this calculation
            // is wrapped in the condition that makes slot
            // positive only.
            let slots = next_slot_started_at - start_at_slot;

            // OPTIMIZE: https://gitlab.com/crypto_project/defi/amm/-/issues/34
            for farm_harvest in farm_harvests
                .values()
                .filter(|h| h.mint != Pubkey::default())
            {
                let farmer_harvest_to_date = *farmer_harvests
                    .get(&farm_harvest.mint)
                    .ok_or_else(|| {
                        // should never if [`sync_harvest_mints`] is correct
                        msg!("Harvests are not in sync");
                        AmmError::InvariantViolation
                    })?;

                // OPTIMIZE: https://gitlab.com/crypto_project/defi/amm/-/issues/34
                let (tps, _) =
                    farm_harvest.tokens_per_slot(snapshot.started_at);

                // We don't have to check for underflow because of a condition
                // in the beginning of the method.
                let eligible_harvest = Decimal::from(slots)
                    .try_mul(farmer_share)?
                    .try_mul(Decimal::from(tps.amount))?
                    .try_floor_u64()?;

                farmer_harvests.insert(
                    farm_harvest.mint,
                    TokenAmount {
                        amount: farmer_harvest_to_date
                            .amount
                            .checked_add(eligible_harvest)
                            .ok_or(AmmError::MathOverflow)?,
                    },
                );
            }
        }

        next_slot_started_at = snapshot.started_at.slot;
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

    #[test]
    fn it_has_stable_size() {
        assert_eq!(Farmer::space(), 504);
    }

    #[test]
    fn it_doesnt_update_vested_if_vesting_amount_is_zero() {
        let mut farmer = Farmer {
            staked: TokenAmount { amount: 0 },
            vested: TokenAmount { amount: 0 },
            vested_at: Slot { slot: 0 },
            ..Default::default()
        };
        let farmer_before_refresh = farmer.clone();

        assert!(farmer.update_vested(Slot { slot: 1 }).is_ok());
        assert_eq!(farmer, farmer_before_refresh);
    }

    #[test]
    fn it_doesnt_update_vested_if_vested_at_is_eq_or_gt_than_last_snapshot_window_end(
    ) {
        let mut farmer = Farmer {
            staked: TokenAmount { amount: 0 },
            vested: TokenAmount { amount: 0 },
            vested_at: Slot { slot: 5 },
            ..Default::default()
        };
        let farmer_before_refresh = farmer.clone();

        assert!(farmer.update_vested(Slot { slot: 5 }).is_ok());
        assert_eq!(farmer, farmer_before_refresh);

        assert!(farmer.update_vested(Slot { slot: 6 }).is_ok());
        assert_eq!(farmer, farmer_before_refresh);
    }

    #[test]
    fn it_errs_update_vested_if_vested_overflows_staked() {
        let mut farmer = Farmer {
            staked: TokenAmount { amount: u64::MAX },
            vested: TokenAmount { amount: u64::MAX },
            vested_at: Slot { slot: 0 },
            ..Default::default()
        };

        assert!(farmer.update_vested(Slot { slot: 6 }).is_err());
    }

    #[test]
    fn it_updates_vested() {
        let mut farmer = Farmer {
            staked: TokenAmount { amount: 10 },
            vested: TokenAmount { amount: 10 },
            vested_at: Slot { slot: 0 },
            ..Default::default()
        };

        assert!(farmer.update_vested(Slot { slot: 5 }).is_ok());

        assert_eq!(farmer.vested_at, Slot { slot: 0 });
        assert_eq!(farmer.vested, TokenAmount { amount: 0 });
        assert_eq!(farmer.staked, TokenAmount { amount: 20 });
    }

    #[test]
    fn it_matches_available_harvest_with_const() {
        let farmer = Farmer::default();

        assert_eq!(farmer.harvests.len(), consts::MAX_HARVEST_MINTS);
    }

    #[test]
    #[serial]
    fn it_skips_available_harvest_in_open_window_calculation_if_slot_gt_or_eq_to_current_slot(
    ) -> Result<()> {
        let farm_harvests = BTreeMap::default();
        let mut farmer_harvests = BTreeMap::default();
        let snapshot = Snapshot {
            staked: TokenAmount { amount: 0 },
            started_at: Slot { slot: 5 },
        };

        set_clock(Slot { slot: 9 });
        update_eligible_harvest_in_open_window(
            &farm_harvests,
            &snapshot,
            &mut farmer_harvests,
            Slot { slot: 10 },
            Default::default(),
        )?;
        assert_eq!(farmer_harvests, BTreeMap::default());

        set_clock(Slot { slot: 10 });
        update_eligible_harvest_in_open_window(
            &farm_harvests,
            &snapshot,
            &mut farmer_harvests,
            Slot { slot: 10 },
            Default::default(),
        )?;
        assert_eq!(farmer_harvests, BTreeMap::default());

        Ok(())
    }

    #[test]
    #[serial]
    fn it_errs_to_calc_available_harvest_in_open_window_if_past_snapshots_havent_been_accounted_for_yet(
    ) {
        set_clock(Slot { slot: 9 });

        let farm_harvests = BTreeMap::default();
        let mut farmer_harvests = BTreeMap::default();

        assert!(update_eligible_harvest_in_open_window(
            &farm_harvests,
            &Snapshot {
                staked: TokenAmount { amount: 0 },
                started_at: Slot { slot: 5 },
            },
            &mut farmer_harvests,
            Slot { slot: 3 },
            Default::default(),
        )
        .unwrap_err()
        .to_string()
        .contains("InvariantViolation"));
    }

    #[test]
    #[serial]
    fn it_works_with_no_harvests() -> Result<()> {
        set_clock(Slot { slot: 9 });

        let farm_harvests = BTreeMap::default();
        let mut farmer_harvests = BTreeMap::default();

        update_eligible_harvest_in_open_window(
            &farm_harvests,
            &Snapshot {
                staked: TokenAmount { amount: 10 },
                started_at: Slot { slot: 5 },
            },
            &mut farmer_harvests,
            Slot { slot: 5 },
            TokenAmount { amount: 5 },
        )?;
        assert_eq!(farmer_harvests, BTreeMap::default());

        update_eligible_harvest_in_open_window(
            &farm_harvests,
            &Snapshot {
                staked: TokenAmount { amount: 10 },
                started_at: Slot { slot: 5 },
            },
            &mut farmer_harvests,
            Slot { slot: 6 },
            TokenAmount { amount: 5 },
        )?;
        assert_eq!(farmer_harvests, BTreeMap::default());

        Ok(())
    }

    #[test]
    #[serial]
    fn it_updates_harvest_in_open_window() -> Result<()> {
        let mint1 = Pubkey::new_unique();
        let harvest1_rho = 10;
        let harvest1 = Harvest {
            mint: mint1,
            vault: Default::default(),
            tokens_per_slot: [TokensPerSlotHistory {
                at: Slot::new(5),
                value: TokenAmount::new(harvest1_rho),
            };
                consts::TOKENS_PER_SLOT_HISTORY_LEN],
        };
        let mint2 = Pubkey::new_unique();
        let harvest2_rho = 1_000;
        let harvest2 = Harvest {
            mint: mint2,
            vault: Default::default(),
            tokens_per_slot: [TokensPerSlotHistory {
                at: Slot::new(5),
                value: TokenAmount::new(harvest2_rho),
            };
                consts::TOKENS_PER_SLOT_HISTORY_LEN],
        };

        let farm_harvests: BTreeMap<_, _> =
            vec![(mint1, &harvest1), (mint2, &harvest2)]
                .into_iter()
                .collect();
        let mut farmer_harvests: BTreeMap<_, _> = vec![
            (mint1, TokenAmount::new(10)),
            (mint2, TokenAmount::default()),
        ]
        .into_iter()
        .collect();

        let current_slot = 10;
        let snapshot_started_at = 5;
        let calculate_next_harvest_from = 5;
        let total_staked = 10;
        let farmer_staked = 5;
        set_clock(Slot { slot: current_slot });
        update_eligible_harvest_in_open_window(
            &farm_harvests,
            &Snapshot {
                staked: TokenAmount {
                    amount: total_staked,
                },
                started_at: Slot {
                    slot: snapshot_started_at,
                },
            },
            &mut farmer_harvests,
            Slot {
                slot: calculate_next_harvest_from,
            },
            TokenAmount::new(farmer_staked),
        )?;
        assert_eq!(
            farmer_harvests,
            vec![
                (
                    mint1,
                    TokenAmount::new(
                        10 + (current_slot + 1 - calculate_next_harvest_from)
                            * harvest1_rho
                            * farmer_staked
                            / total_staked
                    )
                ),
                (
                    mint2,
                    TokenAmount::new(
                        (current_slot + 1 - calculate_next_harvest_from)
                            * harvest2_rho
                            * farmer_staked
                            / total_staked
                    )
                ),
            ]
            .into_iter()
            .collect()
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn it_updates_harvest_in_open_window_when_user_share_is_zero() -> Result<()>
    {
        let mint1 = Pubkey::new_unique();
        let harvest1 = Harvest {
            mint: mint1,
            vault: Default::default(),
            tokens_per_slot: [TokensPerSlotHistory {
                at: Slot::new(5),
                value: TokenAmount::new(10),
            };
                consts::TOKENS_PER_SLOT_HISTORY_LEN],
        };

        let farm_harvests: BTreeMap<_, _> =
            vec![(mint1, &harvest1)].into_iter().collect();
        let mut farmer_harvests: BTreeMap<_, _> =
            vec![(mint1, TokenAmount::default())].into_iter().collect();

        let farmer_staked = 0;
        set_clock(Slot { slot: 10 });
        update_eligible_harvest_in_open_window(
            &farm_harvests,
            &Snapshot {
                staked: TokenAmount { amount: 10 },
                started_at: Slot { slot: 5 },
            },
            &mut farmer_harvests,
            Slot { slot: 5 },
            TokenAmount::new(farmer_staked),
        )?;
        assert_eq!(
            farmer_harvests,
            vec![(mint1, TokenAmount::default())].into_iter().collect()
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn it_updates_harvest_in_open_window_when_tps_is_zero() -> Result<()> {
        let mint1 = Pubkey::new_unique();
        let tps = 0;
        let harvest1 = Harvest {
            mint: mint1,
            vault: Default::default(),
            tokens_per_slot: [TokensPerSlotHistory {
                at: Slot::new(5),
                value: TokenAmount::new(tps),
            };
                consts::TOKENS_PER_SLOT_HISTORY_LEN],
        };

        let farm_harvests: BTreeMap<_, _> =
            vec![(mint1, &harvest1)].into_iter().collect();
        let mut farmer_harvests: BTreeMap<_, _> =
            vec![(mint1, TokenAmount::default())].into_iter().collect();

        set_clock(Slot { slot: 10 });
        update_eligible_harvest_in_open_window(
            &farm_harvests,
            &Snapshot {
                staked: TokenAmount { amount: 10 },
                started_at: Slot { slot: 5 },
            },
            &mut farmer_harvests,
            Slot { slot: 5 },
            TokenAmount::new(5),
        )?;
        assert_eq!(
            farmer_harvests,
            vec![(mint1, TokenAmount::default())].into_iter().collect()
        );

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
    #[serial]
    fn it_updates_harvest_in_snapshot_history_when_tps_constant() -> Result<()>
    {
        // The purpose of this test is to make sure the method
        // update_eligible_harvest_in_past_snapshots
        // correctly calculates the available harvest in the past snapshots when
        // the tps is constant over the timespan. Note: The last
        // snapshot in the snapshots_to_harvest corresponds to the open window
        // and will therefore not be considered by the method for the purpose of
        // eligible harvest.

        let mint1 = Pubkey::new_unique();

        let harvest1 = Harvest {
            mint: mint1,
            vault: Default::default(),
            tokens_per_slot: [TokensPerSlotHistory {
                at: Slot::new(50),
                value: TokenAmount::new(100),
            };
                consts::TOKENS_PER_SLOT_HISTORY_LEN],
        };

        let mint2 = Pubkey::new_unique();
        let harvest2 = Harvest {
            mint: mint2,
            vault: Default::default(),
            tokens_per_slot: [TokensPerSlotHistory {
                at: Slot::new(0),
                value: TokenAmount::new(1_000),
            };
                consts::TOKENS_PER_SLOT_HISTORY_LEN],
        };

        let farm_harvests: BTreeMap<_, _> =
            vec![(mint1, &harvest1), (mint2, &harvest2)]
                .into_iter()
                .collect();

        let mut farmer_harvests: BTreeMap<_, _> = vec![
            (mint1, TokenAmount::new(100)),
            (mint2, TokenAmount::default()),
        ]
        .into_iter()
        .collect();

        let current_slot = 85;
        let calculate_next_harvest_from = 50;
        let farmer_staked = 5_000;

        let snapshots_to_harvest = [
            Snapshot {
                staked: TokenAmount { amount: 10_000 },
                started_at: Slot { slot: 60 },
            },
            Snapshot {
                staked: TokenAmount { amount: 15_000 },
                started_at: Slot { slot: 70 },
            },
            Snapshot {
                staked: TokenAmount { amount: 20_000 },
                started_at: Slot { slot: 80 },
            },
        ]
        .iter()
        .rev();

        set_clock(Slot { slot: current_slot });
        update_eligible_harvest_in_past_snapshots(
            &farm_harvests,
            snapshots_to_harvest,
            &mut farmer_harvests,
            Slot {
                slot: calculate_next_harvest_from,
            },
            TokenAmount {
                amount: farmer_staked,
            },
        )?;

        assert_eq!(
            farmer_harvests,
            vec![
                (
                    mint1,
                    TokenAmount {
                        // 100 represents what has been already harvested prior
                        // to the function call
                        // slot= 60    70
                        amount: 100 + 500 + 333
                    }
                ),
                (
                    mint2,
                    TokenAmount {
                        // Nothing has been harvested prior to the function call
                        // slot=   60      70
                        amount: 5_000 + 3_333
                    }
                ),
            ]
            .into_iter()
            .collect()
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn it_updates_harvest_in_snapshot_history_when_calc_next_harvest_from_eq_open_window_slot(
    ) -> Result<()> {
        let mint = Pubkey::new_unique();

        let harvest = Harvest {
            mint: mint,
            vault: Default::default(),
            tokens_per_slot: [TokensPerSlotHistory {
                at: Slot::new(50),
                value: TokenAmount::new(100),
            };
                consts::TOKENS_PER_SLOT_HISTORY_LEN],
        };

        let farm_harvests: BTreeMap<_, _> =
            vec![(mint, &harvest)].into_iter().collect();

        let mut farmer_harvests: BTreeMap<_, _> =
            vec![(mint, TokenAmount::default())].into_iter().collect();

        let current_slot = 85;

        let farmer_staked = 5_000;

        let snapshots_to_harvest = [
            Snapshot {
                staked: TokenAmount { amount: 10_000 },
                started_at: Slot { slot: 60 },
            },
            Snapshot {
                staked: TokenAmount { amount: 15_000 },
                started_at: Slot { slot: 70 },
            },
            Snapshot {
                staked: TokenAmount { amount: 20_000 },
                started_at: Slot { slot: 80 },
            },
        ]
        .iter()
        .rev();

        let calculate_next_harvest_from = 80;
        set_clock(Slot { slot: current_slot });
        update_eligible_harvest_in_past_snapshots(
            &farm_harvests,
            snapshots_to_harvest,
            &mut farmer_harvests,
            Slot {
                slot: calculate_next_harvest_from,
            },
            TokenAmount {
                amount: farmer_staked,
            },
        )?;

        assert_eq!(
            farmer_harvests,
            vec![(mint, TokenAmount { amount: 0 }),]
                .into_iter()
                .collect()
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn it_updates_harvest_in_snapshot_history_when_calc_next_harvest_from_eq_snapshot_slot(
    ) -> Result<()> {
        let mint = Pubkey::new_unique();

        let harvest = Harvest {
            mint: mint,
            vault: Default::default(),
            tokens_per_slot: [TokensPerSlotHistory {
                at: Slot::new(50),
                value: TokenAmount::new(100),
            };
                consts::TOKENS_PER_SLOT_HISTORY_LEN],
        };

        let farm_harvests: BTreeMap<_, _> =
            vec![(mint, &harvest)].into_iter().collect();

        let mut farmer_harvests: BTreeMap<_, _> =
            vec![(mint, TokenAmount::default())].into_iter().collect();

        let current_slot = 85;

        let farmer_staked = 5_000;

        let snapshots_to_harvest = [
            Snapshot {
                staked: TokenAmount { amount: 10_000 },
                started_at: Slot { slot: 60 },
            },
            Snapshot {
                staked: TokenAmount { amount: 15_000 },
                started_at: Slot { slot: 70 },
            },
            Snapshot {
                staked: TokenAmount { amount: 20_000 },
                started_at: Slot { slot: 80 },
            },
        ]
        .iter()
        .rev();

        let calculate_next_harvest_from = 70;
        set_clock(Slot { slot: current_slot });
        update_eligible_harvest_in_past_snapshots(
            &farm_harvests,
            snapshots_to_harvest,
            &mut farmer_harvests,
            Slot {
                slot: calculate_next_harvest_from,
            },
            TokenAmount {
                amount: farmer_staked,
            },
        )?;

        assert_eq!(
            farmer_harvests,
            vec![(mint, TokenAmount { amount: 333 }),]
                .into_iter()
                .collect()
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn it_updates_harvest_in_snapshot_history_when_tps_variant() -> Result<()> {
        // The purpose of this test is to make sure the method
        // update_eligible_harvest_in_past_snapshots
        // correctly calculates the available harvest in the past snapshots when
        // the tps is variant over the timespan. Note: The last snapshot
        // in the snapshots_to_harvest corresponds to the open window
        // and will therefore not be considered by the method for the purpose of
        // eligible harvest.

        let mint1 = Pubkey::new_unique();

        let harvest1 = Harvest {
            mint: mint1,
            vault: Default::default(),
            tokens_per_slot: generate_tps_history(&mut vec![
                (65, 200),
                (50, 100),
                (35, 50),
                (0, 10),
            ])
            .try_into()
            .unwrap(),
        };

        let mint2 = Pubkey::new_unique();
        let harvest2 = Harvest {
            mint: mint2,
            vault: Default::default(),
            tokens_per_slot: generate_tps_history(&mut vec![
                (65, 2_000),
                (50, 1_000),
                (35, 500),
                (0, 100),
            ])
            .try_into()
            .unwrap(),
        };

        let farm_harvests: BTreeMap<_, _> =
            vec![(mint1, &harvest1), (mint2, &harvest2)]
                .into_iter()
                .collect();

        let mut farmer_harvests: BTreeMap<_, _> = vec![
            (mint1, TokenAmount::new(100)),
            (mint2, TokenAmount::default()),
        ]
        .into_iter()
        .collect();

        let current_slot = 90;
        let calculate_next_harvest_from = 60;
        let farmer_staked = 5_000;

        let snapshots_to_harvest = [
            Snapshot {
                staked: TokenAmount { amount: 10_000 },
                started_at: Slot { slot: 60 },
            },
            Snapshot {
                staked: TokenAmount { amount: 15_000 },
                started_at: Slot { slot: 70 },
            },
            Snapshot {
                staked: TokenAmount { amount: 20_000 },
                started_at: Slot { slot: 80 },
            },
        ]
        .iter()
        .rev();

        set_clock(Slot { slot: current_slot });
        update_eligible_harvest_in_past_snapshots(
            &farm_harvests,
            snapshots_to_harvest,
            &mut farmer_harvests,
            Slot {
                slot: calculate_next_harvest_from,
            },
            TokenAmount {
                amount: farmer_staked,
            },
        )?;

        assert_eq!(
            farmer_harvests,
            vec![
                (
                    mint1,
                    TokenAmount {
                        // 100 represents what has been already harvested prior
                        // to the function call slot=
                        // 60    70
                        amount: 100 + 500 + 666 // 1_266
                    }
                ),
                (
                    mint2,
                    TokenAmount {
                        // Nothing has been harvested prior to the function call
                        // slot=   60      70
                        amount: 5_000 + 6_666 // 11_666
                    }
                ),
            ]
            .into_iter()
            .collect()
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn it_correctly_updates_harvest_in_snapshot_history_when_called_twice(
    ) -> Result<()> {
        // The purpose of this test is to make sure the method
        // update_eligible_harvest_in_past_snapshots
        // correctly calculates the available harvest in the past snapshots when
        // the endpoint is called twice in a row in the same current
        // slot. When the enpoint is first called the expected behaviour
        // is to update the eligible harvest in the past snapshots. We then
        // update calculate_next_harvest_from but do not update the
        // snapshot iterator. The method is expected to handle the old invalid
        // snapshots by skipping them in the calculation of the eligible
        // harvest.

        let mint = Pubkey::new_unique();

        let harvest = Harvest {
            mint: mint,
            vault: Default::default(),
            tokens_per_slot: [TokensPerSlotHistory {
                at: Slot::new(5),
                value: TokenAmount::new(100),
            };
                consts::TOKENS_PER_SLOT_HISTORY_LEN],
        };

        let farm_harvests: BTreeMap<_, _> =
            vec![(mint, &harvest)].into_iter().collect();

        let mut farmer_harvests: BTreeMap<_, _> =
            vec![(mint, TokenAmount::default())].into_iter().collect();

        let current_slot = 90;
        let mut calculate_next_harvest_from = 50;
        let farmer_staked = 5_000;

        let snapshots_to_harvest = [
            Snapshot {
                staked: TokenAmount { amount: 10_000 },
                started_at: Slot { slot: 60 },
            },
            Snapshot {
                staked: TokenAmount { amount: 15_000 },
                started_at: Slot { slot: 70 },
            },
            Snapshot {
                staked: TokenAmount { amount: 20_000 },
                started_at: Slot { slot: 80 },
            },
        ]
        .iter()
        .rev();

        set_clock(Slot { slot: current_slot });
        update_eligible_harvest_in_past_snapshots(
            &farm_harvests,
            snapshots_to_harvest.clone(),
            &mut farmer_harvests,
            Slot {
                slot: calculate_next_harvest_from,
            },
            TokenAmount {
                amount: farmer_staked,
            },
        )?;

        calculate_next_harvest_from = 80;

        assert_eq!(
            farmer_harvests,
            vec![(
                mint,
                TokenAmount {
                    // slot= 60    70
                    amount: 500 + 333 // 833
                }
            ),]
            .into_iter()
            .collect()
        );

        // This call expected not to change the eligible harvest.
        update_eligible_harvest_in_past_snapshots(
            &farm_harvests,
            snapshots_to_harvest.clone(),
            &mut farmer_harvests,
            Slot {
                slot: calculate_next_harvest_from,
            },
            TokenAmount {
                amount: farmer_staked,
            },
        )?;

        assert_eq!(
            farmer_harvests,
            vec![(
                mint,
                TokenAmount {
                    // slot= 60    70
                    amount: 500 + 333 // 833
                }
            ),]
            .into_iter()
            .collect()
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn it_skips_available_harvest_in_snapshot_history_if_slot_gt_or_eq_to_current_slot(
    ) -> Result<()> {
        // The purpose of this test is to make sure the method
        // update_eligible_harvest_in_past_snapshots
        // skips the calculation of eligible harvest whenever there are no
        // snapshots valid in the snapshot iterator. This happens if the
        // calculate_next_harvest_from is greater than or equal to the slots of
        // the snapshots in the snapshot iterator.

        let mint = Pubkey::new_unique();

        let harvest = Harvest {
            mint: mint,
            vault: Default::default(),
            tokens_per_slot: [TokensPerSlotHistory {
                at: Slot::new(5),
                value: TokenAmount::new(100),
            };
                consts::TOKENS_PER_SLOT_HISTORY_LEN],
        };

        let farm_harvests: BTreeMap<_, _> =
            vec![(mint, &harvest)].into_iter().collect();

        let mut farmer_harvests: BTreeMap<_, _> =
            vec![(mint, TokenAmount::new(100))].into_iter().collect();

        let current_slot = 90;
        let calculate_next_harvest_from = 80;
        let farmer_staked = 5_000;

        let snapshots_to_harvest = [
            Snapshot {
                staked: TokenAmount { amount: 15_000 },
                started_at: Slot { slot: 70 },
            },
            Snapshot {
                staked: TokenAmount { amount: 20_000 },
                started_at: Slot { slot: 80 },
            },
        ]
        .iter();

        set_clock(Slot { slot: current_slot });
        let result = update_eligible_harvest_in_past_snapshots(
            &farm_harvests,
            snapshots_to_harvest,
            &mut farmer_harvests,
            Slot {
                slot: calculate_next_harvest_from,
            },
            TokenAmount {
                amount: farmer_staked,
            },
        )?;

        assert_eq!(result, ());

        Ok(())
    }

    #[test]
    #[serial]
    fn it_correctly_updates_available_harvest_in_snapshot_history_even_if_invalid_snapshots_in_snashots_iterator(
    ) -> Result<()> {
        // The purpose of this test is to make sure that the method
        // `update_eligible_harvest_in_past_snapshots` correctly skips
        // older invalid snapshots in the snapshot queue as well as the open
        // window. Usually the snapshot iterator will not contain
        // older invalid snapshots as these are filtered before the function
        // call. However we still test as if, to guarantee extra safety.

        let mint = Pubkey::new_unique();

        let harvest = Harvest {
            mint: mint,
            vault: Default::default(),
            tokens_per_slot: [TokensPerSlotHistory {
                at: Slot::new(5),
                value: TokenAmount::new(100),
            };
                consts::TOKENS_PER_SLOT_HISTORY_LEN],
        };

        let farm_harvests: BTreeMap<_, _> =
            vec![(mint, &harvest)].into_iter().collect();

        let mut farmer_harvests: BTreeMap<_, _> =
            vec![(mint, TokenAmount::new(0))].into_iter().collect();

        let current_slot = 75;
        let calculate_next_harvest_from = 60;
        let farmer_staked = 5_000;

        let snapshots_to_harvest = [
            Snapshot {
                // Snapshot 50 is already entirely harvested therefore it should
                // be skipped.
                staked: TokenAmount { amount: 10_000 },
                started_at: Slot { slot: 50 },
            },
            Snapshot {
                // Snapshot 60 is only valid snapshot.
                staked: TokenAmount { amount: 15_000 },
                started_at: Slot { slot: 60 },
            },
            Snapshot {
                // Snapshot 70 is open window therefore it should be skipped.
                staked: TokenAmount { amount: 20_000 },
                started_at: Slot { slot: 70 },
            },
            Snapshot {
                // Snapshot 30 is invalid therefore it should be skipped.
                staked: TokenAmount { amount: 30_000 },
                started_at: Slot { slot: 30 },
            },
            Snapshot {
                // Snapshot 40 is invalid therefore it should be skipped.
                staked: TokenAmount { amount: 40_000 },
                started_at: Slot { slot: 40 },
            },
        ]
        .iter()
        .rev();

        set_clock(Slot { slot: current_slot });
        update_eligible_harvest_in_past_snapshots(
            &farm_harvests,
            snapshots_to_harvest,
            &mut farmer_harvests,
            Slot {
                slot: calculate_next_harvest_from,
            },
            TokenAmount {
                amount: farmer_staked,
            },
        )?;

        assert_eq!(
            farmer_harvests,
            vec![(mint, TokenAmount::new(333)),].into_iter().collect()
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn it_works_with_no_harvests_in_past_snapshot() -> Result<()> {
        set_clock(Slot { slot: 9 });

        let farm_harvests = BTreeMap::default();
        let mut farmer_harvests = BTreeMap::default();

        let snapshots_to_harvest = [
            Snapshot {
                staked: TokenAmount { amount: 10_000 },
                started_at: Slot { slot: 6 },
            },
            Snapshot {
                staked: TokenAmount { amount: 15_000 },
                started_at: Slot { slot: 7 },
            },
            Snapshot {
                staked: TokenAmount { amount: 20_000 },
                started_at: Slot { slot: 8 },
            },
        ]
        .iter();

        let current_slot = 9;
        let calculate_next_harvest_from = 8;
        let farmer_staked = 5_000;

        set_clock(Slot { slot: current_slot });
        update_eligible_harvest_in_past_snapshots(
            &farm_harvests,
            snapshots_to_harvest,
            &mut farmer_harvests,
            Slot {
                slot: calculate_next_harvest_from,
            },
            TokenAmount {
                amount: farmer_staked,
            },
        )?;

        assert_eq!(farmer_harvests, BTreeMap::default());

        Ok(())
    }

    #[test]
    #[serial]
    fn it_updates_harvest_in_snapshot_history_when_user_share_is_zero(
    ) -> Result<()> {
        let mint = Pubkey::new_unique();

        let harvest = Harvest {
            mint: mint,
            vault: Default::default(),
            tokens_per_slot: [TokensPerSlotHistory {
                at: Slot::new(5),
                value: TokenAmount::new(1_000),
            };
                consts::TOKENS_PER_SLOT_HISTORY_LEN],
        };

        let farm_harvests: BTreeMap<_, _> =
            vec![(mint, &harvest)].into_iter().collect();

        let mut farmer_harvests: BTreeMap<_, _> =
            vec![(mint, TokenAmount::new(0))].into_iter().collect();

        let current_slot = 9;
        let calculate_next_harvest_from = 5;
        let farmer_staked = 0;

        let snapshots_to_harvest = [
            Snapshot {
                staked: TokenAmount { amount: 10_000 },
                started_at: Slot { slot: 6 },
            },
            Snapshot {
                staked: TokenAmount { amount: 15_000 },
                started_at: Slot { slot: 7 },
            },
            Snapshot {
                staked: TokenAmount { amount: 20_000 },
                started_at: Slot { slot: 8 },
            },
        ]
        .iter();

        set_clock(Slot { slot: current_slot });
        update_eligible_harvest_in_past_snapshots(
            &farm_harvests,
            snapshots_to_harvest,
            &mut farmer_harvests,
            Slot {
                slot: calculate_next_harvest_from,
            },
            TokenAmount {
                amount: farmer_staked,
            },
        )?;

        assert_eq!(
            farmer_harvests,
            vec![(mint, TokenAmount::new(0)),].into_iter().collect()
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn it_updates_harvest_in_snapshot_history_when_tps_is_zero() -> Result<()> {
        let mint = Pubkey::new_unique();

        let harvest = Harvest {
            mint: mint,
            vault: Default::default(),
            tokens_per_slot: [TokensPerSlotHistory {
                at: Slot::new(0),
                value: TokenAmount::new(0),
            };
                consts::TOKENS_PER_SLOT_HISTORY_LEN],
        };

        let farm_harvests: BTreeMap<_, _> =
            vec![(mint, &harvest)].into_iter().collect();

        let mut farmer_harvests: BTreeMap<_, _> =
            vec![(mint, TokenAmount::new(0))].into_iter().collect();

        let current_slot = 9;
        let calculate_next_harvest_from = 5;
        let farmer_staked = 5_000;

        let snapshots_to_harvest = [
            Snapshot {
                staked: TokenAmount { amount: 10_000 },
                started_at: Slot { slot: 6 },
            },
            Snapshot {
                staked: TokenAmount { amount: 15_000 },
                started_at: Slot { slot: 7 },
            },
            Snapshot {
                staked: TokenAmount { amount: 20_000 },
                started_at: Slot { slot: 8 },
            },
        ]
        .iter();

        set_clock(Slot { slot: current_slot });
        update_eligible_harvest_in_past_snapshots(
            &farm_harvests,
            snapshots_to_harvest,
            &mut farmer_harvests,
            Slot {
                slot: calculate_next_harvest_from,
            },
            TokenAmount {
                amount: farmer_staked,
            },
        )?;

        assert_eq!(
            farmer_harvests,
            vec![(mint, TokenAmount::new(0)),].into_iter().collect()
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn it_updates_eligible_harvest() -> Result<()> {
        let mut farmer = Farmer {
            staked: TokenAmount { amount: 1_000 },
            vested: TokenAmount { amount: 0 },
            vested_at: Slot { slot: 0 },
            calculate_next_harvest_from: Slot { slot: 5 },
            ..Default::default()
        };

        let mint = Pubkey::new_unique();

        let farm = Farm {
            harvests: generate_harvests(&mut vec![(
                mint,
                generate_tps_history(&mut vec![(35, 100), (0, 50)])
                    .try_into()
                    .unwrap(),
            )])
            .try_into()
            .unwrap(),
            snapshots: Snapshots {
                ring_buffer_tip: 5,
                ring_buffer: generate_snapshots(&mut vec![
                    (0, 2_000),
                    (10, 10_000),
                    (20, 10_000),
                    (30, 20_000),
                    (40, 20_000),
                    (50, 20_000),
                ])
                .try_into()
                .unwrap(),
            },
            ..Default::default()
        };

        set_clock(Slot { slot: 55 });
        farmer.update_eligible_harvest(&farm)?;

        assert_eq!(farmer.staked, TokenAmount { amount: 1_000 });
        assert_eq!(farmer.calculate_next_harvest_from, Slot { slot: 56 });
        assert_eq!(farmer.harvests[1].mint, mint);
        assert_eq!(
            farmer.harvests[1].tokens,
            TokenAmount {
                // last reward is 30 because it includes 6 slots
                // slot=  0   10   20   30   40   50->56
                amount: 125 + 50 + 50 + 25 + 50 + 30
            }
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn it_correctly_updates_eligible_harvest_when_called_twice() -> Result<()> {
        // The purpose of this test is to make sure that when the endpoint is
        // called twice in the same given current slot, the second call
        // will not change the eligible harvest

        let mut farmer = Farmer {
            staked: TokenAmount { amount: 1_000 },
            vested: TokenAmount { amount: 0 },
            vested_at: Slot { slot: 0 },
            calculate_next_harvest_from: Slot { slot: 5 },
            ..Default::default()
        };

        let mint = Pubkey::new_unique();

        let farm = Farm {
            harvests: generate_harvests(&mut vec![(
                mint,
                generate_tps_history(&mut vec![(35, 100), (0, 50)])
                    .try_into()
                    .unwrap(),
            )])
            .try_into()
            .unwrap(),
            snapshots: Snapshots {
                ring_buffer_tip: 5,
                ring_buffer: generate_snapshots(&mut vec![
                    (0, 2_000),
                    (10, 10_000),
                    (20, 10_000),
                    (30, 20_000),
                    (40, 20_000),
                    (50, 20_000),
                ])
                .try_into()
                .unwrap(),
            },
            ..Default::default()
        };

        set_clock(Slot { slot: 55 });
        farmer.update_eligible_harvest(&farm)?;

        assert_eq!(farmer.staked, TokenAmount { amount: 1_000 });
        assert_eq!(farmer.calculate_next_harvest_from, Slot { slot: 56 });

        assert_eq!(farmer.harvests[1].mint, mint);
        assert_eq!(
            farmer.harvests[1].tokens,
            TokenAmount {
                // slot=  0   10   20   30   40   50->56
                amount: 125 + 50 + 50 + 25 + 50 + 30
            }
        );

        // Test: Calling the endpoint second time in the same current_slot
        // should have no effect on eligible harvest
        farmer.update_eligible_harvest(&farm)?;

        assert_eq!(farmer.harvests[1].mint, mint);
        assert_eq!(
            farmer.harvests[1].tokens,
            TokenAmount {
                // slot=  0   10   20   30   40   50->56
                amount: 125 + 50 + 50 + 25 + 50 + 30
            }
        );
        Ok(())
    }

    #[test]
    #[serial]
    fn it_correctly_updates_eligible_harvest_even_with_old_invalid_snapshots_iterator(
    ) -> Result<()> {
        // The purpose of this test is to assert that the method
        // update_eligible_harvest correctly calculates eligible harvest
        // even when calculate_next_harvest_from.slot > 0 and that therefore
        // proves that the method can successfully skip through old invalid
        // snapshots in the ring buffer

        let mut farmer = Farmer {
            staked: TokenAmount { amount: 1_000 },
            vested: TokenAmount { amount: 0 },
            vested_at: Slot { slot: 0 },
            calculate_next_harvest_from: Slot { slot: 45 },
            ..Default::default()
        };

        let mint = Pubkey::new_unique();

        let farm = Farm {
            harvests: generate_harvests(&mut vec![(
                mint,
                generate_tps_history(&mut vec![(0, 100)])
                    .try_into()
                    .unwrap(),
            )])
            .try_into()
            .unwrap(),
            snapshots: Snapshots {
                ring_buffer_tip: 5,
                ring_buffer: generate_snapshots(&mut vec![
                    (0, 2_000),   // should ignore
                    (10, 10_000), // should ignore
                    (20, 10_000), // should ignore
                    (30, 20_000), // should ignore
                    (40, 20_000), // should account for
                    (50, 20_000), // should account for
                ])
                .try_into()
                .unwrap(),
            },
            ..Default::default()
        };

        set_clock(Slot { slot: 55 });
        farmer.update_eligible_harvest(&farm)?;

        assert_eq!(farmer.staked, TokenAmount { amount: 1_000 });
        assert_eq!(farmer.calculate_next_harvest_from, Slot { slot: 56 });
        assert_eq!(farmer.harvests[1].mint, mint);
        assert_eq!(
            farmer.harvests[1].tokens,
            TokenAmount {
                //slot= 40   50->56
                amount: 25 + 30
            }
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn it_skips_updates_eligible_harvest_when_all_snapshots_in_buffer_are_empty(
    ) -> Result<()> {
        let mut farmer = Farmer {
            staked: TokenAmount { amount: 1_000 },
            vested: TokenAmount { amount: 0 },
            vested_at: Slot { slot: 0 },
            calculate_next_harvest_from: Slot { slot: 0 },
            ..Default::default()
        };

        let mint = Pubkey::new_unique();

        let farm = Farm {
            harvests: generate_harvests(&mut vec![(
                mint,
                generate_tps_history(&mut vec![(0, 100)])
                    .try_into()
                    .unwrap(),
            )])
            .try_into()
            .unwrap(),
            snapshots: Snapshots {
                ring_buffer_tip: 0,
                ring_buffer: [Snapshot::default(); consts::SNAPSHOTS_LEN],
            },
            ..Default::default()
        };

        set_clock(Slot { slot: 15 });
        farmer.update_eligible_harvest(&farm)?;

        // Notice that even if there is no increment in the harvest, we still
        // increment calculate_next_harvest_from.
        assert_eq!(farmer.calculate_next_harvest_from, Slot { slot: 16 });

        assert_eq!(farmer.harvests[1].tokens, TokenAmount { amount: 0 });

        Ok(())
    }

    #[test]
    #[serial]
    fn it_adds_to_vested() -> Result<()> {
        let mut farmer = Farmer::default();

        set_clock(Slot::new(15));
        farmer.add_to_vested(TokenAmount::new(10))?;

        assert_eq!(farmer.vested_at, Slot::new(15));
        assert_eq!(farmer.vested, TokenAmount::new(10));

        set_clock(Slot::new(20));
        farmer.add_to_vested(TokenAmount::new(10))?;

        assert_eq!(farmer.vested_at, Slot::new(20));
        assert_eq!(farmer.vested, TokenAmount::new(20));

        set_clock(Slot::new(25));
        assert!(farmer.add_to_vested(TokenAmount::new(u64::MAX)).is_err());

        Ok(())
    }

    #[test]
    #[serial]
    fn it_unstakes_when_unstake_max_is_gt_vested() -> Result<()> {
        let mut farmer = Farmer::default();
        set_clock(Slot::new(15));

        farmer.staked.amount = 30;
        farmer.add_to_vested(TokenAmount::new(10))?;

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
}
