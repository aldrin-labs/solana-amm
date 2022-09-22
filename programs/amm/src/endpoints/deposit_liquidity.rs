//! Endpoint to deposit liquidity for a given [`Pool`] (for either constant
//! product or stable curves). Each user is allowed to deposit liquidity.
//! Moreover, this endpoint computes the necessary amount of tokens that
//! need to be deposited, in order to respect the current pool ratio, as
//! well as the amount of LP tokens to be minted, accordingly.
//! When a [`Pool`] is created by an admin, the amount of LP tokens to be
//! minted corresponds to the minimum value of tokens deposited.

use crate::misc::print_lp_supply;
use crate::prelude::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Accounts)]
pub struct DepositLiquidity<'info> {
    /// User to deposit funds from
    pub user: Signer<'info>,
    /// Pool to deposit funds
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    /// CHECK: UNSAFE_CODES.md#signer
    #[account(
        seeds = [Pool::SIGNER_PDA_PREFIX, pool.key().as_ref()],
        bump
    )]
    pub pool_signer_pda: AccountInfo<'info>,
    #[account(
        mut,
        constraint = lp_mint.key() == pool.mint.key()
            @ err::acc("LP mint must match pool's mint")
    )]
    pub lp_mint: Account<'info, Mint>,
    #[account(
        mut,
        constraint = lp_token_wallet.mint == pool.mint.key()
            @ err::acc("LP wallet must be of the same mint as pool's mint"),
    )]
    pub lp_token_wallet: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

