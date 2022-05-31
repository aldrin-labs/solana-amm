//! User's representation of their position.

use crate::prelude::*;
use std::cell;
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::iter;

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
    /// The slot (inclusive) of since when is the farmer eligible for harvest
    /// again. In other words, upon calculating the available harvest for the
    /// farmer, we set this value to the current slot.
    pub harvest_calculated_until: Slot,
    /// Stores how many tokens is the farmer eligible for **excluding** harvest
    /// since the `harvest_calculated_until` slot.
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
    /// Checks if the vested tokens can be moved to staked tokens. This method
    /// must be called before any other action is taken regarding the farmer's
    /// account.
    pub fn refresh(&mut self, last_snapshot_window_end: Slot) -> Result<()> {
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
    ///
    /// Use use [`cell::Ref`] because farm is too large to fit on stack.
    ///
    /// TODO: unit test this method when [updating eligible harvest in past
    /// windows][issue-18] is finished.
    ///
    /// [issue-18]: https://gitlab.com/crypto_project/defi/amm/-/issues/18
    pub fn update_eligible_harvest(
        &mut self,
        farm: cell::Ref<Farm>,
    ) -> Result<()> {
        let farm_harvests: BTreeMap<_, _> =
            farm.harvests.iter().map(|h| (h.mint, h)).collect();
        let mut farmer_harvests: BTreeMap<_, _> =
            self.harvests.iter().map(|h| (h.mint, h.tokens)).collect();

        sync_harvest_mints(&farm_harvests, &mut farmer_harvests);

        // TODO: https://gitlab.com/crypto_project/defi/amm/-/issues/18

        update_eligible_harvest_in_open_window(
            &farm_harvests,
            &Farm::latest_snapshot(&farm),
            &mut farmer_harvests,
            self.harvest_calculated_until,
            self.staked,
        )?;

        // convert the map back into an array
        self.harvests = farmer_harvests
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
                msg!("Cannot convert available harvests vec into an array");
                AmmError::InvariantViolation
            })?;

        self.harvest_calculated_until.slot = Clock::get()?.slot;

        Ok(())
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
fn update_eligible_harvest_in_open_window(
    farm_harvests: &BTreeMap<Pubkey, &Harvest>,
    open_window: &Snapshot,
    farmer_harvests: &mut BTreeMap<Pubkey, TokenAmount>,
    farmer_harvest_calculated_until: Slot,
    farmer_staked: TokenAmount,
) -> Result<()> {
    let current_slot = Clock::get()?.slot;

    if farmer_harvest_calculated_until.slot >= current_slot {
        return Ok(());
    }

    if farmer_harvest_calculated_until.slot < open_window.started_at.slot {
        msg!("Calculate harvest of past snapshots first");
        // this would only happen if our logic is composed incorrectly
        return Err(error!(AmmError::InvariantViolation));
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
        let slots = current_slot - farmer_harvest_calculated_until.slot;
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
    use crate::prelude::utils::set_clock;
    use serial_test::serial;

    #[test]
    fn it_doesnt_refresh_farmer_if_vesting_amount_is_zero() {
        let mut farmer = Farmer {
            staked: TokenAmount { amount: 0 },
            vested: TokenAmount { amount: 0 },
            vested_at: Slot { slot: 0 },
            ..Default::default()
        };
        let farmer_before_refresh = farmer.clone();

        assert!(farmer.refresh(Slot { slot: 1 }).is_ok());
        assert_eq!(farmer, farmer_before_refresh);
    }

    #[test]
    fn it_doesnt_refresh_farmer_if_vested_at_is_eq_or_gt_than_last_snapshot_window_end(
    ) {
        let mut farmer = Farmer {
            staked: TokenAmount { amount: 0 },
            vested: TokenAmount { amount: 0 },
            vested_at: Slot { slot: 5 },
            ..Default::default()
        };
        let farmer_before_refresh = farmer.clone();

        assert!(farmer.refresh(Slot { slot: 5 }).is_ok());
        assert_eq!(farmer, farmer_before_refresh);

        assert!(farmer.refresh(Slot { slot: 6 }).is_ok());
        assert_eq!(farmer, farmer_before_refresh);
    }

    #[test]
    fn it_errs_refresh_if_vested_overflows_staked() {
        let mut farmer = Farmer {
            staked: TokenAmount { amount: u64::MAX },
            vested: TokenAmount { amount: u64::MAX },
            vested_at: Slot { slot: 0 },
            ..Default::default()
        };

        assert!(farmer.refresh(Slot { slot: 6 }).is_err());
    }

    #[test]
    fn it_refreshes() {
        let mut farmer = Farmer {
            staked: TokenAmount { amount: 10 },
            vested: TokenAmount { amount: 10 },
            vested_at: Slot { slot: 0 },
            ..Default::default()
        };

        assert!(farmer.refresh(Slot { slot: 5 }).is_ok());

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
        let harvest_calculated_until = 5;
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
                slot: harvest_calculated_until,
            },
            TokenAmount::new(farmer_staked),
        )?;
        assert_eq!(
            farmer_harvests,
            vec![
                (
                    mint1,
                    TokenAmount::new(
                        10 + (current_slot - harvest_calculated_until)
                            * harvest1_rho
                            * farmer_staked
                            / total_staked
                    )
                ),
                (
                    mint2,
                    TokenAmount::new(
                        (current_slot - harvest_calculated_until)
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
}
