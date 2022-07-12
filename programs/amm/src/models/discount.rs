//! Discounts are global for all pools, ie. once a [`Discount`] account is
//! created for a user, in any pool they perform a swap they get this discount
//! on the swap fee.
//!
//! The [`Discount`] is always stored in an account with a PDA address for which
//! the user's pubkey is a seed.

use crate::prelude::*;

/// A singleton discount settings model enables us to configure some parts of
/// the discounts feature.
#[account]
pub struct DiscountSettings {
    /// This signer is can call the [`crate::endpoints::put_discount`]
    /// endpoint.
    pub authority: Pubkey,
}

/// A one-to-one account with relationship to a user. We don't store the user's
/// pubkey in the discount account because we don't actually need it, it's
/// sufficient to know the desired user's pubkey to generate a PDA address of an
/// account in which we store this structure.
#[account]
#[derive(Default, PartialEq, Eq, Debug)]
pub struct Discount {
    /// What's the discount the user is eligible for. The discount applies to
    /// the fee, ie. if the amount is `Permillion { permillion: 500_000 }`,
    /// then the user pays only 50% of the fee they would normally.
    ///
    /// The maximum amount of discount is 100%, ie. 1,000,000 permillion.
    pub amount: Permillion,
    /// After this slot, the discount no longer applies.
    pub valid_until: Slot,
}

impl DiscountSettings {
    pub const PDA_SEED: &'static [u8; 17] = b"discount_settings";

    pub fn space() -> usize {
        let discriminant = 8;
        let authority = 32;

        discriminant + authority
    }
}

impl Discount {
    pub const PDA_PREFIX: &'static [u8; 8] = b"discount";

    pub fn space() -> usize {
        let discriminant = 8;
        let amount = 8;
        let valid_until = 8;

        discriminant + amount + valid_until
    }

    pub fn does_apply(&self) -> Result<bool> {
        let time = Slot::current()?;
        Ok(self.amount.permillion > 0 && time <= self.valid_until)
    }
}
