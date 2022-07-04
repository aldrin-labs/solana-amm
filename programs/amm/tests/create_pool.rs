use ::amm::amm::create_pool;
use ::amm::prelude::*;
use anchor_lang::solana_program::system_instruction;
use anchor_lang::system_program;
use anchor_spl::token;
use anchor_spl::token::spl_token::state::AccountState;
use anchortest::{
    builder::*,
    spl::{self, MintExt, TokenAccountExt},
    stub,
};
use pretty_assertions::assert_eq;
use serial_test::serial;
use solana_sdk::instruction::Instruction;
use solana_sdk::program_option::COption;
use std::iter;

const CONST_PROD_AMPLIFIER: u64 = 0;

#[test]
#[serial]
fn with_two_reserves() -> Result<()> {
    let mut test = Tester::default();
    let og_state = test.clone();

    assert!(test.create_pool(CONST_PROD_AMPLIFIER).is_ok());

    let pool = Pool::try_deserialize(&mut test.pool.data.as_slice())?;
    assert_eq!(pool.dimension, 2);
    assert_eq!(pool.mint, test.lp_mint.key);
    assert_eq!(pool.program_toll_wallet, test.program_toll_wallet.key);
    assert_eq!(pool.admin, test.admin.key);
    assert_eq!(pool.signer, test.pool_signer.key);
    assert_eq!(pool.curve, Curve::ConstProd);
    for (reserve, vault) in pool.reserves[0..2].iter().zip(&test.vaults) {
        assert_eq!(vault.key, reserve.vault);
        assert_eq!(reserve.tokens.amount, 0);
        let mint =
            token::TokenAccount::try_deserialize(&mut vault.data.as_slice())?
                .mint;
        assert_eq!(mint, reserve.mint);
    }
    assert_eq!(pool.reserves[2], Reserve::default());
    assert_eq!(pool.reserves[3], Reserve::default());

    // no other changes should have happened
    test.pool = og_state.pool.clone();
    assert_eq!(test, og_state);

    Ok(())
}

#[test]
#[serial]
fn with_three_reserves() -> Result<()> {
    let mut test = Tester::default();
    test.vaults = iter::repeat_with(|| {
        AccountInfoWrapper::new()
            .pack(spl::token_account(test.pool_signer.key))
            .owner(token::ID)
    })
    .take(3)
    .collect();
    let og_state = test.clone();

    assert!(test.create_pool(CONST_PROD_AMPLIFIER).is_ok());

    let pool = Pool::try_deserialize(&mut test.pool.data.as_slice())?;
    assert_eq!(pool.dimension, 3);
    assert_eq!(pool.curve, Curve::ConstProd);
    for (reserve, vault) in pool.reserves[0..3].iter().zip(&test.vaults) {
        assert_eq!(vault.key, reserve.vault);
        assert_eq!(reserve.tokens.amount, 0);
        let mint =
            token::TokenAccount::try_deserialize(&mut vault.data.as_slice())?
                .mint;
        assert_eq!(mint, reserve.mint);
    }
    assert_eq!(pool.reserves[3], Reserve::default());

    // no other changes should have happened
    test.pool = og_state.pool.clone();
    assert_eq!(test, og_state);

    Ok(())
}

#[test]
#[serial]
fn with_four_reserves() -> Result<()> {
    let mut test = Tester::default();
    test.vaults = iter::repeat_with(|| {
        AccountInfoWrapper::new()
            .pack(spl::token_account(test.pool_signer.key))
            .owner(token::ID)
    })
    .take(4)
    .collect();
    let og_state = test.clone();

    assert!(test.create_pool(CONST_PROD_AMPLIFIER).is_ok());

    let pool = Pool::try_deserialize(&mut test.pool.data.as_slice())?;
    assert_eq!(pool.dimension, 4);
    assert_eq!(pool.curve, Curve::ConstProd);
    for (reserve, vault) in pool.reserves.iter().zip(&test.vaults) {
        assert_eq!(vault.key, reserve.vault);
        assert_eq!(reserve.tokens.amount, 0);
        let mint =
            token::TokenAccount::try_deserialize(&mut vault.data.as_slice())?
                .mint;
        assert_eq!(mint, reserve.mint);
    }

    // no other changes should have happened
    test.pool = og_state.pool.clone();
    assert_eq!(test, og_state);

    Ok(())
}

