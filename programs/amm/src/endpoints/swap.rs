//! Given sell (base) and buy (quote) tokens which belong to the pool (ie. there
//! are two reserves with the relevant mints), we calculate based on the curve
//! and current pool's state how many tokens should the user get in return.
//!
//! The user pays a fee for the swap, which is scaled down by the [`Discount`]
//! associated with this user. A fraction of the swap fee is sent to program
//! owner's wallet in LP tokens.

use crate::misc::print_lp_supply;
use crate::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use std::collections::BTreeMap;

#[derive(Accounts)]
pub struct Swap<'info> {
    /// Authority over the sell wallet.
    pub user: Signer<'info>,
    /// CHECK: The user's discount might not be initialized, and that's fine,
    /// we are conditionally parsing this account and only if it's valid
    /// will we consider the discount.
    #[account(
        seeds = [Discount::PDA_PREFIX, user.key().as_ref()],
        bump,
    )]
    pub discount: AccountInfo<'info>,
    #[account(mut)]
    pub pool: Box<Account<'info, Pool>>,
    /// CHECK: pda signer
    #[account(
        seeds = [Pool::SIGNER_PDA_PREFIX, pool.key().as_ref()],
        bump,
    )]
    pub pool_signer: AccountInfo<'info>,
    /// Tokens to SELL flow FROM this account.
    #[account(
        mut,
        constraint = sell_wallet.mint != buy_wallet.mint
            @ err::acc("Mint to swap from mustn't equal the mint to swap to"),
        constraint = sell_wallet.mint == sell_vault.mint
            @ err::acc("Sell wallet mint must match sell vault mint"),
    )]
    pub sell_wallet: Box<Account<'info, TokenAccount>>,
    /// Tokens to BUY flow INTO this account.
    #[account(
        mut,
        constraint = buy_wallet.mint == buy_vault.mint
            @ err::acc("Buy wallet mint must match buy vault mint"),
    )]
    pub buy_wallet: Box<Account<'info, TokenAccount>>,
    /// Tokens to SELL flow INTO this account.
    #[account(
        mut,
        // either the mint is not any reserve's mint, or the vault doesn't match
        constraint = pool.reserve_vault(sell_vault.mint) == Some(sell_vault.key())
            @ err::acc("Sell vault is not reserve's vault"),
    )]
    pub sell_vault: Box<Account<'info, TokenAccount>>,
    /// Tokens to BUY flow FROM this account.
    #[account(
        mut,
        // either the mint is not any reserve's mint, or the vault doesn't match
        constraint = pool.reserve_vault(buy_vault.mint) == Some(buy_vault.key())
            @ err::acc("Buy vault is not reserve's vault"),
    )]
    pub buy_vault: Box<Account<'info, TokenAccount>>,
    /// We mint LPs into `program_toll_wallet`
    #[account(
        mut,
        constraint = pool.mint == lp_mint.key() @ err::acc("LP mint mismatch"),
        constraint = lp_mint.supply > 0 @ err::acc("No liquidity provided yet"),
    )]
    pub lp_mint: Box<Account<'info, Mint>>,
    /// Part of the fee is the program owner's cut, and is payed in LPs.
    #[account(
        mut,
        constraint = pool.program_toll_wallet == program_toll_wallet.key()
            @ err::acc("Program toll wallet mismatch"),
    )]
    pub program_toll_wallet: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

/// 1. Calculates swap fee and how many tokens should the user get in return for
/// the sell tokens.
///
/// 2. Transfer the sold tokens to the vault
///
/// 3. Transfers the bought tokens to the user
///
/// 4. Mints LP token to program owner's wallet as a toll for the swap
pub fn handle(
    ctx: Context<Swap>,
    sell: TokenAmount,
    min_buy: TokenAmount,
) -> Result<()> {
    let accs = ctx.accounts;

    if sell.amount == 0 {
        return Err(error!(err::arg("Sell amount mustn't be zero")));
    }

    //
    // 1.
    //

    let swap_fee =
        calculate_swap_fee(sell, accs.pool.swap_fee, &accs.discount)?;

    // swap fee is a fraction of the sell amount
    let tokens_to_swap = TokenAmount::new(sell.amount - swap_fee.amount);
    // this also updates the reserves' balances
    let bought = accs.pool.swap(
        accs.sell_vault.mint,
        tokens_to_swap,
        accs.buy_vault.mint,
    )?;

    if min_buy > bought {
        msg!(
            "For {} would receive {}, but requested minimum of {}",
            sell.amount,
            bought.amount,
            min_buy.amount
        );
        return Err(error!(AmmError::SlippageExceeded));
    }

    let pda_seeds = &[
        Pool::SIGNER_PDA_PREFIX,
        &accs.pool.key().to_bytes()[..],
        &[*ctx.bumps.get("pool_signer").unwrap()],
    ];

    //
    // 2.
    //
    token::transfer(accs.as_transfer_sold_tokens_to_vault_ctx(), sell.amount)?;

    //
    // 3.
    //
    token::transfer(
        accs.as_transfer_bought_tokens_to_wallet_ctx()
            .with_signer(&[&pda_seeds[..]]),
        bought.amount,
    )?;

    //
    // 4.
    //
    let toll_in_lp_tokens = calculate_toll_in_lp_tokens(
        &accs.pool,
        swap_fee,
        accs.sell_vault.mint,
        accs.lp_mint.supply.into(),
    )?;
    if let Some(toll_in_lp_tokens) = toll_in_lp_tokens {
        // this will lower the value of the LP token mint by such an amount
        // which equals to the value of the toll
        token::mint_to(
            accs.as_pay_toll_ctx().with_signer(&[&pda_seeds[..]]),
            toll_in_lp_tokens.amount,
        )?;
    }

    print_lp_supply(&mut accs.lp_mint)?;

    Ok(())
}