/// The deposit_liquidity endpoint logic can be segmented as follows:
/// 1. Get accounts, pool and remaining_accounts (containing pool token
/// vaults and user token wallets for each token reserve in the pool)
/// 2. Deserialize the data in [`AccountInfo`], encapsulated by
/// ctx.remaining_accounts. Throw error, if deserialization is not successful
/// 3. Check that max_amount_tokens contains correct mints (i.e., they
/// correspond to mints in the pool reserves) 4. Compute how many tokens
/// should be deposit, given the current token ratio in the pool and the
/// TokenAmount's in max_amount_tokens 5. Loop over each
/// (pool_vault, user_wallet) and verify:
///     i. pool_vault and user_wallet contain the same mint's
///     ii. obtain the corresponding reserve, whose mint coincides with
/// pool_vault's mint. If this operation is unsuccessful, throw error.
///     iii. transfer correct amount of tokens from the user token wallet to
/// the pool token vault
/// 6. mint the correct amount of lp tokens to the
/// user lp token wallet
/// 7. Update the pool curve invariant value
/// (only in the case the curve is stable)
pub fn handle<'info>(
    ctx: Context<'_, '_, '_, 'info, DepositLiquidity<'info>>,
    max_amount_tokens: Vec<TokenLimit>,
) -> Result<()> {
    let accs = ctx.accounts;

    let pool_signer_bump_seed = *ctx.bumps.get("pool_signer_pda").unwrap();
    let token_vaults_wallets: Vec<Account<'_, TokenAccount>> = ctx
        .remaining_accounts
        .iter()
        .map(Account::try_from)
        .collect::<Result<_>>()?;
    // prevents a scenario where the user provides vault-token acc pairs of the
    // same mint multiple times
    let unique_mints_in_rem_accounts = token_vaults_wallets
        .iter()
        .map(|acc| acc.mint)
        .collect::<BTreeSet<_>>()
        .len();
    if unique_mints_in_rem_accounts != accs.pool.dimension as usize {
        return Err(error!(err::acc(
            "Invalid use of API, same mint deposit for different tokens"
        )));
    }

    // the length of token_vaults_wallets should be twice the number of
    // non-trivial reserve tokens in the pool this is due to the fact that
    // we are passing both a token vault (in the pool) and a token wallet
    // (of the user) for each non-trivial reserve in the pool
    let expected_rem_accs_len = 2 * accs.pool.dimension as usize;
    if token_vaults_wallets.len() != expected_rem_accs_len {
        return Err(error!(err::acc(format!(
            "The remaining accs must be of length {}",
            expected_rem_accs_len
        ))));
    }

    // convert max_amount_tokens to BTreeMap (to facilitate logic)
    let max_amount_tokens = max_amount_tokens
        .into_iter()
        .map(|h| (h.mint, h.tokens))
        .collect::<BTreeMap<Pubkey, TokenAmount>>();

    // check that max_amount_tokens have the correct mint pubkeys
    accs.pool.check_amount_tokens_is_valid(&max_amount_tokens)?;

    // Get amount of lp tokens to be minted and transferred to user lp token
    // wallet and the amount of tokens that user should deposit on the pool.
    let DepositResult {
        lp_tokens_to_distribute,
        tokens_to_deposit,
    } = accs.pool.deposit_tokens(
        max_amount_tokens,
        TokenAmount::new(accs.lp_mint.supply),
    )?;
    let lp_tokens_to_distribute = lp_tokens_to_distribute.ok_or_else(|| {
        msg!("Provided liquidity is too small to be represented");
        AmmError::InvalidArg
    })?;

    // deposit tokens from pool reserves
    for vault_wallet in token_vaults_wallets.chunks(2) {
        let vault: &Account<'info, TokenAccount> = &vault_wallet[0];
        let user_wallet: &Account<'info, TokenAccount> = &vault_wallet[1];

        if vault.mint != user_wallet.mint {
            return Err(error!(err::acc(
                "Each vault wallet pair must match in mint"
            )));
        }
        if user_wallet.owner != accs.user.key() {
            return Err(error!(err::acc(
                "User must be authority over all wallets"
            )));
        }
        // invalid if passed vault_wallet pubkey is not in the reserves
        if !accs
            .pool
            .reserves()
            .iter()
            .any(|r| r.vault.eq(&vault.key()))
        {
            return Err(error!(err::acc(
                "At least one of the vaults in remaining account \
                    does not correspond to any vaul in the pool reserves",
            )));
        }

        // get tokens to add to the reserve
        let add_tokens_to_reserve =
            tokens_to_deposit.get(&vault.mint).ok_or_else(|| {
                err::acc(format!(
                    "Mint '{}' is not part of this pool",
                    vault.mint
                ))
            })?;

        // if user does not have enough funds we return an error
        if add_tokens_to_reserve.amount > user_wallet.amount {
            msg!("Not enough funds in user wallet for this deposit");
            return Err(error!(AmmError::InvalidArg));
        }

        // make token transfers from user token wallet to pool token vault
        token::transfer(
            accs.transfer_liquidity_from_wallet_to_pool(user_wallet, vault),
            add_tokens_to_reserve.amount,
        )?;
    }

    // logic to mint lp tokens and transfer it to the user lp token wallet
    // get the pool signer bump and signer seeds
    let signer_seeds = &[
        Pool::SIGNER_PDA_PREFIX,
        &accs.pool.key().to_bytes()[..],
        &[pool_signer_bump_seed],
    ];
    token::mint_to(
        accs.mint_lp_tokens_to_user_lp_wallet()
            .with_signer(&[&signer_seeds[..]]),
        lp_tokens_to_distribute.amount,
    )?;

    accs.pool.update_curve_invariant()?;

    print_lp_supply(&mut accs.lp_mint)?;

    Ok(())
}

impl<'info> DepositLiquidity<'info> {
    fn transfer_liquidity_from_wallet_to_pool(
        &self,
        user_wallet: &Account<'info, TokenAccount>,
        pool_vault: &Account<'info, TokenAccount>,
    ) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let cpi_accounts = token::Transfer {
            from: user_wallet.to_account_info(),
            to: pool_vault.to_account_info(),
            authority: self.user.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }

    fn mint_lp_tokens_to_user_lp_wallet(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, token::MintTo<'info>> {
        let cpi_accounts = token::MintTo {
            mint: self.lp_mint.to_account_info(),
            to: self.lp_token_wallet.to_account_info(),
            authority: self.pool_signer_pda.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}
