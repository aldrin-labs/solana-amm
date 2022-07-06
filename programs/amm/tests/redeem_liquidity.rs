#[allow(dead_code)]
mod deposit_redeem;

use ::amm::prelude::*;
use anchor_spl::token;
pub use anchor_spl::token::spl_token::state::{Account, Mint};
use anchortest::{
    builder::*,
    spl::{self, TokenAccountExt},
};
use deposit_redeem::*;
use pretty_assertions::assert_eq;
use serial_test::serial;
use std::collections::BTreeMap;

#[test]
#[serial]
fn redeems_liquidity_from_const_prod_with_two_reserves() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let lp_tokens_to_burn = TokenAmount::new(10);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    tester.redeem_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        lp_tokens_to_burn,
        &reserves,
    )?;

    Ok(())
}

#[test]
#[serial]
fn redeems_liquidity_from_const_prod_with_more_than_two_reserves() -> Result<()>
{
    let (mut tester, reserves) = Tester::new_const_prod(3);
    let lp_tokens_to_burn = TokenAmount::new(10);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    tester.redeem_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        lp_tokens_to_burn,
        &reserves,
    )?;

    Ok(())
}

#[test]
#[serial]
fn redeems_liquidity_from_stable_curve_with_two_reserves() -> Result<()> {
    let (mut tester, reserves) =
        Tester::new_stable_curve(2, 10, Decimal::default());
    let lp_tokens_to_burn = TokenAmount::new(10);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    tester.redeem_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        lp_tokens_to_burn,
        &reserves,
    )?;

    Ok(())
}

#[test]
#[serial]
fn redeems_liquidity_from_stable_curve_with_more_than_two_reserves(
) -> Result<()> {
    let (mut tester, reserves) =
        Tester::new_stable_curve(3, 10, Decimal::default());
    let lp_tokens_to_burn = TokenAmount::new(10);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    tester.redeem_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        lp_tokens_to_burn,
        &reserves,
    )?;

    Ok(())
}

#[test]
#[serial]
fn makes_several_redemptions_from_const_prod_with_two_reserves() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let lp_tokens_to_burn = TokenAmount::new(5);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    tester.redeem_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(5)))
            .collect(),
        lp_tokens_to_burn,
        &reserves,
    )?;

    tester.redeem_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(0)))
            .collect(),
        lp_tokens_to_burn,
        &reserves,
    )?;

    Ok(())
}

#[test]
#[serial]
fn makes_several_redemptions_from_stable_curve_with_two_reserves() -> Result<()>
{
    let (mut tester, reserves) =
        Tester::new_stable_curve(3, 10, Decimal::default());

    let lp_tokens_to_burn = TokenAmount::new(5);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    tester.redeem_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(5)))
            .collect(),
        lp_tokens_to_burn,
        &reserves,
    )?;

    tester.redeem_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(0)))
            .collect(),
        lp_tokens_to_burn,
        &reserves,
    )?;

    Ok(())
}

