//! Initializes new [`Farm`] account. After this call, the admin must
//! add [`Harvest`] for each reward mint they want to distribute using
//! the [`crate::endpoints::farming::add_harvest`] endpoint.

use crate::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

#[derive(Accounts)]
#[instruction(farm_signer_bump_seed: u8)]
pub struct CreateFarm<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(zero)]
    pub farm: AccountLoader<'info, Farm>,
    /// CHECK: UNSAFE_CODES.md#signer
    #[account(
        mut,
        seeds = [Farm::SIGNER_PDA_PREFIX, farm.key().as_ref()],
        bump = farm_signer_bump_seed,
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
    /// CHECK: UNSAFE_CODES.md#token
    #[account(
        init,
        payer = admin,
        space = TokenAccount::LEN,
        owner = token_program.key(),
        seeds = [
            Farm::VESTING_VAULT_PREFIX,
            farm.key().as_ref(),
        ],
        bump,
    )]
    pub vesting_vault: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    /// CHECK: UNSAFE_CODES.md#token
    pub rent: AccountInfo<'info>,
}

pub fn handle(
    ctx: Context<CreateFarm>,
    farm_signer_bump_seed: u8,
) -> Result<()> {
    let accounts = ctx.accounts;

    let mut farm = accounts.farm.load_init()?;

    farm.admin = accounts.admin.key();
    farm.stake_mint = accounts.stake_mint.key();
    farm.stake_vault = accounts.stake_vault.key();
    farm.vesting_vault = accounts.vesting_vault.key();

    msg!("Initializing vaults");

    let signer_seed = &[
        Farm::SIGNER_PDA_PREFIX,
        &accounts.farm.key().to_bytes()[..],
        &[farm_signer_bump_seed],
    ];

    enum Vault {
        Stake,
        Vesting,
    }

    let init_vault = |vault: Vault| {
        token::initialize_account(
            CpiContext::new(
                accounts.token_program.to_account_info(),
                token::InitializeAccount {
                    mint: accounts.stake_mint.to_account_info(),
                    authority: accounts.farm_signer_pda.to_account_info(),
                    rent: accounts.rent.to_account_info(),
                    // it's easier to match this via an enum than to annotate
                    // lifetimes
                    account: if matches!(vault, Vault::Stake) {
                        accounts.stake_vault.to_account_info()
                    } else {
                        accounts.vesting_vault.to_account_info()
                    },
                },
            )
            .with_signer(&[&signer_seed[..]]),
        )
    };

    init_vault(Vault::Stake)?;
    init_vault(Vault::Vesting)?;

    Ok(())
}
