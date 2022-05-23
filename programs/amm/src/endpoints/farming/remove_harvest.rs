//! Removes an existing harvest mint. All farmers who haven't claimed their
//! harvest for this mint yet lose the ability to do so from hereon.
//!
//! When a farmer calculates their harvest after this instruction finishes, we
//! remove the harvest mint from the [`Farmer.harvests`] array.
//!
//! The remaining harvest tokens are transferred to admin selected wallet.

use crate::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};

#[derive(Accounts)]
#[instruction(farm_signer_bump_seed: u8, harvest_mint: Pubkey)]
pub struct RemoveHarvest<'info> {
    /// THe ownership over the farm is checked in the [`handle`] function.
    #[account(mut)]
    pub admin: Signer<'info>,
    /// CHECK: UNSAFE_CODES.md#token
    #[account(mut)]
    pub admin_harvest_wallet: AccountInfo<'info>,
    /// # Important
    /// We must check all constraints in the [`handle`] body because farm needs
    /// to be loaded first.
    #[account(mut)]
    pub farm: AccountLoader<'info, Farm>,
    /// CHECK: UNSAFE_CODES.md#signer
    #[account(
        seeds = [Farm::SIGNER_PDA_PREFIX, farm.key().as_ref()],
        bump = farm_signer_bump_seed,
    )]
    pub farm_signer_pda: AccountInfo<'info>,
    #[account(
        mut,
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

pub fn handle(
    ctx: Context<RemoveHarvest>,
    farm_signer_bump_seed: u8,
    harvest_mint: Pubkey,
) -> Result<()> {
    let accounts = ctx.accounts;

    let mut farm = accounts.farm.load_mut()?;

    if farm.admin != accounts.admin.key() {
        return Err(error!(AmmError::FarmAdminMismatch));
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

    msg!("Transferring remaining harvest tokens to admin's wallet");
    token::transfer(
        CpiContext::new(
            accounts.token_program.to_account_info(),
            token::Transfer {
                from: accounts.harvest_vault.to_account_info(),
                to: accounts.admin_harvest_wallet.to_account_info(),
                authority: accounts.farm_signer_pda.to_account_info(),
            },
        )
        .with_signer(&[&signer_seed[..]]),
        accounts.harvest_vault.amount,
    )?;
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
