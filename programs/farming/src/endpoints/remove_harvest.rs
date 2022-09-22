//! Removes an existing harvest mint.
//!
//! Since we always know in advance how many tokens should be deposited for a
//! farming period, we can assert that once the harvest vault is empty, all
//! users claimed their harvest
//!
//! When a farmer calculates their harvest after this instruction finishes, we
//! remove the harvest mint from the [`Farmer.harvests`] array.
//!
//! The remaining harvest tokens are transferred to admin selected wallet.

use crate::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};

#[derive(Accounts)]
#[instruction(harvest_mint: Pubkey)]
pub struct RemoveHarvest<'info> {
    /// THe ownership over the farm is checked in the [`handle`] function.
    #[account(mut)]
    pub admin: Signer<'info>,
    /// # Important
    /// We must check all constraints in the [`handle`] body because farm needs
    /// to be loaded first.
    #[account(mut)]
    pub farm: AccountLoader<'info, Farm>,
    /// CHECK: UNSAFE_CODES.md#signer
    #[account(
        seeds = [Farm::SIGNER_PDA_PREFIX, farm.key().as_ref()],
        bump,
    )]
    pub farm_signer_pda: AccountInfo<'info>,
    #[account(
        mut,
        // see the module docs
        constraint = harvest_vault.amount == 0
            @ err::acc("Cannot remove harvest which users haven't yet claimed"),
        seeds = [
            Harvest::VAULT_PREFIX,
            farm.key().as_ref(),
            harvest_mint.key().as_ref(),
        ],
        bump,
    )]
    pub harvest_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

pub fn handle(ctx: Context<RemoveHarvest>, harvest_mint: Pubkey) -> Result<()> {
    let farm_signer_bump_seed = *ctx.bumps.get("farm_signer_pda").unwrap();

    let accounts = ctx.accounts;

    let mut farm = accounts.farm.load_mut()?;

    if farm.admin != accounts.admin.key() {
        return Err(error!(FarmingError::FarmAdminMismatch));
    }

    farm.harvests
        .iter_mut()
        .find(|h| h.mint == harvest_mint)
        .map(|h| *h = Harvest::default())
        // shouldn't be reachable because we parse the harvest vault account
        .ok_or_else(|| err::acc("Harvest mint doesn't exist"))?;

    let signer_seed = &[
        Farm::SIGNER_PDA_PREFIX,
        &accounts.farm.key().to_bytes()[..],
        &[farm_signer_bump_seed],
    ];

    msg!("Closing the harvest vault");
    token::close_account(
        CpiContext::new(
            accounts.token_program.to_account_info(),
            token::CloseAccount {
                account: accounts.harvest_vault.to_account_info(),
                destination: accounts.admin.to_account_info(),
                authority: accounts.farm_signer_pda.to_account_info(),
            },
        )
        .with_signer(&[&signer_seed[..]]),
    )?;

    Ok(())
}
