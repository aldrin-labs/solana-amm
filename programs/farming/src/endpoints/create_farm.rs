//! Initializes new [`Farm`] account. After this call, the admin must
//! add [`Harvest`] for each reward mint they want to distribute using
//! the [`crate::endpoints::add_harvest`] endpoint.

use crate::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

#[derive(Accounts)]
pub struct CreateFarm<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(zero)]
    pub farm: AccountLoader<'info, Farm>,
    /// CHECK: UNSAFE_CODES.md#signer
    #[account(
        mut,
        seeds = [Farm::SIGNER_PDA_PREFIX, farm.key().as_ref()],
        bump,
    )]
    pub farm_signer_pda: AccountInfo<'info>,
    pub stake_mint: Account<'info, Mint>,
    /// CHECK: UNSAFE_CODES.md#token
    #[account(
        init,
        payer = admin,
        space = TokenAccount::LEN,
        owner = token_program.key(),
        seeds = [
            Farm::STAKE_VAULT_PREFIX,
            farm.key().as_ref(),
        ],
        bump,
    )]
    pub stake_vault: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    /// CHECK: UNSAFE_CODES.md#token
    pub rent: AccountInfo<'info>,
}

pub fn handle(ctx: Context<CreateFarm>) -> Result<()> {
    let farm_signer_bump_seed = *ctx.bumps.get("farm_signer_pda").unwrap();
    let accounts = ctx.accounts;

    let mut farm = accounts.farm.load_init()?;

    farm.admin = accounts.admin.key();
    farm.stake_mint = accounts.stake_mint.key();
    farm.stake_vault = accounts.stake_vault.key();

    msg!("Initializing stake vault");

    let signer_seed = &[
        Farm::SIGNER_PDA_PREFIX,
        &accounts.farm.key().to_bytes()[..],
        &[farm_signer_bump_seed],
    ];
    token::initialize_account(
        accounts
            .as_init_stake_vault_context()
            .with_signer(&[&signer_seed[..]]),
    )?;

    Ok(())
}

impl<'info> CreateFarm<'info> {
    pub fn as_init_stake_vault_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, token::InitializeAccount<'info>> {
        let cpi_accounts = token::InitializeAccount {
            mint: self.stake_mint.to_account_info(),
            authority: self.farm_signer_pda.to_account_info(),
            rent: self.rent.to_account_info(),
            account: self.stake_vault.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}
