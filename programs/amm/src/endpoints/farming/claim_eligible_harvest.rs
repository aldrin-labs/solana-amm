//! # Additional accounts
//! Pairs of token accounts for each farm harvest where first member of each
//! pair is the harvest vault and second member is the farmer's harvest wallet.
//!
//! ```text
//! [
//!   harvest_vault1,
//!   harvest_wallet1,
//!   harvest_vault2,
//!   harvest_wallet2,
//!   ...
//! ]
//! ```
//!
//! You don't have to provide all harvestable mints. The pairs for mints which
//! you don't provide are still going to be eligible for claiming later.

use crate::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};
use std::{collections::BTreeMap, iter};

#[derive(Accounts)]
pub struct ClaimEligibleHarvest<'info> {
    pub authority: Signer<'info>,
    #[account(
        mut,
        seeds = [
            Farmer::ACCOUNT_PREFIX,
            farmer.farm.as_ref(),
            authority.key().as_ref(),
        ],
        bump,
    )]
    pub farmer: Account<'info, Farmer>,
    /// CHECK: UNSAFE_CODES.md#signer
    #[account(
        seeds = [Farm::SIGNER_PDA_PREFIX, farmer.farm.as_ref()],
        bump,
    )]
    pub farm_signer_pda: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

pub fn handle<'info>(
    ctx: Context<'_, '_, '_, 'info, ClaimEligibleHarvest<'info>>,
) -> Result<()> {
    let accounts = ctx.accounts;

    if ctx.remaining_accounts.is_empty()
        || ctx.remaining_accounts.len() % 2 != 0
    {
        return Err(error!(err::acc("Remaining accounts must come in pairs")));
    }

    // all transfers are authorized by the same PDA
    let pda_seeds = &[
        Farm::SIGNER_PDA_PREFIX,
        accounts.farmer.farm.as_ref(),
        &[*ctx.bumps.get("farm_signer_pda").unwrap()],
    ];

    // collect all available harvests into a map for fast access, in the end
    // we convert the map back into the array
    let mut farmer_harvests: BTreeMap<_, _> = accounts
        .farmer
        .harvests
        .iter()
        .map(|h| (h.mint, h.tokens))
        .collect();
    // for each [vault, wallet] pair (must be same mint) we transfer eligible
    // harvest from vault to wallet
    for accs in ctx.remaining_accounts.chunks(2) {
        // `token::transfer` CPI fails if
        // * vault/wallet not owned by token program
        // * vault authority isn't PDA
        // * mints don't match
        // * not enough funds

        let vault = &accs[0];
        let wallet = &accs[1];

        let data_ref = vault.try_borrow_data()?;
        let mut data: &[u8] = &data_ref;
        let mint = TokenAccount::try_deserialize(&mut data)?.mint;
        // decrement borrow ref before we give the buffer to token program
        drop(data_ref);

        let (expected_vault, _) = Pubkey::find_program_address(
            &[
                Harvest::VAULT_PREFIX,
                accounts.farmer.farm.as_ref(),
                mint.as_ref(),
            ],
            ctx.program_id,
        );

        if expected_vault != vault.key() {
            return Err(error!(err::acc(format!(
                "Harvest vault for mint '{}' expected to be '{}' but got '{}'",
                mint,
                expected_vault,
                vault.key()
            ))));
        }
        if let Some(eligible_harvest) =
            farmer_harvests.get_mut(&mint).filter(|h| h.amount > 0)
        {
            let vault = vault.to_account_info();
            let wallet = wallet.to_account_info();
            token::transfer(
                accounts
                    .as_transfer_eligible_harvest_context(vault, wallet)
                    .with_signer(&[&pda_seeds[..]]),
                eligible_harvest.amount,
            )?;

            // update the map as we will eventually convert it back
            *eligible_harvest = TokenAmount::new(0);
        }
    }

    // the amounts which have been claimed were set to 0, update the array
    //
    // note that not all harvest mints may have been claimed, but the ones which
    // were are set to 0 now
    accounts.farmer.harvests = farmer_harvests
        .into_iter()
        .map(|(mint, tokens)| AvailableHarvest { mint, tokens })
        // pad with uninitialized harvests
        .chain(iter::repeat_with(|| AvailableHarvest {
            mint: Pubkey::default(),
            tokens: TokenAmount::default(),
        }))
        .take(consts::MAX_HARVEST_MINTS)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| {
            msg!("Cannot convert farmer harvest vector into array");
            AmmError::InvariantViolation
        })?;

    Ok(())
}

impl<'info> ClaimEligibleHarvest<'info> {
    fn as_transfer_eligible_harvest_context(
        &self,
        vault: AccountInfo<'info>,
        wallet: AccountInfo<'info>,
    ) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let cpi_accounts = token::Transfer {
            from: vault,
            to: wallet,
            authority: self.farm_signer_pda.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}
