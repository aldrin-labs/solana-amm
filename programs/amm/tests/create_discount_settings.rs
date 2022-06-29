use ::amm::amm::create_discount_settings;
use ::amm::prelude::*;
use anchor_lang::solana_program::system_instruction;
use anchor_lang::system_program;
use anchortest::{builder::*, stub};
use pretty_assertions::assert_eq;
use serial_test::serial;
use solana_sdk::instruction::Instruction;

#[test]
#[serial]
fn works() -> Result<()> {
    let mut test = Tester::default();
    let og_state = test.clone();

    assert!(test.create_discount_settings().is_ok());

    let settings = DiscountSettings::try_deserialize(
        &mut test.discount_settings.data.as_slice(),
    )?;
    assert_eq!(settings.authority, test.discount_settings_authority.key);

    // no other changes should have happened
    test.discount_settings = og_state.discount_settings.clone();
    assert_eq!(test, og_state);

    Ok(())
}

#[test]
#[serial]
fn fails_if_program_data_does_not_match() -> Result<()> {
    let mut test = Tester::default();
    test.amm = AccountInfoWrapper::with_key(amm::ID)
        .program_with_data_addr(Pubkey::new_unique());

    assert!(test
        .create_discount_settings()
        .unwrap_err()
        .to_string()
        .contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_program_authority_does_not_match() -> Result<()> {
    let mut test = Tester::default();
    test.amm_metadata =
        AccountInfoWrapper::new().program_data(Pubkey::new_unique());

    assert!(test
        .create_discount_settings()
        .unwrap_err()
        .to_string()
        .contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_pda_does_not_match() -> Result<()> {
    let mut test = Tester::default();
    test.discount_settings =
        AccountInfoWrapper::pda(amm::ID, "discount_settings", &[b"wrong_seed"])
            .size(DiscountSettings::space())
            .mutable()
            .owner(amm::ID);

    assert!(test
        .create_discount_settings()
        .unwrap_err()
        .to_string()
        .contains("ConstraintSeeds"));

    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
struct Tester {
    program_authority: AccountInfoWrapper,
    amm: AccountInfoWrapper,
    amm_metadata: AccountInfoWrapper,
    discount_settings_authority: AccountInfoWrapper,
    discount_settings: AccountInfoWrapper,
    system_program: AccountInfoWrapper,
}

impl Default for Tester {
    fn default() -> Self {
        let program_authority = AccountInfoWrapper::new().mutable().signer();
        let amm_metadata =
            AccountInfoWrapper::new().program_data(program_authority.key);
        let amm = AccountInfoWrapper::with_key(amm::ID)
            .program_with_data_addr(amm_metadata.key);
        let discount_settings_authority = AccountInfoWrapper::new();
        let discount_settings = AccountInfoWrapper::pda(
            amm::ID,
            "discount_settings",
            &[DiscountSettings::ACCOUNT_SEED],
        )
        .size(DiscountSettings::space())
        .mutable()
        .owner(amm::ID);
        let system_program =
            AccountInfoWrapper::with_key(system_program::ID).program();

        Self {
            amm_metadata,
            amm,
            program_authority,
            discount_settings_authority,
            discount_settings,
            system_program,
        }
    }
}

impl Tester {
    fn create_discount_settings(&mut self) -> Result<()> {
        self.set_syscalls();

        println!("Getting context...");
        let mut ctx = self.context_wrapper();
        println!("Getting accounts...");
        let mut accounts = ctx.accounts()?;

        println!("Executing endpoint...");
        create_discount_settings(ctx.build(&mut accounts))?;
        accounts.exit(&amm::ID)?;

        Ok(())
    }

    fn context_wrapper(&mut self) -> ContextWrapper {
        ContextWrapper::new(amm::ID)
            .acc(&mut self.program_authority)
            .acc(&mut self.amm)
            .acc(&mut self.amm_metadata)
            .acc(&mut self.discount_settings_authority)
            .acc(&mut self.discount_settings)
            .acc(&mut self.system_program)
    }

    fn set_syscalls(&self) {
        stub::Syscalls::new(CpiValidator(
            CpiValidatorState::CreateDiscountSettings {
                program_authority: self.program_authority.key,
                discount_settings: self.discount_settings.key,
            },
        ))
        .set();
    }
}

struct CpiValidator(CpiValidatorState);
enum CpiValidatorState {
    CreateDiscountSettings {
        program_authority: Pubkey,
        discount_settings: Pubkey,
    },
    Done,
}

impl stub::ValidateCpis for CpiValidator {
    fn validate_next_instruction(
        &mut self,
        ix: &Instruction,
        accounts: &[AccountInfo],
    ) {
        match self.0 {
            CpiValidatorState::CreateDiscountSettings {
                program_authority,
                discount_settings,
            } => {
                let rent =
                    Rent::default().minimum_balance(DiscountSettings::space());
                let expected_ix = system_instruction::create_account(
                    &program_authority,
                    &discount_settings,
                    rent,
                    DiscountSettings::space() as u64,
                    &amm::ID,
                );
                assert_eq!(&expected_ix, ix);

                let discount_settings = accounts
                    .iter()
                    .find(|acc| acc.key() == discount_settings)
                    .unwrap();
                let mut lamports = discount_settings.lamports.borrow_mut();
                **lamports = rent;

                self.0 = CpiValidatorState::Done;
            }
            CpiValidatorState::Done => {
                panic!("No more instructions expected, got {:#?}", ix);
            }
        }
    }
}
