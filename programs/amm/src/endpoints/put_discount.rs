//! Either creates a [`Discount`] model for a user - if it doesn't exist yet -
//! or updates an existing one. In the former scenario, the authority must be
//! mutable so that we can transfer rent to the new account.
//!
//! See the [`crate::models::discount`] module for more info.

use crate::prelude::*;
use anchor_lang::system_program;

#[derive(Accounts)]
#[instruction(user: Pubkey)]
pub struct PutDiscount<'info> {
    #[account(
        constraint = authority.key() == discount_settings.authority
            @ err::acc("The authority must be the discount settings authority"),
    )]
    pub authority: Signer<'info>,
    /// CHECK: we create the discount account if it does not exist yet in the
    /// [`handle`] fn
    #[account(
        mut,
        seeds = [Discount::PDA_PREFIX, user.as_ref()],
        bump,
    )]
    pub discount: AccountInfo<'info>,
    #[account(
        seeds = [DiscountSettings::PDA_SEED],
        bump,
    )]
    pub discount_settings: Account<'info, DiscountSettings>,
    pub system_program: Program<'info, System>,
}

pub fn handle(
    ctx: Context<PutDiscount>,
    user: Pubkey,
    discount_amount: Permillion,
    valid_until: Slot,
) -> Result<()> {
    let accs = ctx.accounts;

    if valid_until <= Slot::current()? {
        return Err(error!(err::arg(
            "The slot until which the discount is valid must be in the future"
        )));
    }

    if discount_amount > Permillion::from_percent(100) {
        return Err(error!(err::arg(
            "Maximum discount can be 100%, ie. 1,000,000 permillion"
        )));
    }

    let should_be_created = accs.discount.owner == &system_program::ID;
    if should_be_created {
        // If the discount account is not yet created, we create it first, as
        // typically with PUT APIs. The following logic is basically what
        // `#[account(init)]` does.
        //
        // We don't have to check that the system program owns it, because it's
        // a PDA and therefore can only be created via our program, and
        // specifically this endpoint. If the data are empty then for
        // sure this account does not exist.

        // we must transfer rent from authority
        if !accs.authority.is_writable {
            return Err(error!(err::acc(
                "Authority must be writable \
                because discount account doesn't exist yet"
            )));
        }

        let pda_seeds = &[
            Discount::PDA_PREFIX,
            user.as_ref(),
            &[*ctx.bumps.get("discount").unwrap()],
        ];
        let rent = Rent::get()?.minimum_balance(Discount::space());
        system_program::create_account(
            accs.as_create_discount_account_context()
                .with_signer(&[&pda_seeds[..]]),
            rent,
            Discount::space() as u64,
            ctx.program_id,
        )?;
    }

    // overwrites the data in the discount account
    let discount = Discount {
        valid_until,
        amount: discount_amount,
    };
    let mut discount_data = accs.discount.try_borrow_mut_data()?;
    discount.try_serialize(&mut discount_data.as_mut())?;

    Ok(())
}

impl<'info> PutDiscount<'info> {
    fn as_create_discount_account_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, system_program::CreateAccount<'info>>
    {
        let cpi_accounts = system_program::CreateAccount {
            from: self.authority.to_account_info(),
            to: self.discount.to_account_info(),
        };
        let cpi_program = self.system_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}
