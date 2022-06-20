use crate::prelude::*;
use anchor_spl::token::{self, Token};

#[derive(Accounts)]
pub struct StopFarming<'info> {
    /// Over the [`Farmer`] account.
    pub authority: Signer<'info>,
    #[account(
        mut,
        seeds = [
            Farmer::ACCOUNT_PREFIX,
            farm.key().as_ref(),
            authority.key().as_ref(),
        ],
        bump,
    )]
    pub farmer: Account<'info, Farmer>,
    /// CHECK: UNSAFE_CODES.md#token
    #[account(mut)]
    pub stake_wallet: AccountInfo<'info>,
    pub farm: AccountLoader<'info, Farm>,
    /// CHECK: UNSAFE_CODES.md#signer
    #[account(
        seeds = [Farm::SIGNER_PDA_PREFIX, farm.key().as_ref()],
        bump,
    )]
    pub farm_signer_pda: AccountInfo<'info>,
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

pub fn handle(
    ctx: Context<StopFarming>,
    unstake_max: TokenAmount,
) -> Result<()> {
    let accounts = ctx.accounts;

    if unstake_max.amount == 0 {
        return Err(error!(err::arg(
            "The provided unstake maximum amount needs to be bigger than zero"
        )));
    }

    let farm = accounts.farm.load()?;

    accounts
        .farmer
        .check_vested_period_and_update_harvest(&farm, Slot::current()?)?;

    // removes the amount of tokens to be unstaked from the
    let unstake = accounts.farmer.unstake(unstake_max)?;
    let pda_seeds = &[
        Farm::SIGNER_PDA_PREFIX,
        &accounts.farm.key().to_bytes()[..],
        &[*ctx.bumps.get("farm_signer_pda").unwrap()],
    ];
    token::transfer(
        accounts
            .as_unstake_tokens_context()
            .with_signer(&[&pda_seeds[..]]),
        unstake.amount,
    )?;

    Ok(())
}

impl<'info> StopFarming<'info> {
    fn as_unstake_tokens_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let cpi_accounts = token::Transfer {
            from: self.stake_vault.to_account_info(),
            to: self.stake_wallet.to_account_info(),
            authority: self.farm_signer_pda.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}
