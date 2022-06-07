//! [`Farmer`] uses this endpoint to stake new tokens. The tokens are marked as
//! "vesting", ie. they only start earning harvest from the next window. This is
//! because when a snapshot window starts, the total amount of staked funds is
//! locked for the whole duration of the staking period.
//!
//! This endpoint also updates all eligible harvest up until this point and sets
//! [`Farmer`]'s `harvest_calculated_until` property to the current slot. This
//! avoid a scenario where the newly staked tokens would affect past harvest.

use crate::prelude::*;
use anchor_spl::token::{self, Token};

#[derive(Accounts)]
pub struct StartFarming<'info> {
    /// Authority over the `stake_wallet`, doesn't necessarily have to be
    /// the farmer's authority.
    pub wallet_authority: Signer<'info>,
    /// we don't need to check whether the farmer authority matches
    /// the signer authority, the farmer can only gain in this endpoint
    #[account(
        mut,
        constraint = farmer.farm == farm.key()
            @ err::acc("Farmer is set up for a different farm"),
    )]
    pub farmer: Account<'info, Farmer>,
    /// Stake amount is transferred FROM this wallet.
    ///
    /// CHECK: UNSAFE_CODES.md#token
    #[account(mut)]
    pub stake_wallet: AccountInfo<'info>,
    /// Used to update eligible harvest of the farmer.
    pub farm: AccountLoader<'info, Farm>,
    /// Stake amount is transferred INTO this vault.
    ///
    /// CHECK: UNSAFE_CODES.md#token
    #[account(
        mut,
        seeds = [
            Farm::STAKE_VAULT_PREFIX,
            farm.key().as_ref(),
        ],
        bump,
    )]
    pub stake_vault: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

pub fn handle(ctx: Context<StartFarming>, stake: TokenAmount) -> Result<()> {
    let accounts = ctx.accounts;

    if stake.amount == 0 {
        msg!(
            "The provided stake amount needs \
            to be bigger than zero"
        );
        return Err(error!(AmmError::InvalidArg));
    }

    let farm = accounts.farm.load()?;

    accounts
        .farmer
        .check_vested_period_and_update_harvest(&farm)?;

    // marks the funds as vested, they won't be eligible for harvest until the
    // next snapshot
    accounts.farmer.add_to_vested(stake)?;
    // from farmer's wallet to farm's vault
    token::transfer(accounts.as_stake_tokens_context(), stake.amount)?;

    Ok(())
}

impl<'info> StartFarming<'info> {
    fn as_stake_tokens_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let cpi_accounts = token::Transfer {
            from: self.stake_wallet.to_account_info(),
            to: self.stake_vault.to_account_info(),
            authority: self.wallet_authority.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}