#[test]
#[serial]
fn fails_if_user_redeems_more_tokens_mints_than_mints_in_pool_reserves(
) -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let lp_tokens_to_burn = TokenAmount::new(10);
    let mint1 = Pubkey::new_unique();
    let mint2 = Pubkey::new_unique();
    let mint3 = Pubkey::new_unique();

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    let min_amount_tokens: BTreeMap<Pubkey, TokenAmount> = [
        (mint1, TokenAmount::new(100)),
        (mint2, TokenAmount::new(100)),
        (mint3, TokenAmount::new(100)),
    ]
    .into_iter()
    .collect();

    let error = tester
        .redeem_liquidity(min_amount_tokens, lp_tokens_to_burn, &reserves)
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidTokenMints"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_user_redeems_less_tokens_than_pool_reserves() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let lp_tokens_to_burn = TokenAmount::new(10);
    let mint1 = Pubkey::new_unique();

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    let min_amount_tokens: BTreeMap<Pubkey, TokenAmount> =
        [(mint1, TokenAmount::new(100))].into_iter().collect();

    let error = tester
        .redeem_liquidity(min_amount_tokens, lp_tokens_to_burn, &reserves)
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidTokenMints"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_user_tokens_do_not_have_correct_mints() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let lp_tokens_to_burn = TokenAmount::new(10);
    let mint1 = Pubkey::new_unique();
    let mint2 = Pubkey::new_unique();

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    let min_amount_tokens: BTreeMap<Pubkey, TokenAmount> = [
        (mint1, TokenAmount::new(100)),
        (mint2, TokenAmount::new(100)),
    ]
    .into_iter()
    .collect();

    let error = tester
        .redeem_liquidity(min_amount_tokens, lp_tokens_to_burn, &reserves)
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidTokenMints"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_same_mint_passed_multiple_times() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let lp_tokens_to_burn = TokenAmount::new(10);
    let mint = reserves[0].mint;

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    let min_amount_tokens: BTreeMap<Pubkey, TokenAmount> =
        [(mint, TokenAmount::new(10)), (mint, TokenAmount::new(10))]
            .into_iter()
            .collect();

    let error = tester
        .redeem_liquidity(min_amount_tokens, lp_tokens_to_burn, &reserves)
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidArg"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_user_does_not_have_enough_lp_tokens() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let lp_tokens_to_burn = TokenAmount::new(10);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    spl::token_account::change_amount(
        &tester.lp_token_wallet.to_account_info(),
        -1,
    )
    .unwrap();

    let error = tester
        .redeem_liquidity(
            reserves
                .iter()
                .map(|r| (r.mint, TokenAmount::new(10)))
                .collect(),
            lp_tokens_to_burn,
            &reserves,
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidLpTokenAmount"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_redemption_amounts_below_min_amount_tokens() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let lp_tokens_to_burn = TokenAmount::new(10);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    let error = tester
        .redeem_liquidity(
            reserves
                .iter()
                .map(|r| (r.mint, TokenAmount::new(1_000)))
                .collect(),
            lp_tokens_to_burn,
            &reserves,
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidArg"));

    Ok(())
}

#[test]
#[serial]
fn pool_is_correctly_updated_const_prod_case() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let pool_og_state =
        Pool::try_deserialize(&mut tester.pool.data.as_slice())?;

    let max_amount_tokens = [(0, 100), (1, 10)]
        .into_iter()
        .map(|ind_ta| (reserves[ind_ta.0].mint, TokenAmount::new(ind_ta.1)))
        .collect::<BTreeMap<Pubkey, TokenAmount>>();

    tester
        .deposit_liquidity(max_amount_tokens, &reserves)
        .unwrap();

    let lp_tokens_to_burn = TokenAmount::new(
        spl::mint::from_acc_info(&tester.lp_mint.to_account_info()).supply / 2,
    );

    let min_amount_tokens = [(0, 50), (1, 5)]
        .into_iter()
        .map(|ind_ta| (reserves[ind_ta.0].mint, TokenAmount::new(ind_ta.1)))
        .collect::<BTreeMap<Pubkey, TokenAmount>>();

    tester.redeem_liquidity(min_amount_tokens, lp_tokens_to_burn, &reserves)?;

    let pool_end_state =
        Pool::try_deserialize(&mut tester.pool.data.as_slice())?;

    assert_eq!(pool_end_state.admin, pool_og_state.admin);
    assert_eq!(pool_end_state.signer, pool_og_state.signer);
    assert_eq!(pool_end_state.mint, pool_og_state.mint);
    assert_eq!(
        pool_end_state.program_toll_wallet,
        pool_og_state.program_toll_wallet
    );
    assert_eq!(pool_end_state.dimension, pool_og_state.dimension);

    let initial_reserves = pool_og_state.reserves();
    let final_reserves = pool_end_state.reserves();

    assert_eq!(initial_reserves[0].mint, final_reserves[0].mint);
    assert_eq!(initial_reserves[0].vault, final_reserves[0].vault);
    assert_eq!(
        initial_reserves[0].tokens.amount + 50,
        final_reserves[0].tokens.amount
    );

    assert_eq!(initial_reserves[1].mint, final_reserves[1].mint);
    assert_eq!(initial_reserves[1].vault, final_reserves[1].vault);
    assert_eq!(
        initial_reserves[1].tokens.amount + 5,
        final_reserves[1].tokens.amount
    );

    assert_eq!(pool_end_state.curve, pool_og_state.curve);
    assert_eq!(pool_end_state.fee, pool_og_state.fee);

    Ok(())
}

#[test]
#[serial]
fn pool_is_correctly_updated_stable_curve_case() -> Result<()> {
    let lp_tokens_to_burn = TokenAmount::new(50);
    let (mut tester, reserves) =
        Tester::new_stable_curve(2, 10, Decimal::default());
    let pool_og_state =
        Pool::try_deserialize(&mut tester.pool.data.as_slice())?;

    tester
        .deposit_liquidity(
            reserves
                .iter()
                .map(|r| (r.mint, TokenAmount::new(100)))
                .collect(),
            &reserves,
        )
        .unwrap();

    tester.redeem_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(50)))
            .collect(),
        lp_tokens_to_burn,
        &reserves,
    )?;

    let pool_end_state =
        Pool::try_deserialize(&mut tester.pool.data.as_slice())?;

    assert_eq!(pool_end_state.admin, pool_og_state.admin);
    assert_eq!(pool_end_state.signer, pool_og_state.signer);
    assert_eq!(pool_end_state.mint, pool_og_state.mint);
    assert_eq!(
        pool_end_state.program_toll_wallet,
        pool_og_state.program_toll_wallet
    );
    assert_eq!(pool_end_state.dimension, pool_og_state.dimension);

    let initial_reserves = pool_og_state.reserves();
    let final_reserves = pool_end_state.reserves();

    assert_eq!(initial_reserves[0].mint, final_reserves[0].mint);
    assert_eq!(initial_reserves[0].vault, final_reserves[0].vault);
    assert_eq!(
        initial_reserves[0].tokens.amount + 50,
        final_reserves[0].tokens.amount
    );

    assert_eq!(initial_reserves[1].mint, final_reserves[1].mint);
    assert_eq!(initial_reserves[1].vault, final_reserves[1].vault);
    assert_eq!(
        initial_reserves[1].tokens.amount + 50,
        final_reserves[1].tokens.amount
    );

    assert_eq!(
        pool_end_state.curve,
        Curve::Stable {
            amplifier: 10,
            invariant: SDecimal::from(100_u64)
        }
    );

    assert_eq!(pool_end_state.fee, pool_og_state.fee);

    Ok(())
}

