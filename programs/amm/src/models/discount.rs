use crate::prelude::*;

#[account]
pub struct DiscountSettings {
    /// This signer is can call the [`crate::endpoints::put_discount`]
    /// endpoint.
    pub authority: Pubkey,
}

impl DiscountSettings {
    pub const ACCOUNT_SEED: &'static [u8; 17] = b"discount_settings";

    pub fn space() -> usize {
        let discriminant = 8;
        let authority = 32;

        discriminant + authority
    }
}
