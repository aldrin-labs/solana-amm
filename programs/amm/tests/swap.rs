use ::amm::amm::swap;
use ::amm::endpoints::{calculate_swap_fee, calculate_toll_in_lp_tokens};
use ::amm::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token;
use anchortest::{
    builder::*,
    spl::{self, *},
    stub,
};
use pretty_assertions::assert_eq;
use serial_test::serial;
use solana_sdk::instruction::Instruction;
use std::iter;
use std::sync::{Arc, Mutex};

#[test]
#[serial]
fn swaps_const_prod_two_reserves_no_discount() -> Result<()> {
    let pool_before = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(9),
        reserves: create_two_reserves(
            TokenAmount::new(20_000),
            TokenAmount::new(20_000),
        ),
        ..Default::default()
    };

    let mut test = Tester::no_discount(pool_before.clone());

    let supply_before = test.lp_supply();

    test.swap(
        TokenAmount::new(10_000),
        TokenAmount::new(6_254),
        pool_before.reserves[0].mint,
        pool_before.reserves[1].mint,
    )?;

    let mut pool_after = test.pool_copy();

    // 29_000 = 20_000 + 10_000 * (1 - 0.09)
    assert_eq!(pool_after.reserves[0].tokens.amount, 29_100);
    // 13_746 = 29_000 / K, whereas K = 20_000 * 20_000
    assert_eq!(pool_after.reserves[1].tokens.amount, 13_746);

    // only these 3 values can change
    pool_after.reserves[0].tokens = pool_before.reserves[0].tokens;
    pool_after.reserves[1].tokens = pool_before.reserves[1].tokens;
    pool_after.curve = pool_before.curve;
    assert_eq!(pool_before, pool_after);

    let supply_after = test.lp_supply();

    assert_eq!(supply_before + 51, supply_after);

    Ok(())
}

#[test]
#[serial]
fn swaps_stable_curve_three_reserves_no_discount() -> Result<()> {
    let pool_before = Pool {
        dimension: 3,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(9),
        reserves: create_reserves(&[
            TokenAmount::new(20_000_000_000),
            TokenAmount::new(19_989_000_000),
            TokenAmount::new(20_002_000_000),
        ]),
        curve: Curve::Stable {
            amplifier: 10,
            invariant: Default::default(),
        },
        ..Default::default()
    };

    let mut test = Tester::no_discount(pool_before.clone());

    let supply_before = test.lp_supply();

    test.swap(
        TokenAmount::new(10_000_000),
        TokenAmount::new(9_000_000),
        pool_before.reserves[0].mint,
        pool_before.reserves[1].mint,
    )?;

    let mut pool_after = test.pool_copy();

    assert_eq!(pool_after.reserves[0].tokens.amount, 20_009_100_000);
    assert_eq!(pool_after.reserves[1].tokens.amount, 19_979_900_101);
    assert_eq!(pool_after.reserves[2].tokens.amount, 20_002_000_000);

    // only these 3 values can change
    pool_after.reserves[0].tokens = pool_before.reserves[0].tokens;
    pool_after.reserves[1].tokens = pool_before.reserves[1].tokens;
    pool_after.curve = pool_before.curve;
    assert_eq!(pool_before, pool_after);

    let supply_after = test.lp_supply();

    assert_eq!(supply_before, supply_after);

    Ok(())
}

#[test]
#[serial]
fn swaps_const_prod_two_reserves_discount() -> Result<()> {
    let pool_before = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(9),
        reserves: create_two_reserves(
            TokenAmount::new(20_000),
            TokenAmount::new(20_000),
        ),
        ..Default::default()
    };

    let mut test = Tester::with_discount(
        pool_before.clone(),
        Discount {
            amount: Permillion::from_percent(50),
            valid_until: Slot::new(500),
        },
    );

    let supply_before = test.lp_supply();

    test.swap(
        TokenAmount::new(10_000),
        TokenAmount::new(6_463),
        pool_before.reserves[0].mint,
        pool_before.reserves[1].mint,
    )?;

    let pool = test.pool_copy();

    // reserve x = 20_000 + 9_550, whereas the latest
    // 9_550 = 10_000 * (1 - 9%(1 - 50%))
    assert_eq!(pool.reserves[0].tokens.amount, 29_550);
    // reserve y = 20_000 - 6_463, whereas the latest
    // 6_463 = floor(y0 - (K / x1)) = floor(20_000 - (400_000_000 / 29_550))
    assert_eq!(pool.reserves[1].tokens.amount, 13_537);

    let supply_after = test.lp_supply();

    assert_eq!(supply_before + 25, supply_after);

    Ok(())
}

