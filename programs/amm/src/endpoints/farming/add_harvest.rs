//! Adds new harvestable mint. Farmers who calculate their harvests from here on
//! will have a new pubkey in their [`Farmer.harvests`].
//!
//! This endpoint fails if there are already [`consts::MAX_HARVEST_MINTS`]
//! different harvest mints. First, you must
//! [`crate::endpoints::farming::remove_harvest`] and only then you can add a
//! new harvest.
//!
//! Either call this as an instruction in a transaction in which you deposit
//! tokens into the harvest vault afterwards as another instruction, or set the
//! `tokens_per_second` parameter to 0. Otherwise, if the [`Farm`] is already
//! being used, claiming tokens will fail because the harvest vault is empty.

use crate::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

#[derive(Accounts)]
pub struct AddHarvest<'info> {
    /// The ownership over the farm is checked in the [`handle`] function.
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
    pub harvest_mint: Account<'info, Mint>,
    /// CHECK: UNSAFE_CODES.md#token
    #[account(
        init,
        payer = admin,
        space = TokenAccount::LEN,
        owner = token_program.key(),
        seeds = [
            Harvest::VAULT_PREFIX,
            farm.key().as_ref(),
            harvest_mint.key().as_ref(),
        ],
        bump,
    )]
    pub harvest_vault: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    /// CHECK: UNSAFE_CODES.md#token
    pub rent: AccountInfo<'info>,
}

pub fn handle(
    ctx: Context<AddHarvest>,
    tokens_per_slot: TokenAmount,
) -> Result<()> {
    let farm_signer_bump_seed = *ctx.bumps.get("farm_signer_pda").unwrap();
    let accounts = ctx.accounts;

    let mut farm = accounts.farm.load_mut()?;

    if farm.admin != accounts.admin.key() {
        return Err(error!(AmmError::FarmAdminMismatch));
    }

    farm.add_harvest(
        accounts.harvest_mint.key(),
        accounts.harvest_vault.key(),
        tokens_per_slot,
    )?;

    // init the token account for the harvest vault
    let signer_seed = &[
        Farm::SIGNER_PDA_PREFIX,
        &accounts.farm.key().to_bytes()[..],
        &[farm_signer_bump_seed],
    ];
    token::initialize_account(
        CpiContext::new(
            accounts.token_program.to_account_info(),
            token::InitializeAccount {
                mint: accounts.harvest_mint.to_account_info(),
                authority: accounts.farm_signer_pda.to_account_info(),
                rent: accounts.rent.to_account_info(),
                account: accounts.harvest_vault.to_account_info(),
            },
        )
        .with_signer(&[&signer_seed[..]]),
    )?;

    Ok(())
}
