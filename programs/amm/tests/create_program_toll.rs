use ::amm::amm::create_program_toll;
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

    assert!(test.create_program_toll().is_ok());

    let toll =
        ProgramToll::try_deserialize(&mut test.program_toll.data.as_slice())?;
    assert_eq!(toll.authority, test.program_toll_authority.key);

    // no other changes should have happened
    test.program_toll = og_state.program_toll.clone();
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
        .create_program_toll()
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
        .create_program_toll()
        .unwrap_err()
        .to_string()
        .contains("InvalidAccountInput"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_pda_does_not_match() -> Result<()> {
    let mut test = Tester::default();
    test.program_toll =
        AccountInfoWrapper::pda(amm::ID, "program_toll", &[b"wrong_seed"])
            .size(ProgramToll::space())
            .mutable()
            .owner(amm::ID);

    assert!(test
        .create_program_toll()
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
    program_toll_authority: AccountInfoWrapper,
    program_toll: AccountInfoWrapper,
    system_program: AccountInfoWrapper,
}

impl Default for Tester {
    fn default() -> Self {
        let program_authority = AccountInfoWrapper::new().mutable().signer();
        let amm_metadata =
            AccountInfoWrapper::new().program_data(program_authority.key);
        let amm = AccountInfoWrapper::with_key(amm::ID)
            .program_with_data_addr(amm_metadata.key);
        let program_toll_authority = AccountInfoWrapper::new();
        let program_toll = AccountInfoWrapper::pda(
            amm::ID,
            "program_toll",
            &[ProgramToll::ACCOUNT_SEED],
        )
        .size(ProgramToll::space())
        .mutable()
        .owner(amm::ID);
        let system_program =
            AccountInfoWrapper::with_key(system_program::ID).program();

        Self {
            amm_metadata,
            amm,
            program_authority,
            program_toll_authority,
            program_toll,
            system_program,
        }
    }
}

impl Tester {
    fn create_program_toll(&mut self) -> Result<()> {
        self.set_syscalls();

        println!("Getting context...");
        let mut ctx = self.context_wrapper();
        println!("Getting accounts...");
        let mut accounts = ctx.accounts()?;

        println!("Executing endpoint...");
        create_program_toll(ctx.build(&mut accounts))?;
        accounts.exit(&amm::ID)?;

        Ok(())
    }

    fn context_wrapper(&mut self) -> ContextWrapper {
        ContextWrapper::new(amm::ID)
            .acc(&mut self.program_authority)
            .acc(&mut self.amm)
            .acc(&mut self.amm_metadata)
            .acc(&mut self.program_toll_authority)
            .acc(&mut self.program_toll)
            .acc(&mut self.system_program)
    }

    fn set_syscalls(&self) {
        stub::Syscalls::new(CpiValidator(
            CpiValidatorState::CreateProgramToll {
                program_authority: self.program_authority.key,
                program_toll: self.program_toll.key,
            },
        ))
        .set();
    }
}

struct CpiValidator(CpiValidatorState);
enum CpiValidatorState {
    CreateProgramToll {
        program_authority: Pubkey,
        program_toll: Pubkey,
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
            CpiValidatorState::CreateProgramToll {
                program_authority,
                program_toll,
            } => {
                let rent =
                    Rent::default().minimum_balance(ProgramToll::space());
                let expected_ix = system_instruction::create_account(
                    &program_authority,
                    &program_toll,
                    rent,
                    ProgramToll::space() as u64,
                    &amm::ID,
                );
                assert_eq!(&expected_ix, ix);

                let program_toll = accounts
                    .iter()
                    .find(|acc| acc.key() == program_toll)
                    .unwrap();
                let mut lamports = program_toll.lamports.borrow_mut();
                **lamports = rent;

                self.0 = CpiValidatorState::Done;
            }
            CpiValidatorState::Done => {
                panic!("No more instructions expected, got {:#?}", ix);
            }
        }
    }
}
