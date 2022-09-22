//! Initializes new [`Farmer`] account. After this call,
//! farmer can add, stake and remove his tokens from
//! given [`Farm`].
//!
//! The [`Farmer`] account pubkey is a PDA with a seed which guarantees a single
//! [`Farmer`] account per user:
//! ```text
//! [
//!   "farmer",
//!   farmPubkey,
//!   authorityPubkey,
//! ]
//! ```

use crate::prelude::*;

#[derive(Accounts)]
pub struct CreateFarmer<'info> {
    /// Payer can create a farmer on behalf of another user, or payer and
    /// authority can be the same key.
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: the user who wishes to create a new [`Farmer`] account and will
    /// be the authority over withdrawals and claims
    pub authority: AccountInfo<'info>,
    #[account(
        init,
        payer = payer,
        space = Farmer::space(),
        seeds = [
            Farmer::ACCOUNT_PREFIX,
            farm.key().as_ref(),
            authority.key().as_ref(),
        ],
        bump,
    )]
    pub farmer: Account<'info, Farmer>,
    pub farm: AccountLoader<'info, Farm>,
    pub system_program: Program<'info, System>,
}

pub fn handle(ctx: Context<CreateFarmer>) -> Result<()> {
    let accs = ctx.accounts;

    // load ref to farm struct in order to assure we can load [`Farm`]
    let farm = accs.farm.load()?;

    // set both farmer `farm` and `authority` public keys
    accs.farmer.authority = accs.authority.key();
    accs.farmer.farm = accs.farm.key();

    // set empty harvests, note that harvests don't have to be in any particular
    // order
    accs.farmer
        .set_harvests(farm.harvests.map(|h| (h.mint, TokenAmount::new(0))))?;

    Ok(())
}
