use ::amm::amm::put_discount;
use ::amm::prelude::*;
use anchor_lang::solana_program::system_instruction;
use anchor_lang::system_program;
use anchortest::{builder::*, stub};
use pretty_assertions::assert_eq;
use serial_test::serial;
use solana_sdk::instruction::Instruction;
use std::sync::{Arc, Mutex};

#[test]
#[serial]
fn creates_new_discount() -> Result<()> {
    let user = Pubkey::new_unique();
    let discount_amount = Permillion::from_percent(50);
    let valid_until = Slot::new(500);

    let mut test = Tester::new(user);
    let og_state = test.clone();

    assert!(test
        .put_discount_create(discount_amount, valid_until)
        .is_ok());

    let discount =
        Discount::try_deserialize(&mut test.discount.data.as_slice())?;
    assert_eq!(
        discount,
        Discount {
            valid_until,
            amount: discount_amount
        }
    );

    // no other changes should have happened
    test.discount = og_state.discount.clone();
    assert_eq!(test, og_state);

    Ok(())
}

#[test]
#[serial]
fn updates_existing_discount() -> Result<()> {
    let user = Pubkey::new_unique();

    let mut test = Tester::new(user);
    let og_state = test.clone();

    let discount_amount = Permillion::from_percent(50);
    let valid_until = Slot::new(500);
    assert!(test
        .put_discount_create(discount_amount, valid_until)
        .is_ok());
    // the stub of the system_program::create_account doesn't change the
    // owner of the account, so we need to change it manually
    test.discount.owner = amm::ID;

    let discount_amount = Permillion::from_percent(25);
    let valid_until = Slot::new(550);
    test.authority.is_writable = false;
    assert!(test
        .put_discount_update(discount_amount, valid_until)
        .is_ok());

    let discount =
        Discount::try_deserialize(&mut test.discount.data.as_slice())?;
    assert_eq!(
        discount,
        Discount {
            valid_until,
            amount: discount_amount
        }
    );

    // no other changes should have happened
    test.discount = og_state.discount.clone();
    test.authority.is_writable = og_state.authority.is_writable;
    assert_eq!(test, og_state);

    Ok(())
}

