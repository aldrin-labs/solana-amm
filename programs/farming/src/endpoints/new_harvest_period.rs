//! If the [`Harvest`] has no [`HarvestPeriod`], or if the latest one ended
//! already, then we create a new one (optionally in future as a scheduled
//! launch.)
//!
//! The admin can default to current slot by using `starts_at = 0`.
//!
//! Both `starts_at` and `ends_at` are inclusive.

use crate::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};
use std::cmp::Ordering;

#[derive(Accounts)]
#[instruction(harvest_mint: Pubkey)]
pub struct NewHarvestPeriod<'info> {
    /// The ownership over the farm is checked in the [`handle`] function.
    pub admin: Signer<'info>,
    /// # Important
    /// We must check all constraints in the [`handle`] body because farm needs
    /// to be loaded first
    #[account(mut)]
    pub farm: AccountLoader<'info, Farm>,
    /// Admin's wallet which transfers necessary harvest tokens for this
    /// period. Unless this is a scheduled launch overwrite, then we might even
    /// need to transfer some tokens back here.
    #[account(mut)]
    pub harvest_wallet: Account<'info, TokenAccount>,
    /// We move funds from wallet into this account, unless this overwrites
    /// a scheduled launch, then we might need to move some tokens from here.
    #[account(
        mut,
        seeds = [
            Harvest::VAULT_PREFIX,
            farm.key().as_ref(),
            harvest_mint.as_ref(),
        ],
        bump,
    )]
    pub harvest_vault: Account<'info, TokenAccount>,
    /// CHECK: UNSAFE_CODES.md#signer
    #[account(
        seeds = [Farm::SIGNER_PDA_PREFIX, farm.key().as_ref()],
        bump,
    )]
    pub farm_signer_pda: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

pub fn handle(
    ctx: Context<NewHarvestPeriod>,
    harvest_mint: Pubkey,
    starts_at: Slot,
    ends_at: Slot,
    tps: TokenAmount,
) -> Result<()> {
    let accounts = ctx.accounts;
    let mut farm = accounts.farm.load_mut()?;

    if farm.admin != accounts.admin.key() {
        return Err(error!(FarmingError::FarmAdminMismatch));
    }

    let scheduled_launch = farm.new_harvest_period(
        Slot::current()?,
        harvest_mint,
        (starts_at, ends_at),
        tps,
    )?;

    // if we're overwriting a scheduled launch, then there have been tokens
    // deposited already, so we only need to cover the difference
    let new_period_total_tokens =
        total_tokens_emitted_per_period((starts_at, ends_at), tps)?;
    let tokens_deposited_for_scheduled_launch = if let Some(HarvestPeriod {
        starts_at,
        ends_at,
        tps,
    }) = scheduled_launch
    {
        total_tokens_emitted_per_period((starts_at, ends_at), tps)?
    } else {
        TokenAmount::new(0)
    };

    match new_period_total_tokens
        .amount
        .cmp(&tokens_deposited_for_scheduled_launch.amount)
    {
        // both launches require same amount of tokens, don't do anything
        Ordering::Equal => (),
        // new scheduled launch requires less tokens, return some
        Ordering::Less => {
            let pda_seeds = &[
                Farm::SIGNER_PDA_PREFIX,
                &accounts.farm.key().to_bytes()[..],
                &[*ctx.bumps.get("farm_signer_pda").unwrap()],
            ];
            token::transfer(
                accounts
                    .as_return_harvest_tokens_context()
                    .with_signer(&[&pda_seeds[..]]),
                tokens_deposited_for_scheduled_launch.amount
                    - new_period_total_tokens.amount,
            )?;
        }
        // new period requires more tokens, deposit the difference
        Ordering::Greater => {
            let deposit_amount = new_period_total_tokens.amount
                - tokens_deposited_for_scheduled_launch.amount;
            if deposit_amount > accounts.harvest_wallet.amount {
                return Err(error!(err::acc(format!(
                    "Insufficient tokens in harvest wallet, must deposit {}",
                    deposit_amount
                ))));
            }

            token::transfer(
                accounts.as_deposit_harvest_context(),
                deposit_amount,
            )?;
        }
    }

    Ok(())
}

impl<'info> NewHarvestPeriod<'info> {
    fn as_deposit_harvest_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let cpi_accounts = token::Transfer {
            from: self.harvest_wallet.to_account_info(),
            to: self.harvest_vault.to_account_info(),
            authority: self.admin.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }

    fn as_return_harvest_tokens_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let cpi_accounts = token::Transfer {
            from: self.harvest_vault.to_account_info(),
            to: self.harvest_wallet.to_account_info(),
            authority: self.farm_signer_pda.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

fn total_tokens_emitted_per_period(
    period: (Slot, Slot),
    tps: TokenAmount,
) -> Result<TokenAmount> {
    let (starts_at, ends_at) = period;

    let slots = ends_at
        .slot
        .checked_sub(starts_at.slot)
        .ok_or(FarmingError::MathOverflow)?
        .checked_add(1)
        .ok_or(FarmingError::MathOverflow)?;
    let required_tokens = slots
        .checked_mul(tps.amount)
        .ok_or(FarmingError::MathOverflow)?;
    Ok(TokenAmount::new(required_tokens))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_calculates_total_token_required() {
        assert_eq!(
            total_tokens_emitted_per_period(
                (Slot::new(10), Slot::new(15),),
                TokenAmount::new(10)
            )
            .unwrap(),
            TokenAmount::new(6 * 10),
        );

        assert!(total_tokens_emitted_per_period(
            (Slot::new(10), Slot::new(1),),
            TokenAmount::new(10)
        )
        .is_err(),);

        assert!(total_tokens_emitted_per_period(
            (Slot::new(0), Slot::new(u64::MAX),),
            TokenAmount::new(10)
        )
        .is_err(),);

        assert!(total_tokens_emitted_per_period(
            (Slot::new(0), Slot::new(2),),
            TokenAmount::new(u64::MAX)
        )
        .is_err(),);

        assert_eq!(
            total_tokens_emitted_per_period(
                (Slot::new(10), Slot::new(10),),
                TokenAmount::new(100)
            )
            .unwrap(),
            TokenAmount::new(100),
        );
    }
}