#[test]
#[serial]
fn ignores_discount_if_not_valid_anymore() -> Result<()> {
    let pool_before = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(9),
        reserves: create_two_reserves(
            TokenAmount::new(20_000),
            TokenAmount::new(20_000),
        ),
        ..Default::default()
    };

    let mut test = Tester::with_discount(
        pool_before.clone(),
        Discount {
            amount: Permillion::from_percent(50),
            valid_until: Slot::new(500),
        },
    )
    .slot(Slot::new(501));

    let supply_before = test.lp_supply();

    test.swap(
        TokenAmount::new(10_000),
        TokenAmount::new(6_254),
        pool_before.reserves[0].mint,
        pool_before.reserves[1].mint,
    )?;

    let pool = test.pool_copy();

    // reserve x = 20_000 + 9_100, whereas the latest
    // 9_100 = 10_000 * (1 - 9%)
    assert_eq!(pool.reserves[0].tokens.amount, 29_100);
    // reserve y = 20_000 - 6_254, whereas the latest
    // 6_254 = floor(y0 - (K / x1)) = floor(20_000 - (400_000_000 / 29_100))
    assert_eq!(pool.reserves[1].tokens.amount, 13_746);

    let supply_after = test.lp_supply();

    assert_eq!(supply_before + 51, supply_after);

    Ok(())
}

#[test]
#[serial]
fn fails_if_sell_amount_is_zero() -> Result<()> {
    let pool = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(9),
        reserves: create_two_reserves(
            TokenAmount::new(20_000),
            TokenAmount::new(20_000),
        ),
        ..Default::default()
    };

    let mut test = Tester::no_discount(pool.clone());

    let error = test
        .swap(
            TokenAmount::new(0),
            TokenAmount::new(9_500),
            pool.reserves[0].mint,
            pool.reserves[1].mint,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("InvalidArg"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_slippage_exceeded() -> Result<()> {
    let pool = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(9),
        reserves: create_two_reserves(
            TokenAmount::new(20_000),
            TokenAmount::new(20_000),
        ),
        ..Default::default()
    };

    let mut test = Tester::no_discount(pool.clone());

    let error = test
        .swap(
            TokenAmount::new(10_000),
            TokenAmount::new(20_000),
            pool.reserves[0].mint,
            pool.reserves[1].mint,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("SlippageExceeded"));

    Ok(())
}

#[test]
#[serial]
fn updates_stable_curve_invariant() -> Result<()> {
    let pool = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(0),
        reserves: create_two_reserves(
            TokenAmount::new(20_000),
            TokenAmount::new(20_000),
        ),
        curve: Curve::Stable {
            amplifier: 2,
            invariant: Default::default(),
        },
        ..Default::default()
    };

    let mut test = Tester::no_discount(pool.clone());

    let pool_before = test.pool_copy();

    test.swap(
        TokenAmount::new(10_000),
        TokenAmount::new(8_000),
        pool.reserves[0].mint,
        pool.reserves[1].mint,
    )?;

    let pool_after = test.pool_copy();

    // Assert that the pool state, specifically the invariant does not change
    assert_eq!(pool_before.curve, pool_after.curve);

    Ok(())
}

#[test]
#[serial]
fn skips_lp_mint_if_fee_would_not_allow_for_it() -> Result<()> {
    let pool = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(1), // tiny fee
        reserves: create_two_reserves(
            TokenAmount::new(10),
            TokenAmount::new(10),
        ),
        ..Default::default()
    };
    let mut test = Tester::no_discount(pool.clone());

    let supply_before = test.lp_supply();

    test.swap(
        TokenAmount::new(2),
        TokenAmount::new(0),
        pool.reserves[0].mint,
        pool.reserves[1].mint,
    )?;

    let supply_after = test.lp_supply();
    assert_eq!(supply_before, supply_after);

    Ok(())
}