impl<'info> Swap<'info> {
    fn as_pay_toll_ctx(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, token::MintTo<'info>> {
        let cpi_accounts = token::MintTo {
            authority: self.pool_signer.to_account_info(),
            mint: self.lp_mint.to_account_info(),
            to: self.program_toll_wallet.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }

    fn as_transfer_sold_tokens_to_vault_ctx(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let cpi_accounts = token::Transfer {
            authority: self.user.to_account_info(),
            from: self.sell_wallet.to_account_info(),
            to: self.sell_vault.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }

    fn as_transfer_bought_tokens_to_wallet_ctx(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let cpi_accounts = token::Transfer {
            authority: self.pool_signer.to_account_info(),
            from: self.buy_vault.to_account_info(),
            to: self.buy_wallet.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

// `swap_fee = sell_amount * (swap_fee_share - swap_fee_share * discount)`
pub fn calculate_swap_fee(
    sell: TokenAmount,
    swap_fee_share: Permillion,
    discount: &AccountInfo,
) -> Result<TokenAmount> {
    let is_discount_created = discount.owner == &crate::ID;

    let swap_fee_share: Decimal = swap_fee_share.into();
    let swap_fee_share = if is_discount_created {
        // we've already verified it's the correct discount bcs of the pda
        let discount = Account::<Discount>::try_from(discount)?;
        if !discount.does_apply()? {
            swap_fee_share
        } else {
            let discount: Decimal = discount.amount.into();
            swap_fee_share.try_sub(swap_fee_share.try_mul(discount)?)?
        }
    } else {
        swap_fee_share
    };

    // total swap fee, ie. liquidity providers fee + toll fee
    let swap_fee = TokenAmount::new(
        Decimal::from(sell.amount)
            .try_mul(swap_fee_share)?
            .try_ceil()?,
    );

    Ok(swap_fee)
}

// To find out how many LPs should we mint, we pretend to deposit to the pool.
// We deposit tokens which are in total worth the `swap_fee * toll_share`.
// Returns the LP amount.
pub fn calculate_toll_in_lp_tokens(
    pool: &Pool,
    swap_fee: TokenAmount,
    sell_mint: Pubkey,
    lp_supply: TokenAmount,
) -> Result<Option<TokenAmount>> {
    let toll_in_sell_tokens_divided_by_dimension = TokenAmount::new(
        Decimal::from(swap_fee)
            .try_mul(Decimal::from(consts::PROGRAM_TOLL_SWAP_FEE_SHARE))?
            // Since we will pretend to deposit this, we need to divide it by
            // the number of reserves. The LPs we get from the fake call to
            // [`Pool::deposit_tokens`] below will return LPs as in all the
            // reserves were deposited to in the same ratio.
            .try_div(Decimal::from(pool.dimension))?
            .try_round()?,
    );

    if toll_in_sell_tokens_divided_by_dimension.amount == 0 {
        return Ok(None);
    }

    let max_deposits: BTreeMap<_, _> = pool
        .reserves()
        .iter()
        .map(|r| {
            (
                r.mint,
                if r.mint == sell_mint {
                    // this is going to become the limiting factor
                    toll_in_sell_tokens_divided_by_dimension
                } else {
                    // We don't care about how many tokens of the other reserves
                    // are deposited, the limiting factor is the sell tokens
                    // mint. The [`Pool::deposit_tokens`]
                    // function ensures that ratios
                    // are preserved.
                    TokenAmount::max_value()
                },
            )
        })
        .collect();

    // We make a fake call (by cloning [`Pool`]) to the [`Pool::deposit_tokens`]
    // which would tell us that if we were to deposit tokens worth the toll,
    // we would get this many LPs. We don't actually deposit anything

    let toll_in_lp_tokens = pool
        // IMPORTANT: we don't actually want to deposit these tokens, we are
        // just wondering how many LPs would they amount to if we deposited them
        .clone()
        .deposit_tokens(max_deposits, lp_supply)?
        .lp_tokens_to_distribute;

    Ok(toll_in_lp_tokens)
}
