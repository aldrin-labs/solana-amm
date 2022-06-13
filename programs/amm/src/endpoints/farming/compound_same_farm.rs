//! If the stake mint is also one of the harvestable mints, such as is the case
//! with RIN staking (for staking RIN you get RIN harvest), then we enable bots
//! to transfer farmer's harvest into their stake total. This endpoint operates
//! this logic in a single farm, therefore transfering the tokens from the
//! harvest vault to the stake vault. In order for this to work,the farm admin
//! needs to whitelist itself via the endpoint
//! [`whitelist_farm_form_compounding`]. For the same logic accross different
//! farms see endpoint [`compound_across_farms`].

use crate::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};

#[derive(Accounts)]
pub struct CompoundSameFarm<'info> {
    /// Used to update eligible harvest of the farmer.
    pub farm: AccountLoader<'info, Farm>,
    /// CHECK: UNSAFE_CODES.md#signer
    #[account(
        seeds = [Farm::SIGNER_PDA_PREFIX, farm.key().as_ref()],
        bump,
    )]
    pub farm_signer_pda: AccountInfo<'info>,
    /// CHECK: UNSAFE_CODES.md#signer
    /// The whitelist is signaled by the existance of the following PDA
    #[account(
        seeds = [
            Farm::WHITELIST_PDA_PREFIX,
            farm.key().as_ref(),
            farm.key().as_ref()
        ],
        bump,
    )]
    pub whitelist_compounding: Account<'info, WhitelistCompounding>,
    /// Harvested amount is transferred INTO this vault.
    #[account(
        mut,
        seeds = [
            Farm::STAKE_VAULT_PREFIX,
            farm.key().as_ref(),
        ],
        bump,
    )]
    pub stake_vault: Account<'info, TokenAccount>,
    /// Harvested amount is transferred FROM this vault.
    #[account(
        mut,
        constraint = harvest_vault.mint == stake_vault.mint
            @ err::acc(
                "Compounding is only possible if stake mint is a harvestable \
                mint of the farm as well"
            ),
        seeds = [
            Harvest::VAULT_PREFIX,
            farm.key().as_ref(),
            harvest_vault.mint.as_ref(),
        ],
        bump,
    )]
    pub harvest_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = farmer.farm == farm.key()
            @ err::acc("Farmer is set up for a different farm"),
    )]
    pub farmer: Account<'info, Farmer>,
    pub token_program: Program<'info, Token>,
}

pub fn handle(ctx: Context<CompoundSameFarm>) -> Result<()> {
    let accounts = ctx.accounts;
    let farm_info = accounts.farm.to_account_info();

    Pubkey::try_find_program_address(
        &[
            Farm::WHITELIST_PDA_PREFIX,
            farm_info.key().as_ref(),
            farm_info.key().as_ref(),
        ],
        ctx.program_id,
    )
    .ok_or_else(|| err::acc("Farm is not whitelisted"))?;

    let farm = accounts.farm.load()?;

    accounts
        .farmer
        .check_vested_period_and_update_harvest(&farm)?;

    // get all harvestable tokens of the farmer and add them to their vested
    // tokens
    let compound_tokens = accounts.farmer.claim_harvest(farm.stake_mint)?;
    accounts.farmer.add_to_vested(compound_tokens)?;

    // transfer all those harvestable tokens to the stake vault
    let pda_seeds = &[
        Farm::SIGNER_PDA_PREFIX,
        &accounts.farm.key().to_bytes()[..],
        &[*ctx.bumps.get("farm_signer_pda").unwrap()],
    ];
    token::transfer(
        accounts
            .as_transfer_from_harvest_vault_to_stake_vault_context()
            .with_signer(&[&pda_seeds[..]]),
        compound_tokens.amount,
    )?;

    Ok(())
}

impl<'info> CompoundSameFarm<'info> {
    fn as_transfer_from_harvest_vault_to_stake_vault_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let cpi_accounts = token::Transfer {
            from: self.harvest_vault.to_account_info(),
            to: self.stake_vault.to_account_info(),
            authority: self.farm_signer_pda.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}