#[test]
#[serial]
fn fails_if_more_than_four_reserves() -> Result<()> {
    let mut test = Tester::default();
    test.vaults = iter::repeat_with(|| {
        AccountInfoWrapper::new()
            .pack(spl::token_account(test.pool_signer.key))
            .owner(token::ID)
    })
    .take(5)
    .collect();

    assert!(test
        .create_pool(CONST_PROD_AMPLIFIER)
        .unwrap_err()
        .to_string()
        .contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_less_than_two_reserves() -> Result<()> {
    let mut test = Tester::default();
    test.vaults = iter::repeat_with(|| {
        AccountInfoWrapper::new()
            .pack(spl::token_account(test.pool_signer.key))
            .owner(token::ID)
    })
    .take(1)
    .collect();
    assert!(test
        .create_pool(CONST_PROD_AMPLIFIER)
        .unwrap_err()
        .to_string()
        .contains("InvalidAccountInput"));

    let mut test = Tester::default();
    test.vaults = vec![];
    assert!(test
        .create_pool(CONST_PROD_AMPLIFIER)
        .unwrap_err()
        .to_string()
        .contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn uses_stable_curve_if_amplifier_not_zero() -> Result<()> {
    let mut test = Tester::default();
    let og_state = test.clone();

    let stable_curve_amplifier = 2;
    assert!(test.create_pool(stable_curve_amplifier).is_ok());

    let pool = Pool::try_deserialize(&mut test.pool.data.as_slice())?;
    assert_eq!(
        pool.curve,
        Curve::Stable {
            amplifier: stable_curve_amplifier,
            invariant: SDecimal::default()
        }
    );

    // no other changes should have happened
    test.pool = og_state.pool.clone();
    assert_eq!(test, og_state);

    Ok(())
}

#[test]
#[serial]
fn fails_on_duplicate_reserve_mint() -> Result<()> {
    let mut test = Tester::default();
    let mint = Pubkey::new_unique();
    test.vaults = iter::repeat_with(|| {
        AccountInfoWrapper::new()
            .pack(spl::token_account(test.pool_signer.key).mint(mint))
            .owner(token::ID)
    })
    .take(2)
    .collect();

    assert!(test
        .create_pool(CONST_PROD_AMPLIFIER)
        .unwrap_err()
        .to_string()
        .contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_vault_has_close_authority() -> Result<()> {
    let mut test = Tester::default();
    test.vaults = iter::repeat_with(|| {
        AccountInfoWrapper::new()
            .pack({
                let mut vault = spl::token_account(test.pool_signer.key);
                vault.close_authority = COption::Some(Pubkey::new_unique());

                vault
            })
            .owner(token::ID)
    })
    .take(2)
    .collect();

    assert!(test
        .create_pool(CONST_PROD_AMPLIFIER)
        .unwrap_err()
        .to_string()
        .contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_vault_has_delegate() -> Result<()> {
    let mut test = Tester::default();
    test.vaults = iter::repeat_with(|| {
        AccountInfoWrapper::new()
            .pack({
                let mut vault = spl::token_account(test.pool_signer.key);
                vault.delegate = COption::Some(Pubkey::new_unique());

                vault
            })
            .owner(token::ID)
    })
    .take(2)
    .collect();

    assert!(test
        .create_pool(CONST_PROD_AMPLIFIER)
        .unwrap_err()
        .to_string()
        .contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_vault_owner_is_not_pool_signer() -> Result<()> {
    let mut test = Tester::default();
    test.vaults = iter::repeat_with(|| {
        AccountInfoWrapper::new()
            .pack({
                let mut vault = spl::token_account(test.pool_signer.key);
                vault.owner = Pubkey::new_unique();

                vault
            })
            .owner(token::ID)
    })
    .take(2)
    .collect();

    assert!(test
        .create_pool(CONST_PROD_AMPLIFIER)
        .unwrap_err()
        .to_string()
        .contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_vault_is_frozen() -> Result<()> {
    let mut test = Tester::default();
    test.vaults = iter::repeat_with(|| {
        AccountInfoWrapper::new()
            .pack({
                let mut vault = spl::token_account(test.pool_signer.key);
                vault.state = AccountState::Frozen;

                vault
            })
            .owner(token::ID)
    })
    .take(2)
    .collect();

    assert!(test
        .create_pool(CONST_PROD_AMPLIFIER)
        .unwrap_err()
        .to_string()
        .contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_vaults_not_empty_but_lp_mint_supply_is_zero() -> Result<()> {
    let mut test = Tester::default();
    test.vaults = iter::repeat_with(|| {
        AccountInfoWrapper::new()
            .pack(spl::token_account(test.pool_signer.key).amount(10))
            .owner(token::ID)
    })
    .take(2)
    .collect();

    assert!(test
        .create_pool(CONST_PROD_AMPLIFIER)
        .unwrap_err()
        .to_string()
        .contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_vaults_empty_but_lp_mint_supply_is_not_zero() -> Result<()> {
    let mut test = Tester::default();
    test.lp_mint.data = AccountInfoWrapper::new()
        .pack(spl::mint(test.pool_signer.key).supply(10))
        .data;

    assert!(test
        .create_pool(CONST_PROD_AMPLIFIER)
        .unwrap_err()
        .to_string()
        .contains("InvalidAccountInput"));

    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
struct Tester {
    admin: AccountInfoWrapper,
    pool: AccountInfoWrapper,
    pool_signer: AccountInfoWrapper,
    lp_mint: AccountInfoWrapper,
    program_toll: AccountInfoWrapper,
    program_toll_wallet: AccountInfoWrapper,
    token_program: AccountInfoWrapper,
    system_program: AccountInfoWrapper,
    vaults: Vec<AccountInfoWrapper>,
}

impl Default for Tester {
    fn default() -> Self {
        let admin = AccountInfoWrapper::new().mutable().signer();
        let pool = AccountInfoWrapper::new()
            .signer()
            .owner(amm::ID)
            .mutable()
            .size(Pool::space());
        let pool_signer = AccountInfoWrapper::pda(
            amm::ID,
            "pool_signer",
            &[Pool::SIGNER_PDA_PREFIX, pool.key.as_ref()],
        );
        let lp_mint = AccountInfoWrapper::new()
            .pack(spl::mint(pool_signer.key))
            .owner(token::ID);
        let program_toll_authority = Pubkey::new_unique();
        let program_toll = AccountInfoWrapper::pda(
            amm::ID,
            "program_toll",
            &[ProgramToll::PDA_SEED],
        )
        .data(ProgramToll {
            authority: program_toll_authority,
        })
        .owner(amm::ID);
        let program_toll_wallet = AccountInfoWrapper::new()
            .pack(spl::token_account(program_toll_authority).mint(lp_mint.key))
            .owner(token::ID);
        let token_program = AccountInfoWrapper::with_key(token::ID).program();
        let system_program =
            AccountInfoWrapper::with_key(system_program::ID).program();
        let vaults = iter::repeat_with(|| {
            AccountInfoWrapper::new()
                .pack(spl::token_account(pool_signer.key))
                .owner(token::ID)
        })
        .take(2)
        .collect();

        Self {
            admin,
            pool,
            pool_signer,
            lp_mint,
            program_toll,
            program_toll_wallet,
            token_program,
            system_program,
            vaults,
        }
    }
}

impl Tester {
    fn create_pool(&mut self, amplifier: u64) -> Result<()> {
        self.set_syscalls();

        let mut ctx = self.context_wrapper();
        let mut accounts = ctx.accounts()?;

        create_pool(ctx.build(&mut accounts), amplifier)?;
        accounts.exit(&amm::ID)?;

        Ok(())
    }

    fn context_wrapper(&mut self) -> ContextWrapper {
        ContextWrapper::new(amm::ID)
            .acc(&mut self.admin)
            .acc(&mut self.pool)
            .acc(&mut self.pool_signer)
            .acc(&mut self.program_toll)
            .acc(&mut self.program_toll_wallet)
            .acc(&mut self.lp_mint)
            .acc(&mut self.token_program)
            .acc(&mut self.system_program)
            .remaining_accounts(self.vaults.iter_mut())
    }

    fn set_syscalls(&self) {
        stub::Syscalls::new(CpiValidator(CpiValidatorState::CreatePool {
            admin: self.admin.key,
            pool: self.pool.key,
        }))
        .set();
    }
}

struct CpiValidator(CpiValidatorState);
enum CpiValidatorState {
    CreatePool { admin: Pubkey, pool: Pubkey },
    Done,
}

impl stub::ValidateCpis for CpiValidator {
    fn validate_next_instruction(
        &mut self,
        ix: &Instruction,
        accounts: &[AccountInfo],
    ) {
        match self.0 {
            CpiValidatorState::CreatePool { admin, pool } => {
                let rent = Rent::default().minimum_balance(Pool::space());
                let expected_ix = system_instruction::create_account(
                    &admin,
                    &pool,
                    rent,
                    Pool::space() as u64,
                    &amm::ID,
                );
                assert_eq!(&expected_ix, ix);

                let pool =
                    accounts.iter().find(|acc| acc.key() == pool).unwrap();
                let mut lamports = pool.lamports.borrow_mut();
                **lamports = rent;

                self.0 = CpiValidatorState::Done;
            }
            CpiValidatorState::Done => {
                panic!("No more instructions expected, got {:#?}", ix);
            }
        }
    }
}
