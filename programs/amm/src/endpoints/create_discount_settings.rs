//! To give user specific discounts for [`crate::endpoints::swap`], we create
//! the [`Discount`] account in one-to-one relationship with the user.
//!
//! However, the endpoint [`crate::endpoints::put_discount`] must be
//! permissioned to some higher authority than e.g. a pool's admin, because the
//! discounts are global. Therefore, we enable the program's upgrade authority
//! to create a [`DiscountSettings`] account with a pubkey of the authority who
//! can perform that action.

use crate::prelude::*;

/// Only the program authority can delegate authority to set discounts.
#[cfg(not(feature = "dev"))]
#[derive(Accounts)]
pub struct CreateDiscountSettings<'info> {
    #[account(mut)]
    pub program_authority: Signer<'info>,
    #[account(
        constraint = amm.programdata_address()? == Some(amm_metadata.key())
            @ err::acc("AMM program metadata account mismatch"),
    )]
    pub amm: Program<'info, crate::program::Amm>,
    #[account(
        constraint = amm_metadata.upgrade_authority_address ==
            Some(program_authority.key())
            @ err::acc("Signer isn't program's authority"),
    )]
    pub amm_metadata: Account<'info, ProgramData>,
    /// CHECK: authority which can call the [`crate::endpoints::put_discount`]
    /// endpoint.
    pub discount_settings_authority: AccountInfo<'info>,
    #[account(
        init,
        payer = program_authority,
        space = DiscountSettings::space(),
        seeds = [DiscountSettings::PDA_SEED],
        bump,
    )]
    pub discount_settings: Account<'info, DiscountSettings>,
    pub system_program: Program<'info, System>,
}

/// Due to the way the anchor loads programs on localnet (so that we can use any
/// pubkey and don't have to sign the program deploy), the programs on localnet
/// don't have the same structure in terms of having a data account as with
/// normal deployment.
///
/// That's why for localnet we compile the program under a dev feature and
/// remove the checks which are in the production program. The integration tests
/// still use the production endpoint.
#[cfg(feature = "dev")]
#[derive(Accounts)]
pub struct CreateDiscountSettings<'info> {
    #[account(mut)]
    pub discount_settings_authority: Signer<'info>,
    #[account(
        init,
        payer = discount_settings_authority,
        space = DiscountSettings::space(),
        seeds = [DiscountSettings::PDA_SEED],
        bump,
    )]
    pub discount_settings: Account<'info, DiscountSettings>,
    pub system_program: Program<'info, System>,
}

pub fn handle(ctx: Context<CreateDiscountSettings>) -> Result<()> {
    let accounts = ctx.accounts;

    accounts.discount_settings.authority =
        accounts.discount_settings_authority.key();

    Ok(())
}