#[test]
#[serial]
fn fails_if_pool_signer_mismatches() -> Result<()> {
    let pool = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(9),
        reserves: create_two_reserves(
            TokenAmount::new(20_000),
            TokenAmount::new(20_000),
        ),
        ..Default::default()
    };

    let mut test = Tester::no_discount(pool.clone());
    test.pool_signer = AccountInfoWrapper::new();

    let error = test
        .swap(
            TokenAmount::new(10_000),
            TokenAmount::new(9_500),
            pool.reserves[0].mint,
            pool.reserves[1].mint,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("ConstraintSeeds"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_sell_wallet_mint_matches_buy_wallet_mint() -> Result<()> {
    let pool = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(9),
        reserves: create_two_reserves(
            TokenAmount::new(20_000),
            TokenAmount::new(20_000),
        ),
        ..Default::default()
    };

    let mut test = Tester::no_discount(pool.clone());
    test.sell_wallet = test.sell_wallet.clone().pack(
        spl::token_account::from_acc_info(&test.sell_wallet.to_account_info())
            .mint(pool.reserves[1].mint),
    );

    let error = test
        .swap(
            TokenAmount::new(10_000),
            TokenAmount::new(9_500),
            pool.reserves[0].mint,
            pool.reserves[1].mint,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_sell_vault_mint_not_eq_sell_wallet_mint() -> Result<()> {
    let pool = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(9),
        reserves: create_two_reserves(
            TokenAmount::new(20_000),
            TokenAmount::new(20_000),
        ),
        ..Default::default()
    };

    let mut test = Tester::no_discount(pool.clone());
    test.sell_vault = test.sell_vault.clone().pack(
        spl::token_account::from_acc_info(&test.sell_vault.to_account_info())
            .mint(Pubkey::new_unique()),
    );

    let error = test
        .swap(
            TokenAmount::new(10_000),
            TokenAmount::new(9_500),
            pool.reserves[0].mint,
            pool.reserves[1].mint,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_buy_wallet_mint_not_eq_buy_vault_mint() -> Result<()> {
    let pool = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(9),
        reserves: create_two_reserves(
            TokenAmount::new(20_000),
            TokenAmount::new(20_000),
        ),
        ..Default::default()
    };

    let mut test = Tester::no_discount(pool.clone());
    test.buy_wallet = test.buy_wallet.clone().pack(
        spl::token_account::from_acc_info(&test.buy_wallet.to_account_info())
            .mint(Pubkey::new_unique()),
    );

    let error = test
        .swap(
            TokenAmount::new(10_000),
            TokenAmount::new(9_500),
            pool.reserves[0].mint,
            pool.reserves[1].mint,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_buy_vault_does_not_match_pool_reserve_vault() -> Result<()> {
    let pool = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(9),
        reserves: create_two_reserves(
            TokenAmount::new(20_000),
            TokenAmount::new(20_000),
        ),
        ..Default::default()
    };

    let mut test = Tester::no_discount(pool.clone());
    test.buy_vault.key = Pubkey::new_unique();

    let error = test
        .swap(
            TokenAmount::new(10_000),
            TokenAmount::new(9_500),
            pool.reserves[0].mint,
            pool.reserves[1].mint,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_sell_vault_does_not_match_pool_reserve_vault() -> Result<()> {
    let pool = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(9),
        reserves: create_two_reserves(
            TokenAmount::new(20_000),
            TokenAmount::new(20_000),
        ),
        ..Default::default()
    };

    let mut test = Tester::no_discount(pool.clone());
    test.sell_vault.key = Pubkey::new_unique();

    let error = test
        .swap(
            TokenAmount::new(10_000),
            TokenAmount::new(9_500),
            pool.reserves[0].mint,
            pool.reserves[1].mint,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_lp_mint_mismatches_pool_mint() -> Result<()> {
    let pool = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(9),
        reserves: create_two_reserves(
            TokenAmount::new(20_000),
            TokenAmount::new(20_000),
        ),
        ..Default::default()
    };

    let mut test = Tester::no_discount(pool.clone());
    test.lp_mint.key = Pubkey::new_unique();

    let error = test
        .swap(
            TokenAmount::new(10_000),
            TokenAmount::new(9_500),
            pool.reserves[0].mint,
            pool.reserves[1].mint,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_lp_mint_supply_is_zero() -> Result<()> {
    let pool = Pool {
        dimension: 2,
        program_toll_wallet: Pubkey::new_unique(),
        swap_fee: Permillion::from_percent(9),
        reserves: create_two_reserves(TokenAmount::new(0), TokenAmount::new(0)),
        ..Default::default()
    };

    let mut test = Tester::no_discount(pool.clone());
    test.lp_mint = test.lp_mint.clone().pack(
        spl::mint::from_acc_info(&test.lp_mint.to_account_info()).supply(0),
    );

    let error = test
        .swap(
            TokenAmount::new(10_000),
            TokenAmount::new(9_500),
            pool.reserves[0].mint,
            pool.reserves[1].mint,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("InvalidAccountInput"));

    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
struct Tester {
    time: Slot,
    user: AccountInfoWrapper,
    discount: AccountInfoWrapper,
    pool: AccountInfoWrapper,
    pool_signer: AccountInfoWrapper,
    sell_wallet: AccountInfoWrapper,
    buy_wallet: AccountInfoWrapper,
    sell_vault: AccountInfoWrapper,
    buy_vault: AccountInfoWrapper,
    lp_mint: AccountInfoWrapper,
    program_toll_wallet: AccountInfoWrapper,
    token_program: AccountInfoWrapper,
}

impl Tester {
    fn with_discount(pool_data: Pool, user_discount: Discount) -> Self {
        Self::new(pool_data, Some(user_discount))
    }

    fn no_discount(pool_data: Pool) -> Self {
        Self::new(pool_data, None)
    }

    fn slot(mut self, slot: Slot) -> Self {
        self.time = slot;
        self
    }

    // Since the order of the reserves does not matter (that's unit tested),
    // we make a convention for parametrizing the tests:
    // The first reserve is always base (sell);
    // The second is always quote (buy).
    fn new(mut pool_data: Pool, user_discount: Option<Discount>) -> Self {
        pool_data.update_curve_invariant().ok();

        let user = AccountInfoWrapper::new().signer();
        let discount = AccountInfoWrapper::pda(
            amm::ID,
            "discount",
            &[Discount::PDA_PREFIX, user.key.as_ref()],
        );
        let discount = if let Some(d) = user_discount {
            discount.owner(amm::ID).data(d)
        } else {
            discount.owner(system_program::ID)
        };
        let pool = AccountInfoWrapper::new()
            .owner(amm::ID)
            .mutable()
            .data(pool_data.clone());
        let pool_signer = AccountInfoWrapper::pda(
            amm::ID,
            "pool_signer",
            &[Pool::SIGNER_PDA_PREFIX, pool.key.as_ref()],
        );
        let lp_mint = AccountInfoWrapper::with_key(pool_data.mint)
            .mutable()
            .pack(spl::mint::new(pool_signer.key).supply(10_000))
            .owner(token::ID);
        let program_toll_wallet =
            AccountInfoWrapper::with_key(pool_data.program_toll_wallet)
                .mutable()
                .pack(
                    spl::token_account::new(Pubkey::new_unique())
                        .mint(lp_mint.key),
                )
                .owner(token::ID);
        let Reserve {
            mint: sell_mint,
            vault: sell_vault,
            tokens: sell_tokens,
        } = pool_data.reserves()[0];
        let sell_vault = AccountInfoWrapper::with_key(sell_vault)
            .mutable()
            .pack(
                spl::token_account::new(pool_signer.key)
                    .mint(sell_mint)
                    .amount(sell_tokens.amount),
            )
            .owner(token::ID);
        let sell_wallet = AccountInfoWrapper::new()
            .mutable()
            .pack(
                spl::token_account::new(user.key)
                    .mint(sell_mint)
                    .amount(u64::MAX / 2),
            )
            .owner(token::ID);
        let Reserve {
            mint: buy_mint,
            vault: buy_vault,
            tokens: buy_tokens,
        } = pool_data.reserves()[1];
        let buy_vault = AccountInfoWrapper::with_key(buy_vault)
            .mutable()
            .pack(
                spl::token_account::new(pool_signer.key)
                    .mint(buy_mint)
                    .amount(buy_tokens.amount),
            )
            .owner(token::ID);
        let buy_wallet = AccountInfoWrapper::new()
            .mutable()
            .pack(
                spl::token_account::new(user.key)
                    .mint(buy_mint)
                    .amount(u64::MAX / 2),
            )
            .owner(token::ID);
        let token_program =
            AccountInfoWrapper::with_key(anchor_spl::token::ID).program();

        Self {
            time: Slot::new(0),
            user,
            discount,
            pool,
            pool_signer,
            sell_wallet,
            buy_wallet,
            sell_vault,
            buy_vault,
            lp_mint,
            program_toll_wallet,
            token_program,
        }
    }

    fn pool_copy(&self) -> Pool {
        Pool::try_deserialize(&mut self.pool.data.as_slice()).unwrap()
    }

    fn lp_supply(&mut self) -> u64 {
        spl::mint::from_acc_info(&self.lp_mint.to_account_info()).supply
    }

    fn swap(
        &mut self,
        sell: TokenAmount,
        min_buy: TokenAmount,
        sell_mint: Pubkey,
        buy_mint: Pubkey,
    ) -> Result<()> {
        // we set it to done initially just so that we can set the slot, will
        // overwrite it later
        self.set_syscalls(CpiValidatorState::Done);

        let mut pool = self.pool_copy();
        let fee = calculate_swap_fee(
            sell,
            pool.swap_fee,
            &self.discount.to_account_info(),
        )
        .unwrap_or_default();
        let receive_tokens = pool
            .swap(
                sell_mint,
                TokenAmount::new(sell.amount - fee.amount),
                buy_mint,
            )
            .unwrap_or_default();
        let supply =
            spl::mint::from_acc_info(&self.lp_mint.to_account_info()).supply;
        let mint_toll = calculate_toll_in_lp_tokens(
            &pool,
            fee,
            pool.reserves[0].mint,
            supply.into(),
        )
        .ok()
        .flatten()
        .map(|tokens| MintToll {
            tokens,
            signer: self.pool_signer.key,
            destination: self.program_toll_wallet.key,
            mint: self.lp_mint.key,
        });
        let state = CpiValidatorState::TransferSoldTokens {
            user: self.user.key,
            vault: self.sell_vault.key,
            wallet: self.sell_wallet.key,
            tokens: sell,
            next_cpi: TransferBoughtTokens {
                signer: self.pool_signer.key,
                vault: self.buy_vault.key,
                wallet: self.buy_wallet.key,
                tokens: receive_tokens,
                next_cpi: mint_toll,
            },
        };
        let state = self.set_syscalls(state);

        let mut ctx = self.context_wrapper();
        let mut accounts = ctx.accounts()?;

        swap(ctx.build(&mut accounts), sell, min_buy)?;
        accounts.exit(&amm::ID)?;

        assert_eq!(*state.lock().unwrap(), CpiValidatorState::Done);

        Ok(())
    }

    fn context_wrapper(&mut self) -> ContextWrapper {
        ContextWrapper::new(amm::ID)
            .acc(&mut self.user)
            .acc(&mut self.discount)
            .acc(&mut self.pool)
            .acc(&mut self.pool_signer)
            .acc(&mut self.sell_wallet)
            .acc(&mut self.buy_wallet)
            .acc(&mut self.sell_vault)
            .acc(&mut self.buy_vault)
            .acc(&mut self.lp_mint)
            .acc(&mut self.program_toll_wallet)
            .acc(&mut self.token_program)
    }

    fn set_syscalls(
        &self,
        state: CpiValidatorState,
    ) -> Arc<Mutex<CpiValidatorState>> {
        let state = Arc::new(Mutex::new(state));
        stub::Syscalls::new(CpiValidator(Arc::clone(&state)))
            .slot(self.time.slot)
            .set();
        state
    }
}

struct CpiValidator(Arc<Mutex<CpiValidatorState>>);
#[derive(Debug, Eq, PartialEq)]
enum CpiValidatorState {
    TransferSoldTokens {
        user: Pubkey,
        vault: Pubkey,
        wallet: Pubkey,
        tokens: TokenAmount,
        next_cpi: TransferBoughtTokens,
    },
    TransferBoughtTokens(TransferBoughtTokens),
    MintToll(MintToll),
    Done,
}
#[derive(Debug, Eq, PartialEq, Clone)]
struct TransferBoughtTokens {
    signer: Pubkey,
    vault: Pubkey,
    wallet: Pubkey,
    tokens: TokenAmount,
    next_cpi: Option<MintToll>,
}
#[derive(Debug, Eq, PartialEq, Clone)]
struct MintToll {
    signer: Pubkey,
    mint: Pubkey,
    destination: Pubkey,
    tokens: TokenAmount,
}

impl stub::ValidateCpis for CpiValidator {
    fn validate_next_instruction(
        &mut self,
        ix: &Instruction,
        accounts: &[AccountInfo],
    ) {
        let mut state = self.0.lock().unwrap();
        match *state {
            CpiValidatorState::TransferSoldTokens {
                user,
                vault,
                wallet,
                tokens,
                ref next_cpi,
            } => {
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

                let from_wallet = &accounts[0];
                let to_vault = &accounts[1];
                assert_eq!(from_wallet.key(), wallet.key());
                assert_eq!(to_vault.key(), vault.key());

                spl::token_account::transfer(
                    &from_wallet,
                    &to_vault,
                    tokens.amount,
                )
                .expect("Source wallet does not have enough tokens");

                *state =
                    CpiValidatorState::TransferBoughtTokens(next_cpi.clone());
            }
            CpiValidatorState::TransferBoughtTokens(TransferBoughtTokens {
                signer,
                vault,
                wallet,
                tokens,
                ref next_cpi,
            }) => {
                let expected_ix = token::spl_token::instruction::transfer(
                    &token::ID,
                    &vault,
                    &wallet,
                    &signer,
                    &[],
                    tokens.amount,
                )
                .unwrap();
                assert_eq!(&expected_ix, ix);

                let from_vault = &accounts[0];
                let to_wallet = &accounts[1];
                assert_eq!(from_vault.key(), vault.key());
                assert_eq!(to_wallet.key(), wallet.key());

                spl::token_account::transfer(
                    &from_vault,
                    &to_wallet,
                    tokens.amount,
                )
                .expect("Source vault does not have enough tokens");

                *state = if let Some(next_cpi) = next_cpi {
                    CpiValidatorState::MintToll(next_cpi.clone())
                } else {
                    CpiValidatorState::Done
                };
            }
            CpiValidatorState::MintToll(MintToll {
                mint,
                destination,
                signer,
                tokens,
            }) => {
                let expected_ix = token::spl_token::instruction::mint_to(
                    &token::ID,
                    &mint,
                    &destination,
                    &signer,
                    &[],
                    tokens.amount,
                )
                .unwrap();
                assert_eq!(&expected_ix, ix);

                let wallet = &accounts[0];
                let lp_mint = &accounts[1];
                assert_eq!(wallet.key(), destination);
                assert_eq!(lp_mint.key(), mint);

                spl::mint::mint_to(wallet, lp_mint, tokens.amount)
                    .expect("Cannot mint LP tokens");

                *state = CpiValidatorState::Done;
            }
            CpiValidatorState::Done => {
                panic!("No more instructions expected, got {:#?}", ix);
            }
        }
    }
}

fn create_two_reserves(sell: TokenAmount, buy: TokenAmount) -> [Reserve; 4] {
    [
        Reserve {
            mint: Pubkey::new_unique(),
            vault: Pubkey::new_unique(),
            tokens: sell,
        },
        Reserve {
            mint: Pubkey::new_unique(),
            vault: Pubkey::new_unique(),
            tokens: buy,
        },
        Reserve::default(),
        Reserve::default(),
    ]
}

fn create_reserves(amounts: &[TokenAmount]) -> [Reserve; 4] {
    amounts
        .into_iter()
        .copied()
        .map(|tokens| Reserve {
            mint: Pubkey::new_unique(),
            vault: Pubkey::new_unique(),
            tokens,
        })
        .chain(iter::repeat(Reserve::default()))
        .take(4)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}
