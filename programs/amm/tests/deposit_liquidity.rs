#[allow(dead_code)]
mod deposit_redeem;

use ::amm::prelude::*;
use anchor_spl::token;
pub use anchor_spl::token::spl_token::state::{Account as TokenAccount, Mint};
use anchortest::{
    builder::*,
    spl::{self, MintExt, TokenAccountExt},
};
use deposit_redeem::*;
use pretty_assertions::assert_eq;
use serial_test::serial;
use std::collections::BTreeMap;

#[test]
#[serial]
fn makes_initial_deposit_into_const_prod_with_two_reserves() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10)))
            .collect(),
        &reserves,
    )?;

    Ok(())
}

#[test]
#[serial]
fn makes_initial_deposit_into_const_prod_with_more_than_two_reserves(
) -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(3);

    tester.deposit_liquidity(
        reserves_to_max_amount_tokens(&reserves, 100),
        &reserves,
    )?;
    Ok(())
}

#[test]
#[serial]
fn makes_initial_deposit_into_stable_curve_with_two_reserves() -> Result<()> {
    let (mut tester, reserves) =
        Tester::new_stable_curve(2, 10, Decimal::default());

    tester.deposit_liquidity(
        reserves_to_max_amount_tokens(&reserves, 100),
        &reserves,
    )?;

    let pool_data = Pool::try_deserialize(&mut tester.pool.data.as_slice())?;

    assert_eq!(pool_data.curve.invariant().unwrap(), Decimal::from(200_u64));
    Ok(())
}

#[test]
#[serial]
fn makes_several_deposits_into_const_prod_with_two_reserves() -> Result<()> {
    let (mut tester, reserves) =
        Tester::new_stable_curve(3, 10, Decimal::default());

    tester.deposit_liquidity(
        reserves_to_max_amount_tokens(&reserves, 100),
        &reserves,
    )?;

    let pool_data = Pool::try_deserialize(&mut tester.pool.data.as_slice())?;

    assert_eq!(pool_data.curve.invariant().unwrap(), Decimal::from(300_u64));
    Ok(())
}

