use ::amm::amm::deposit_liquidity;
use ::amm::prelude::*;
use anchor_spl::token;
pub use anchor_spl::token::spl_token::state::Mint;
use anchortest::{
    builder::*,
    spl::{self, TokenAccountExt},
    stub,
};
use pretty_assertions::assert_eq;
use serial_test::serial;
use solana_sdk::instruction::Instruction;
use solana_sdk::program_pack::Pack;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

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
                spl::token_account(tester.pool_signer.key)
                    .amount(100_000)
                    .mint(mint),
            )
            .owner(token::ID),
        AccountInfoWrapper::new()
            .pack(
                spl::token_account(tester.user.key)
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
            spl::token_account(tester.user.key)
                .mint(reserves[0].mint)
                .amount(2),
        )
        .owner(token::ID);
    tester.vaults_wallets[3] = AccountInfoWrapper::new()
        .pack(
            spl::token_account(tester.user.key)
                .mint(reserves[1].mint)
                .amount(1),
        )
        .owner(token::ID);

    let error = tester
        .deposit_liquidity(max_amount_tokens, &reserves)
        .unwrap_err()
        .to_string();

    assert!(error.contains("InvalidTokenAmount"));

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
        spl::token_account(tester.user.key)
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
    println!("{}", error);
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
        spl::token_account(Pubkey::new_unique())
            .mint(reserves[0].mint)
            .amount(1_000_000),
    );

    tester.vaults_wallets[3] = AccountInfoWrapper::new().pack(
        spl::token_account(Pubkey::new_unique())
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
fn fails_if_multiple_vault_wallet_pairs_of_the_same_mint() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    tester.vaults_wallets.extend_from_slice(&vec![
        AccountInfoWrapper::new()
            .pack(
                spl::token_account(tester.pool_signer.key)
                    .amount(100_000)
                    .mint(reserves[0].mint),
            )
            .owner(token::ID),
        AccountInfoWrapper::new()
            .pack(
                spl::token_account(tester.user.key)
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
        spl::token_account(tester.user.key)
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
fn fails_if_user_is_not_authority_over_a_wallet() -> Result<()> {
    let (mut tester, reserves) = Tester::new_const_prod(2);
    // make the second wallet be owned by a random pubkey
    tester.vaults_wallets[3] = tester.vaults_wallets[3].clone().pack(
        spl::token_account(Pubkey::new_unique())
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
        spl::token_account(tester.pool_signer.key)
            .amount(100_000)
            .mint(unknown_mint),
    );
    tester.vaults_wallets[3] = tester.vaults_wallets[3].clone().pack(
        spl::token_account(tester.user.key)
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

#[derive(Clone, Debug, PartialEq)]
struct Tester {
    user: AccountInfoWrapper,
    pool: AccountInfoWrapper,
    pool_signer: AccountInfoWrapper,
    lp_mint: AccountInfoWrapper,
    lp_token_wallet: AccountInfoWrapper,
    token_program: AccountInfoWrapper,
    vaults_wallets: Vec<AccountInfoWrapper>,
}

impl Tester {
    fn new_const_prod(dimension: usize) -> (Self, Vec<Reserve>) {
        Self::new(dimension, Curve::ConstProd)
    }

    fn new_stable_curve(
        dimension: usize,
        amplifier: u64,
        invariant: Decimal,
    ) -> (Self, Vec<Reserve>) {
        Self::new(
            dimension,
            Curve::Stable {
                amplifier,
                invariant: invariant.into(),
            },
        )
    }

    fn new(dimension: usize, curve: Curve) -> (Self, Vec<Reserve>) {
        let user = AccountInfoWrapper::new().mutable().signer();
        let pool = AccountInfoWrapper::new().owner(amm::ID).mutable();
        let pool_signer = AccountInfoWrapper::pda(
            amm::ID,
            "pool_signer",
            &[Pool::SIGNER_PDA_PREFIX, pool.key.as_ref()],
        );
        let lp_mint = AccountInfoWrapper::new()
            .mutable()
            .pack(spl::mint(pool_signer.key))
            .owner(token::ID);
        let lp_token_wallet = AccountInfoWrapper::new()
            .mutable()
            .pack(spl::token_account(user.key).mint(lp_mint.key))
            .owner(token::ID);
        let token_program = AccountInfoWrapper::with_key(token::ID).program();
        let mut reserves = [Reserve::default(); consts::MAX_RESERVES];
        let vaults_wallets: Vec<_> = (0..dimension)
            .map(|index| {
                let mint = Pubkey::new_unique();
                let vault = AccountInfoWrapper::new()
                    .pack(spl::token_account(pool_signer.key).mint(mint))
                    .owner(token::ID);

                reserves[index] = Reserve {
                    vault: vault.key,
                    mint: mint,
                    tokens: TokenAmount::new(0),
                };

                let wallet = AccountInfoWrapper::new()
                    .pack(
                        spl::token_account(user.key)
                            .mint(mint)
                            .amount(1_000_000_000),
                    )
                    .owner(token::ID);
                vec![vault, wallet].into_iter()
            })
            .flatten()
            .collect();
        assert_eq!(vaults_wallets.len(), dimension * 2);

        let pool_data = Pool {
            signer: pool_signer.key,
            mint: lp_mint.key,
            dimension: dimension as u64,
            curve,
            reserves,
            ..Default::default()
        };
        let reserves = pool_data.reserves().iter().copied().collect();
        let pool = pool.data(pool_data);

        (
            Self {
                user,
                pool,
                pool_signer,
                lp_mint,
                lp_token_wallet,
                token_program,
                vaults_wallets,
            },
            reserves,
        )
    }
}

impl Tester {
    fn deposit_liquidity(
        &mut self,
        max_amount_tokens: BTreeMap<Pubkey, TokenAmount>,
        reserves: &[Reserve],
    ) -> Result<()> {
        let mut pool = Pool::try_deserialize(&mut self.pool.data.as_slice())?;
        let lp_mint = Mint::unpack(&mut self.lp_mint.data.as_slice())?;
        let DepositResult {
            lp_tokens_to_distribute,
            tokens_to_deposit,
        } = pool
            .deposit_tokens(
                max_amount_tokens.clone(),
                TokenAmount::new(lp_mint.supply),
            )
            // We might provide args which make deposit_tokens fail, but we
            // still want to test that scenario, therefore we must't panic here.
            // The default value therefore becomes irrelevant because the handle
            // function shall never reach any transfer.
            .unwrap_or_default();

        // Generally, the order of the reserves and the transfers does not
        // correspond. However, in our tests, we generate the vaults_wallets
        // vec in such a fashion that it actually does correspond to the order
        // or reserves, therefore we take a shortcut and just order the
        // transfers by reserves.
        let transfers: Vec<_> = reserves
            .iter()
            .zip(self.vaults_wallets.chunks(2))
            .map(|(r, vault_wallet)| {
                (
                    vault_wallet[0].key, // vault
                    vault_wallet[1].key, // wallet
                    tokens_to_deposit
                        .get(&r.mint)
                        .copied()
                        // in case we want to test for mismatch between
                        // input args and reserves, we cannot panic
                        .unwrap_or(TokenAmount::new(0)),
                )
            })
            .collect();
        let state = self.set_syscalls(CpiValidatorState::Deposit {
            user: self.user.key,
            transfers,
            next_cpi: MintLpTokens {
                mint: self.lp_mint.key,
                destination: self.lp_token_wallet.key,
                pool_signer: self.pool_signer.key,
                lp_tokens_to_distribute,
            },
        });

        let mut ctx = self.context_wrapper();
        let mut accounts = ctx.accounts()?;

        let max_amount_tokens: Vec<_> = max_amount_tokens
            .into_iter()
            .map(|(mint, tokens)| DepositMintTokens { mint, tokens })
            .collect();

        deposit_liquidity(ctx.build(&mut accounts), max_amount_tokens)?;
        accounts.exit(&amm::ID)?;

        assert_eq!(*state.lock().unwrap(), CpiValidatorState::Done);

        Ok(())
    }

    fn context_wrapper(&mut self) -> ContextWrapper {
        ContextWrapper::new(amm::ID)
            .acc(&mut self.user)
            .acc(&mut self.pool)
            .acc(&mut self.pool_signer)
            .acc(&mut self.lp_mint)
            .acc(&mut self.lp_token_wallet)
            .acc(&mut self.token_program)
            .remaining_accounts(self.vaults_wallets.iter_mut())
    }

    fn set_syscalls(
        &self,
        state: CpiValidatorState,
    ) -> Arc<Mutex<CpiValidatorState>> {
        let state = Arc::new(Mutex::new(state));
        stub::Syscalls::new(CpiValidator(Arc::clone(&state))).set();
        state
    }
}

struct CpiValidator(Arc<Mutex<CpiValidatorState>>);

#[derive(Debug, PartialEq, Eq)]
enum CpiValidatorState {
    Deposit {
        user: Pubkey,
        transfers: Vec<(Pubkey, Pubkey, TokenAmount)>,
        next_cpi: MintLpTokens,
    },
    MintLpTokens(MintLpTokens),
    Done,
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct MintLpTokens {
    mint: Pubkey,
    destination: Pubkey,
    pool_signer: Pubkey,
    lp_tokens_to_distribute: TokenAmount,
}

impl stub::ValidateCpis for CpiValidator {
    fn validate_next_instruction(
        &mut self,
        ix: &Instruction,
        _accounts: &[AccountInfo],
    ) {
        let mut state = self.0.lock().unwrap();

        match *state {
            CpiValidatorState::Deposit {
                user,
                ref mut transfers,
                ref next_cpi,
            } => {
                // take the first transfer, ie. the one that should correspond
                // to the current instruction, as they are sorted
                let (vault, wallet, tokens) = transfers.remove(0);

                let expected_ix = token::spl_token::instruction::transfer(
                    &token::ID,
                    &wallet,
                    &vault,
                    &user,
                    &[],
                    tokens.amount,
                )
                .unwrap();
                assert_eq!(&expected_ix, ix);

                if transfers.is_empty() {
                    *state = CpiValidatorState::MintLpTokens(next_cpi.clone());
                }
            }
            CpiValidatorState::MintLpTokens(MintLpTokens {
                mint,
                destination,
                pool_signer,
                lp_tokens_to_distribute,
            }) => {
                let expected_ix = token::spl_token::instruction::mint_to(
                    &token::ID,
                    &mint,
                    &destination,
                    &pool_signer,
                    &[],
                    lp_tokens_to_distribute.amount,
                )
                .unwrap();
                assert_eq!(&expected_ix, ix);

                *state = CpiValidatorState::Done;
            }
            CpiValidatorState::Done => {
                panic!("No more instructions expected, got {:#?}", ix);
            }
        }
    }
}
