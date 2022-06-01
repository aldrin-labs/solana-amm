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
use std::iter;

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
    // this should also be checked by the PDA seed, that is the harvest vault
    // key will already exist and `init` will fail
    let already_exists = farm
        .harvests
        .iter()
        .any(|h| h.mint == accounts.harvest_mint.key());
    if already_exists {
        return Err(error!(err::acc("Harvest mint already exists")));
    }

    if let Some(harvest) = farm
        .harvests
        .iter_mut()
        .find(|h| h.mint == Pubkey::default())
    {
        harvest.mint = accounts.harvest_mint.key();
        harvest.vault = accounts.harvest_vault.key();
        // we could also just write to zeroth index, because the array should be
        // all zeroes, but let's overwrite the whole array anyway
        harvest.tokens_per_slot = iter::once(TokensPerSlotHistory {
            value: tokens_per_slot,
            at: Slot::current()?,
        })
        .chain(iter::repeat(TokensPerSlotHistory::default()))
        .take(consts::TOKENS_PER_SLOT_HISTORY_LEN)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| {
            msg!("Cannot convert tokens per slot history vector into array");
            AmmError::InvariantViolation
        })?;
    } else {
        return Err(error!(err::acc("Reached maximum harvest mints")));
    }

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
