//! In some cases, such as migrations and so on, we want to airdrop harvest
//! tokens to some users in such a way that they can claim them at their
//! convenience.
//!
//! This endpoint transfers given amount of tokens into a harvest vault and
//! increments the eligible harvest of the farmer by the same amount.

use crate::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};

#[derive(Accounts)]
pub struct Airdrop<'info> {
    /// Authority over the `harvest_wallet`, doesn't necessarily have to be
    /// the farmer's authority.
    pub wallet_authority: Signer<'info>,
    /// we don't need to check whether the farmer authority matches
    /// the signer authority, the farmer can only gain in this endpoint
    #[account(mut)]
    pub farmer: Account<'info, Farmer>,
    /// Airdrop amount is transferred FROM this wallet.
    ///
    /// CHECK: UNSAFE_CODES.md#token
    #[account(mut)]
    pub harvest_wallet: Account<'info, TokenAccount>,
    /// Airdrop amount is transferred INTO this vault.
    ///
    /// CHECK: UNSAFE_CODES.md#token
    #[account(
        mut,
        seeds = [
            Harvest::VAULT_PREFIX,
            farmer.farm.as_ref(),
            harvest_wallet.mint.as_ref(),
        ],
        bump,
    )]
    pub harvest_vault: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

pub fn handle(ctx: Context<Airdrop>, airdrop: TokenAmount) -> Result<()> {
    let accs = ctx.accounts;

    if airdrop.amount == 0 {
        return Err(error!(err::arg(
            "The provided airdrop amount needs to be bigger than zero"
        )));
    }

    msg!("{}", accs.harvest_wallet.mint);
    msg!("{:#?}", accs.farmer.harvests);

    // get the harvest mint and increase the amount of tokens the user is
    // eligible for by the same amount which will be deposited into the vault
    // in the next step
    let harvest = accs
        .farmer
        .get_harvest_mut(accs.harvest_wallet.mint)
        .ok_or_else(|| {
            err::acc("Farmer cannot harvest given harvest wallet's mint")
        })?;
    harvest.amount = harvest
        .amount
        .checked_add(airdrop.amount)
        .ok_or(FarmingError::MathOverflow)?;

    // from authority's wallet to farm's vault
    token::transfer(accs.as_airdrop_context(), airdrop.amount)?;

    Ok(())
}

impl<'info> Airdrop<'info> {
    fn as_airdrop_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let cpi_accounts = token::Transfer {
            from: self.harvest_wallet.to_account_info(),
            to: self.harvest_vault.to_account_info(),
            authority: self.wallet_authority.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}
