//! Endpoint to redeem liquidity for a given [`Pool`] (for either constant
//! product or stable curves). Each user is allowed to redeem liquidity.
//! Moreover, this endpoint computes the necessary amount of tokens that
//! need to be redeem, given the amount of LP tokens the user wants to burn,
//! such that the redemption respects the current pool ratio.

use crate::prelude::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Accounts)]
pub struct RedeemLiquidity<'info> {
    /// User to redeem funds
    pub user: Signer<'info>,
    /// Pool to redeem funds from
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    /// CHECK: UNSAFE_CODES.md#signer
    #[account(
        seeds = [Pool::SIGNER_PDA_PREFIX, pool.key().as_ref()],
        bump
    )]
    pub pool_signer: AccountInfo<'info>,
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

/// The redeem_liquidity endpoint logic can be segmented as follows:
/// 1. Get accounts, pool and remaining_accounts (containing pool token
/// vaults and user token wallets for each token reserve in the pool)
/// 2. Deserialize the data in [`AccountInfo`], encapsulated by
/// ctx.remaining_accounts. Throw error, if deserialization is not successful
/// 3. Check that `min_amount_tokens` contains correct mints (i.e., they
/// correspond to mints in the pool reserves)
/// 4. Compute how many tokens should be redeemed, given the current token ratio
/// in the pool and the TokenAmount's in [`min_amount_tokens`]
/// 5. Loop over each (pool_vault, user_wallet) and verify:
///     i. pool_vault and user_wallet contain the same mint
///     ii. obtain the corresponding reserve, whose mint coincides with
/// pool_vault's mint. If this operation is unsuccessful, throw error.
///     iii. transfer correct amount of tokens from the pool token vault to
/// the user token wallet
/// 6. burn the correct amount of lp tokens from the user lp token wallet
/// 7. Update the pool curve invariant value if stable curve
pub fn handle<'info>(
    ctx: Context<'_, '_, '_, 'info, RedeemLiquidity<'info>>,
    lp_tokens_to_burn: TokenAmount,
    min_amount_tokens: Vec<RedeemMintTokens>,
) -> Result<()> {
    let accs = ctx.accounts;

    let pool_signer_bump_seed = *ctx.bumps.get("pool_signer").unwrap();

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
        return Err(error!(err::acc(format!(
            "There must be exactly {} unique mints in remaining accs, not {}",
            accs.pool.dimension, unique_mints_in_rem_accounts
        ))));
    }

    if lp_tokens_to_burn.amount > accs.lp_mint.supply {
        return Err(error!(err::arg(
            "The amount of lp tokens to burn cannot \
            surpass current supply."
        )));
    }

    // if user does not have enough lp tokens we return an error
    if lp_tokens_to_burn.amount > accs.lp_token_wallet.amount {
        return Err(error!(AmmError::InvalidLpTokenAmount));
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

    // convert min_amount_tokens to BTreeMap (to facilitate logic)
    let min_amount_tokens = min_amount_tokens
        .into_iter()
        .map(|h| (h.mint, h.tokens))
        .collect::<BTreeMap<Pubkey, TokenAmount>>();

    // check that min_amount_tokens have the correct mint pubkeys
    accs.pool.check_amount_tokens_is_valid(&min_amount_tokens)?;

    // Get amount of lp tokens to be burned and transferred to user lp token
    // wallet and the amount of tokens that user should deposit on the pool.
    //
    // This mutates the state of the pool, removing the amounts returned.
    let tokens_to_redeem = accs.pool.redeem_tokens(
        min_amount_tokens,
        lp_tokens_to_burn,
        TokenAmount::new(accs.lp_mint.supply),
    )?;

    // redeem tokens from pool reserves
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

        // get tokens to remove to the reserve
        let tokens_redeemed =
            tokens_to_redeem.get(&vault.mint).ok_or_else(|| {
                err::acc(format!(
                    "Mint '{}' is not part of this pool",
                    vault.mint
                ))
            })?;

        // make token transfers from pool token vault to user token wallet
        // this fails if the user does not have enough tokens
        let signer_seeds = &[
            Pool::SIGNER_PDA_PREFIX,
            &accs.pool.key().to_bytes()[..],
            &[pool_signer_bump_seed],
        ];
        token::transfer(
            accs.transfer_liquidity_from_pool_to_wallet(user_wallet, vault)
                .with_signer(&[&signer_seeds[..]]),
            tokens_redeemed.amount,
        )?;
    }

    // burn the LP tokens which are being exchanged for the reserve liquidity
    token::burn(
        accs.burn_lp_tokens_from_user_lp_wallet(),
        lp_tokens_to_burn.amount,
    )?;

    // no-op if const prod
    accs.pool.update_curve_invariant()?;

    Ok(())
}

impl<'info> RedeemLiquidity<'info> {
    fn transfer_liquidity_from_pool_to_wallet(
        &self,
        user_wallet: &Account<'info, TokenAccount>,
        pool_vault: &Account<'info, TokenAccount>,
    ) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let cpi_accounts = token::Transfer {
            from: pool_vault.to_account_info(),
            to: user_wallet.to_account_info(),
            authority: self.pool_signer.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }

    fn burn_lp_tokens_from_user_lp_wallet(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, token::Burn<'info>> {
        let cpi_accounts = token::Burn {
            mint: self.lp_mint.to_account_info(),
            from: self.lp_token_wallet.to_account_info(),
            authority: self.user.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}