#[test]
#[serial]
fn fails_if_user_deposits_more_tokens_than_pool_reserves() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);

    let new_mint = Pubkey::new_unique();

    let mut max_amount_tokens: BTreeMap<Pubkey, TokenAmount> =
        [(0, 100), (1, 10)]
            .iter()
            .map(|ind_ta| (reserves[ind_ta.0].mint, TokenAmount::new(ind_ta.1)))
            .collect();
    max_amount_tokens.insert(new_mint, TokenAmount::new(50));

    let error = tester
        .deposit_liquidity(max_amount_tokens, &reserves)
        .unwrap_err()
        .to_string();
    assert!(error.contains("InvalidTokenMints"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_less_than_two_reserves() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let mint = Pubkey::new_unique();
    tester.vaults_wallets = [
        AccountInfoWrapper::new()
            .pack(
                spl::token_account::new(tester.pool_signer.key)
                    .amount(100_000)
                    .mint(mint),
            )
            .owner(token::ID),
        AccountInfoWrapper::new()
            .pack(
                spl::token_account::new(tester.user.key)
                    .amount(100_000)
                    .mint(mint),
            )
            .owner(token::ID),
    ]
    .into_iter()
    .collect();

    let max_amount_tokens: BTreeMap<Pubkey, TokenAmount> =
        [(mint, TokenAmount::new(100))].into_iter().collect();

    let error = tester
        .deposit_liquidity(max_amount_tokens, &reserves)
        .unwrap_err()
        .to_string();
    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_same_mint_passed_multiple_times() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    let mint = reserves[0].mint;

    let max_amount_tokens: BTreeMap<Pubkey, TokenAmount> =
        [(mint, TokenAmount::new(100)), (mint, TokenAmount::new(10))]
            .into_iter()
            .collect();

    let error = tester
        .deposit_liquidity(max_amount_tokens, &reserves)
        .unwrap_err()
        .to_string();
    assert!(error.contains("InvalidArg"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_user_does_not_have_enough_funds() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);

    let max_amount_tokens = [(0, 100), (1, 10)]
        .into_iter()
        .map(|ind_ta| (reserves[ind_ta.0].mint, TokenAmount::new(ind_ta.1)))
        .collect::<BTreeMap<Pubkey, TokenAmount>>();

    tester.vaults_wallets[1] = AccountInfoWrapper::new()
        .pack(
            spl::token_account::new(tester.user.key)
                .mint(reserves[0].mint)
                .amount(2),
        )
        .owner(token::ID);
    tester.vaults_wallets[3] = AccountInfoWrapper::new()
        .pack(
            spl::token_account::new(tester.user.key)
                .mint(reserves[1].mint)
                .amount(1),
        )
        .owner(token::ID);

    let error = tester
        .deposit_liquidity(max_amount_tokens, &reserves)
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidArg"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_user_tokens_do_not_have_correct_mints() {
    let (mut tester, reserves) = Tester::new_const_prod(2);

    let max_amount_tokens = [(0, 100), (1, 10)]
        .into_iter()
        .map(|ind_ta| (reserves[ind_ta.0].mint, TokenAmount::new(ind_ta.1)))
        .collect::<BTreeMap<Pubkey, TokenAmount>>();

    tester.vaults_wallets[1] = AccountInfoWrapper::new().pack(
        spl::token_account::new(tester.user.key)
            .mint(Pubkey::new_unique())
            .amount(1_000_000),
    );

    let error = tester
        .deposit_liquidity(max_amount_tokens, &reserves)
        .unwrap_err()
        .to_string();
    assert!(error.contains("AnchorError"))
}

#[test]
#[serial]
fn fails_if_incorrect_specified_max_amount_tokens() {
    let (mut tester, reserves) = Tester::new_const_prod(2);

    let max_amount_tokens = [
        (Pubkey::new_unique(), TokenAmount::new(100)),
        (Pubkey::new_unique(), TokenAmount::new(10)),
    ]
    .into_iter()
    .collect::<BTreeMap<Pubkey, TokenAmount>>();

    let error = tester
        .deposit_liquidity(max_amount_tokens, &reserves)
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidTokenMints"));
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
        initial_reserves[0].tokens.amount + 100,
        final_reserves[0].tokens.amount
    );

    assert_eq!(initial_reserves[1].mint, final_reserves[1].mint);
    assert_eq!(initial_reserves[1].vault, final_reserves[1].vault);
    assert_eq!(
        initial_reserves[1].tokens.amount + 10,
        final_reserves[1].tokens.amount
    );

    assert_eq!(pool_end_state.curve, pool_og_state.curve);
    assert_eq!(pool_end_state.fee, pool_og_state.fee);

    Ok(())
}

#[test]
#[serial]
fn pool_is_correctly_updated_stable_curve_case() -> Result<()> {
    let (mut tester, reserves) =
        Tester::new_stable_curve(2, 10, Decimal::default());
    let pool_og_state =
        Pool::try_deserialize(&mut tester.pool.data.as_slice())?;

    tester.deposit_liquidity(
        reserves_to_max_amount_tokens(&reserves, 100),
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
        initial_reserves[0].tokens.amount + 100,
        final_reserves[0].tokens.amount
    );

    assert_eq!(initial_reserves[1].mint, final_reserves[1].mint);
    assert_eq!(initial_reserves[1].vault, final_reserves[1].vault);
    assert_eq!(
        initial_reserves[1].tokens.amount + 100,
        final_reserves[1].tokens.amount
    );

    assert_eq!(
        pool_end_state.curve,
        Curve::Stable {
            amplifier: 10,
            invariant: SDecimal::from(200_u64)
        }
    );
    assert_eq!(pool_end_state.fee, pool_og_state.fee);

    Ok(())
}

#[test]
#[serial]
fn fails_if_user_does_not_own_token_account_wallets() {
    let (mut tester, reserves) = Tester::new_const_prod(2);

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

    let max_amount_tokens = [
        (reserves[0].mint, TokenAmount::new(100)),
        (reserves[1].mint, TokenAmount::new(10)),
    ]
    .into_iter()
    .collect::<BTreeMap<Pubkey, TokenAmount>>();

    let error = tester
        .deposit_liquidity(max_amount_tokens, &reserves)
        .unwrap_err()
        .to_string();
    assert!(error.contains("AnchorError"));
}

#[test]
#[serial]
fn initial_deposit_does_not_affect_subsequent_deposits_stable_curve(
) -> Result<()> {
    // in the previous version of AMM, if the first deposit was very small,
    // then the subsequent deposits couldn't go too large because of overflow

    let (mut tester, reserves) =
        Tester::new_stable_curve(2, 2, Decimal::zero());

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(1)))
            .collect(),
        &reserves,
    )?;

    // make sure the user wallets have enough funds
    tester.vaults_wallets[1] = tester.vaults_wallets[1].clone().pack(
        spl::token_account::new(tester.user.key)
            .amount(500_000_000_000000)
            .mint(reserves[0].mint),
    );

    tester.vaults_wallets[3] = tester.vaults_wallets[3].clone().pack(
        spl::token_account::new(tester.user.key)
            .amount(500_000_000_000000)
            .mint(reserves[1].mint),
    );

    tester.deposit_liquidity(
        reserves
            .iter()
            // second deposit is huge
            .map(|r| (r.mint, TokenAmount::new(500_000_000_000000)))
            .collect(),
        &reserves,
    )?;

    let pool = Pool::try_deserialize(&mut tester.pool.data.as_slice())?;
    for reserve in pool.reserves() {
        assert_eq!(reserve.tokens.amount, 500_000_000_000001);
    }

    Ok(())
}