#[test]
#[serial]
fn fails_if_valid_from_slot_less_or_eq_to_current_slot() -> Result<()> {
    let user = Pubkey::new_unique();
    let discount_amount = Permillion::from_percent(50);
    let valid_until = Slot::new(500);

    let mut test = Tester::new(user).slot(600);
    assert!(test
        .put_discount_create(discount_amount, valid_until)
        .unwrap_err()
        .to_string()
        .contains("InvalidArg"));

    let mut test = Tester::new(user).slot(500);
    assert!(test
        .put_discount_create(discount_amount, valid_until)
        .unwrap_err()
        .to_string()
        .contains("InvalidArg"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_discount_is_over_100_percent() -> Result<()> {
    let user = Pubkey::new_unique();
    let valid_until = Slot::new(500);

    let mut test = Tester::new(user);
    let discount_amount = Permillion::from_percent(101);
    assert!(test
        .put_discount_create(discount_amount, valid_until)
        .unwrap_err()
        .to_string()
        .contains("InvalidArg"));

    Ok(())
}

#[test]
#[serial]
fn fails_if_authority_is_not_mutable_on_creation() -> Result<()> {
    let user = Pubkey::new_unique();
    let valid_until = Slot::new(500);
    let discount_amount = Permillion::from_percent(50);

    let mut test = Tester::new(user);
    test.authority.is_writable = false;
    assert!(test
        .put_discount_create(discount_amount, valid_until)
        .unwrap_err()
        .to_string()
        .contains("InvalidAccountInput"));

    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
struct Tester {
    user: Pubkey,
    slot: u64,
    authority: AccountInfoWrapper,
    discount: AccountInfoWrapper,
    discount_settings: AccountInfoWrapper,
    system_program: AccountInfoWrapper,
}

impl Tester {
    fn new(user: Pubkey) -> Self {
        let authority = AccountInfoWrapper::new().mutable().signer();
        let discount = AccountInfoWrapper::pda(
            amm::ID,
            "discount",
            &[Discount::PDA_PREFIX, user.as_ref()],
        )
        .owner(system_program::ID)
        .mutable()
        .size(Discount::space());
        let discount_settings = AccountInfoWrapper::pda(
            amm::ID,
            "discount_settings",
            &[DiscountSettings::PDA_SEED],
        )
        .data(DiscountSettings {
            authority: authority.key,
        })
        .owner(amm::ID);
        let system_program =
            AccountInfoWrapper::with_key(system_program::ID).program();

        Self {
            user,
            slot: 0,
            authority,
            discount,
            discount_settings,
            system_program,
        }
    }
}

impl Tester {
    fn slot(mut self, slot: u64) -> Self {
        self.slot = slot;
        self
    }

    /// Does not expect a call to [`system_program::create_account`]
    fn put_discount_update(
        &mut self,
        discount_amount: Permillion,
        valid_until: Slot,
    ) -> Result<()> {
        self.put_discount(CpiValidatorState::Done, discount_amount, valid_until)
    }

    /// Will expect a call to [`system_program::create_account`]
    fn put_discount_create(
        &mut self,
        discount_amount: Permillion,
        valid_until: Slot,
    ) -> Result<()> {
        self.put_discount(
            CpiValidatorState::CreateDiscount {
                payer: self.authority.key,
                discount: self.discount.key,
            },
            discount_amount,
            valid_until,
        )
    }

    fn put_discount(
        &mut self,
        state: CpiValidatorState,
        discount_amount: Permillion,
        valid_until: Slot,
    ) -> Result<()> {
        let user = self.user;

        let state = self.set_syscalls(state);

        let mut ctx = self.context_wrapper();
        let mut accounts = ctx.accounts()?;

        put_discount(
            ctx.build(&mut accounts),
            user,
            discount_amount,
            valid_until,
        )?;
        accounts.exit(&amm::ID)?;

        assert_eq!(*state.lock().unwrap(), CpiValidatorState::Done);

        Ok(())
    }

    fn context_wrapper(&mut self) -> ContextWrapper {
        ContextWrapper::new(amm::ID)
            .acc(&mut self.authority)
            .acc(&mut self.discount)
            .acc(&mut self.discount_settings)
            .acc(&mut self.system_program)
            .ix_data(self.user.as_ref().to_vec())
    }

    fn set_syscalls(
        &self,
        state: CpiValidatorState,
    ) -> Arc<Mutex<CpiValidatorState>> {
        let state = Arc::new(Mutex::new(state));

        let syscalls = stub::Syscalls::new(CpiValidator(Arc::clone(&state)));
        syscalls.slot(self.slot);
        syscalls.set();

        state
    }
}

struct CpiValidator(Arc<Mutex<CpiValidatorState>>);
#[derive(Debug, Eq, PartialEq)]
enum CpiValidatorState {
    CreateDiscount { payer: Pubkey, discount: Pubkey },
    Done,
}

impl stub::ValidateCpis for CpiValidator {
    fn validate_next_instruction(
        &mut self,
        ix: &Instruction,
        accounts: &[AccountInfo],
    ) {
        let mut state = self.0.lock().unwrap();
        match *state {
            CpiValidatorState::CreateDiscount { payer, discount } => {
                let rent = Rent::default().minimum_balance(Discount::space());
                let expected_ix = system_instruction::create_account(
                    &payer,
                    &discount,
                    rent,
                    Discount::space() as u64,
                    &amm::ID,
                );
                assert_eq!(&expected_ix, ix);

                let pool =
                    accounts.iter().find(|acc| acc.key() == discount).unwrap();
                let mut lamports = pool.lamports.borrow_mut();
                **lamports = rent;

                *state = CpiValidatorState::Done;
            }
            CpiValidatorState::Done => {
                panic!("No more instructions expected, got {:#?}", ix);
            }
        }
    }
}
