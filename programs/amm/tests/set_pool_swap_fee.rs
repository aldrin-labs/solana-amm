use ::amm::amm::set_pool_swap_fee;
use ::amm::prelude::*;
use anchortest::builder::*;
use pretty_assertions::assert_eq;
use serial_test::serial;

#[test]
#[serial]
fn works() -> Result<()> {
    let mut test = Tester::default();

    let fee = Permillion { permillion: 5_000 };
    assert!(test.set_pool_swap_fee(fee).is_ok());

    let pool = Pool::try_deserialize(&mut test.pool.data.as_slice())?;
    assert_eq!(pool.fee, fee);

    Ok(())
}

#[test]
#[serial]
fn max_swap_fee_is_inclusive() -> Result<()> {
    let mut test = Tester::default();

    assert!(test.set_pool_swap_fee(consts::MAX_SWAP_FEE).is_ok());

    let pool = Pool::try_deserialize(&mut test.pool.data.as_slice())?;
    assert_eq!(pool.fee, consts::MAX_SWAP_FEE);

    Ok(())
}

#[test]
#[serial]
fn fails_if_discount_more_than_1_percent() -> Result<()> {
    let mut test = Tester::default();

    let fee = Permillion {
        permillion: 100_000,
    };
    assert!(test
        .set_pool_swap_fee(fee)
        .unwrap_err()
        .to_string()
        .contains("InvalidArg"));

    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
struct Tester {
    admin: AccountInfoWrapper,
    pool: AccountInfoWrapper,
}

impl Default for Tester {
    fn default() -> Self {
        let admin = AccountInfoWrapper::new().signer();
        let pool =
            AccountInfoWrapper::new()
                .mutable()
                .owner(amm::ID)
                .data(Pool {
                    admin: admin.key,
                    ..Default::default()
                });

        Self { admin, pool }
    }
}

impl Tester {
    fn set_pool_swap_fee(&mut self, fee: Permillion) -> Result<()> {
        let mut ctx = self.context_wrapper();
        let mut accounts = ctx.accounts()?;

        set_pool_swap_fee(ctx.build(&mut accounts), fee)?;
        accounts.exit(&amm::ID)?;

        Ok(())
    }

    fn context_wrapper(&mut self) -> ContextWrapper {
        ContextWrapper::new(amm::ID)
            .acc(&mut self.admin)
            .acc(&mut self.pool)
    }
}