#[test]
#[serial]
fn fails_if_multiple_vault_wallet_pairs_of_the_same_mint() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
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
        .deposit_liquidity(
            reserves_to_max_amount_tokens(&reserves, 100),
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
    // point second wallet to first vault mint
    tester.vaults_wallets[3] = tester.vaults_wallets[3].clone().pack(
        spl::token_account::new(tester.user.key)
            .amount(100_000)
            .mint(reserves[0].mint),
    );

    let error = tester
        .deposit_liquidity(
            reserves_to_max_amount_tokens(&reserves, 100),
            &reserves,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn initial_deposit_does_not_affect_subsequent_deposits_const_prod() -> Result<()>
{
    // in the previous version of AMM, if the first deposit was very small,
    // then the subsequent deposits couldn't go too large because of overflow

    let (mut tester, reserves) = Tester::new_const_prod(2);

    tester.deposit_liquidity(
        vec![
            (reserves[0].mint, TokenAmount::new(1)),
            // if you divide these two amounts you get 1:10_000
            // 5 orders of magnitude
            (reserves[1].mint, TokenAmount::new(100_000)),
        ]
        .into_iter()
        .collect(),
        &reserves,
    )?;

    // make sure the user wallets have enough funds
    tester.vaults_wallets[1] = tester.vaults_wallets[1].clone().pack(
        spl::token_account::new(tester.user.key)
            .amount(100_000_000_000_000)
            .mint(reserves[0].mint),
    );

    tester.vaults_wallets[3] = tester.vaults_wallets[3].clone().pack(
        spl::token_account::new(tester.user.key)
            .amount(1_000_000_000_000_000_000)
            .mint(reserves[1].mint),
    );

    //       100_000_000_000_000
    // 1_000_000_000_000_000_000
    tester.deposit_liquidity(
        vec![
            (reserves[0].mint, TokenAmount::new(100_000_000_000_000)),
            // 4 orders of magnitude
            (
                reserves[1].mint,
                TokenAmount::new(1_000_000_000_000_000_000),
            ),
        ]
        .into_iter()
        .collect(),
        &reserves,
    )?;

    let pool = Pool::try_deserialize(&mut tester.pool.data.as_slice())?;
    let reserves = pool.reserves();
    // The ratio of 1:100_000 will be preserved, therefore the
    // expected results don't correspond to the max_amount_tokens inputed
    // if you divide these two you get 1:100_000:
    //        10_000_000_000_001
    // 1_000_000_000_000_100_000
    assert_eq!(reserves[0].tokens.amount, 10_000_000_000_001);
    assert_eq!(reserves[1].tokens.amount, 1_000_000_000_000_100_000);
    Ok(())
}

#[test]
#[serial]
fn fails_if_user_is_not_authority_over_a_wallet() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    // make the second wallet be owned by a random pubkey
    tester.vaults_wallets[3] = tester.vaults_wallets[3].clone().pack(
        spl::token_account::new(Pubkey::new_unique())
            .amount(100_000)
            // this is a correct mint
            .mint(reserves[1].mint),
    );

    let error = tester
        .deposit_liquidity(
            reserves_to_max_amount_tokens(&reserves, 100),
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
        .deposit_liquidity(
            reserves_to_max_amount_tokens(&reserves, 100),
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

    tester.vaults_wallets[0] = AccountInfoWrapper::new()
        .pack(
            spl::token_account::new(Pubkey::new_unique())
                .mint(reserves[0].mint)
                .amount(1_000_000),
        )
        .owner(token::ID);

    let error = tester
        .deposit_liquidity(
            reserves
                .iter()
                .map(|r| (r.mint, TokenAmount::new(10)))
                .collect(),
            &reserves,
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_no_lp_tokens_would_be_minted() -> Result<()> {
    // in the previous version of AMM, if the first deposit was very small,
    // then the subsequent deposits couldn't go too large because of overflow

    let (mut tester, reserves) = Tester::new_const_prod(2);

    tester.deposit_liquidity(
        reserves
            .iter()
            .map(|r| (r.mint, TokenAmount::new(10_000)))
            .collect(),
        &reserves,
    )?;

    // make 1 LP token expensive
    let lp_mint =
        spl::mint::from_acc_info(&tester.lp_mint.to_account_info()).supply(10);
    tester.lp_mint = tester.lp_mint.pack(lp_mint);

    let error = tester
        .deposit_liquidity(
            reserves
                .iter()
                // second deposit is tiny
                .map(|r| (r.mint, TokenAmount::new(10)))
                .collect(),
            &reserves,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("InvalidArg"));

    Ok(())
}

// Creates input arg into the [`deposit_liquidity`] endpoint with all maxes
// being the same.
fn reserves_to_max_amount_tokens(
    reserves: &[Reserve],
    amounts: u64,
) -> BTreeMap<Pubkey, TokenAmount> {
    reserves
        .iter()
        .map(|r| (r.mint, TokenAmount::new(amounts)))
        .collect()
}
