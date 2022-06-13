//! If the stake mint is also one of the harvestable mints, such as is the case
//! with RIN staking (for staking RIN you get RIN harvest), then we enable bots
//! to transfer farmer's harvest into their stake total. This endpoint operates
//! this logic accross two farms, the source farm and the target farm.
//! The eligible harvest is transfered from the harvest vault of the source
//! farm to the stake vault of the target farm. In order for this to work,
//! the source farm admin needs to whitelist the target farm via the endpoint
//! [`whitelist_farm_for_compounding`]. For the same logic but operating only
//! in single farm (harvest and stake vault both under the same farm),
//! see endpoint [`compound_same_farm`].

use crate::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};

#[derive(Accounts)]
pub struct CompoundAcrossFarms<'info> {
    /// Farm which emits the harvest.
    pub source_farm: AccountLoader<'info, Farm>,
    /// Farm which receives the harvest.
    #[account(
        constraint = target_farm.key() != source_farm.key()
            @ err::acc("This endpoint cannot be used to compound the same farm"),
    )]
    pub target_farm: AccountLoader<'info, Farm>,
    /// CHECK: UNSAFE_CODES.md#signer
    #[account(
        seeds = [Farm::SIGNER_PDA_PREFIX, source_farm.key().as_ref()],
        bump,
    )]
    pub source_farm_signer_pda: AccountInfo<'info>,
    /// CHECK: UNSAFE_CODES.md#signer
    /// The whitelist is signaled by the existance of the following PDA
    #[account(
        seeds = [
            Farm::WHITELIST_PDA_PREFIX,
            source_farm.key().as_ref(),
            target_farm.key().as_ref()
        ],
        bump,
    )]
    pub whitelist_compounding: Account<'info, WhitelistCompounding>,
    /// Harvested amount is transferred INTO this vault.
    #[account(
        mut,
        seeds = [
            Farm::STAKE_VAULT_PREFIX,
            target_farm.key().as_ref(),
        ],
        bump,
    )]
    pub target_stake_vault: Account<'info, TokenAccount>,
    /// Harvested amount is transferred FROM this vault.
    #[account(
        mut,
        constraint = source_harvest_vault.mint == target_stake_vault.mint
            @ err::acc(
                "Compounding is only possible if stake mint is a harvestable \
                mint of the farm as well"
            ),
        seeds = [
            Harvest::VAULT_PREFIX,
            source_farm.key().as_ref(),
            source_harvest_vault.mint.as_ref(),
        ],
        bump,
    )]
    pub source_harvest_vault: Account<'info, TokenAccount>,
    /// Harvest of this farmer is transferred into target farm's stake vault.
    #[account(
        mut,
        constraint = source_farmer.authority == target_farmer.authority
            @ err::acc("Source and target farmer must be of the same user"),
        constraint = source_farmer.farm == source_farm.key()
            @ err::acc("Source farmer is set up for a different farm"),
    )]
    pub source_farmer: Box<Account<'info, Farmer>>,
    /// Harvested tokens are added to this farmer's vested tokens.
    #[account(
        mut,
        constraint = target_farmer.farm == target_farm.key()
            @ err::acc("Target farmer is set up for a different farm"),
    )]
    pub target_farmer: Box<Account<'info, Farmer>>,
    pub token_program: Program<'info, Token>,
}

pub fn handle(ctx: Context<CompoundAcrossFarms>) -> Result<()> {
    let accounts = ctx.accounts;

    let source_farm = accounts.source_farm.load()?;
    let target_farm = accounts.target_farm.load()?;

    accounts
        .source_farmer
        .check_vested_period_and_update_harvest(&source_farm)?;

    // get all harvestable tokens of the farmer and add them to their vested
    // tokens
    let compound_tokens = accounts
        .source_farmer
        .claim_harvest(target_farm.stake_mint)?;
    accounts.target_farmer.add_to_vested(compound_tokens)?;

    // transfer all those harvestable tokens to the stake vault
    let pda_seeds = &[
        Farm::SIGNER_PDA_PREFIX,
        &accounts.source_farm.key().to_bytes()[..],
        &[*ctx.bumps.get("source_farm_signer_pda").unwrap()],
    ];
    token::transfer(
        accounts
            .as_transfer_from_harvest_vault_to_stake_vault_context()
            .with_signer(&[&pda_seeds[..]]),
        compound_tokens.amount,
    )?;

    Ok(())
}

impl<'info> CompoundAcrossFarms<'info> {
    fn as_transfer_from_harvest_vault_to_stake_vault_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let cpi_accounts = token::Transfer {
            from: self.source_harvest_vault.to_account_info(),
            to: self.target_stake_vault.to_account_info(),
            authority: self.source_farm_signer_pda.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}
