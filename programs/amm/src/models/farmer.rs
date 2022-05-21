//! User's representation of their position.

use crate::prelude::*;

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
    /// Since there are multiple harvestable mints, this must be an array. The
    /// mint tells us for which token mint does the associated integer,
    /// _available harvest_ amount, apply.
    ///
    /// # Note
    /// Len must match [`consts::MAX_HARVEST_MINTS`].
    pub available_harvest: [MintHash; 10],
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
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert_eq!(farmer.available_harvest.len(), consts::MAX_HARVEST_MINTS);
    }
}