#[test]
#[serial]
fn fails_if_user_does_not_own_token_account_wallets() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let lp_tokens_to_burn = TokenAmount::new(10);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    tester.vaults_wallets[1] = AccountInfoWrapper::new().pack(
        spl::token_account::new(Pubkey::new_unique())
            .mint(reserves[0].mint)
            .amount(1_000_000),
    );

    tester.vaults_wallets[3] = AccountInfoWrapper::new().pack(
        spl::token_account::new(Pubkey::new_unique())
            .mint(reserves[1].mint)
            .amount(1_000_000),
    );

    let error = tester
        .redeem_liquidity(
            reserves
                .iter()
                .map(|r| (r.mint, TokenAmount::new(10)))
                .collect(),
            lp_tokens_to_burn,
            &reserves,
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("AnchorError"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_multiple_vault_wallet_pairs_of_the_same_mint() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let lp_tokens_to_burn = TokenAmount::new(10);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    tester.vaults_wallets.extend_from_slice(&vec![
        AccountInfoWrapper::new()
            .pack(
                spl::token_account::new(tester.pool_signer.key)
                    .amount(100_000)
                    .mint(reserves[0].mint),
            )
            .owner(token::ID),
        AccountInfoWrapper::new()
            .pack(
                spl::token_account::new(tester.user.key)
                    .amount(100_000)
                    .mint(reserves[0].mint),
            )
            .owner(token::ID),
    ]);

    let error = tester
        .redeem_liquidity(
            reserves
                .iter()
                .map(|r| (r.mint, TokenAmount::new(10)))
                .collect(),
            lp_tokens_to_burn,
            &reserves,
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_multiple_vault_wallet_pair_mint_mismatches() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let lp_tokens_to_burn = TokenAmount::new(10);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    // point second wallet to first vault mint
    tester.vaults_wallets[3] = tester.vaults_wallets[3].clone().pack(
        spl::token_account::new(tester.user.key)
            .amount(100_000)
            .mint(reserves[0].mint),
    );

    let error = tester
        .redeem_liquidity(
            reserves
                .iter()
                .map(|r| (r.mint, TokenAmount::new(10)))
                .collect(),
            lp_tokens_to_burn,
            &reserves,
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_user_is_not_authority_over_a_wallet() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let lp_tokens_to_burn = TokenAmount::new(10);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    // make the second wallet be owned by a random pubkey
    tester.vaults_wallets[3] = tester.vaults_wallets[3].clone().pack(
        spl::token_account::new(Pubkey::new_unique())
            .amount(100_000)
            // this is a correct mint
            .mint(reserves[1].mint),
    );

    let error = tester
        .redeem_liquidity(
            reserves
                .iter()
                .map(|r| (r.mint, TokenAmount::new(10)))
                .collect(),
            lp_tokens_to_burn,
            &reserves,
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_wrong_mint_pair_provided() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let lp_tokens_to_burn = TokenAmount::new(10);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    let unknown_mint = Pubkey::new_unique();
    tester.vaults_wallets[2] = tester.vaults_wallets[2].clone().pack(
        spl::token_account::new(tester.pool_signer.key)
            .amount(100_000)
            .mint(unknown_mint),
    );
    tester.vaults_wallets[3] = tester.vaults_wallets[3].clone().pack(
        spl::token_account::new(tester.user.key)
            .amount(100_000)
            .mint(unknown_mint),
    );

    let error = tester
        .redeem_liquidity(
            reserves
                .iter()
                .map(|r| (r.mint, TokenAmount::new(10)))
                .collect(),
            lp_tokens_to_burn,
            &reserves,
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_at_least_one_wrong_vault_is_provided() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let lp_tokens_to_burn = TokenAmount::new(10);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    tester.vaults_wallets[0] = AccountInfoWrapper::new()
        .pack(
            spl::token_account::new(Pubkey::new_unique())
                .mint(reserves[0].mint)
                .amount(1_000_000),
        )
        .owner(token::ID);

    let error = tester
        .redeem_liquidity(
            reserves
                .iter()
                .map(|r| (r.mint, TokenAmount::new(10)))
                .collect(),
            lp_tokens_to_burn,
            &reserves,
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}
