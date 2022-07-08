//! Implements Newton-Raphson method for numerical approximation of
//! differentiable function zeroes.
//!
//! https://en.wikipedia.org/wiki/Newton%27s_method

use crate::prelude::*;

// The method should converge within few iterations, due to the fact
// we are approximating positive root from a well positioned first
// initial guess.
// We use the same max that was used in the old AMM version.
const MAX_ITERATIONS: usize = 32;

pub fn compute(
    amp: u64,
    token_reserves_amount: &[TokenAmount],
) -> Result<Decimal> {
    // if amplifier is zero, then the invariant of the curve is just the product
    // of tokens
    if amp == 0 {
        msg!("Input value of amplifier is zero, reduces to constant product curve case");
        return Err(error!(AmmError::InvalidArg));
    }

    // we proved that the invariant D value is bounded above by the sum of
    // tokens reserve amounts. For this reason, the value of D should be
    // able to be represented by a Decimal type, whenever each single token
    // reserve is also represented by Decimal (which should always be the case)
    StableCurveInvariant::new(amp, token_reserves_amount)?.compute()
}

struct StableCurveInvariant {
    // number of reserves
    exponent: u64,
    // initial guess for Newton's Method
    initial_guess: Decimal,
    // scale down exponent
    scl_down_coef: Decimal,
    // amplifier * n - 1
    first_order_coeff: Decimal,
    // amplifier * n * sum
    polynomial_third_term: Decimal,
}

impl StableCurveInvariant {
    fn new(amp: u64, token_reserves_amount: &[TokenAmount]) -> Result<Self> {
        let amp = Decimal::from(amp);

        let sum = token_reserves_amount
            .iter()
            .try_fold(Decimal::zero(), |acc, el| {
                acc.try_add(Decimal::from(el.amount))
            })?; // our initial guess for Newton's method

        let scl_down_sum = scale_down_value(sum)?;
        let initial_guess = scl_down_sum.scale_down;
        let scl_down_exp = scl_down_sum.exponent;

        let scl_down_coef =
            Decimal::from(1000_u64).try_pow(scl_down_exp as u64)?;

        let product = token_reserves_amount.iter().try_fold(
            Decimal::one(),
            |acc, el| {
                acc.try_mul(Decimal::from(el.amount).try_div(scl_down_coef)?)
            },
        )?;

        let exponent = token_reserves_amount.len() as u64;
        let base: Decimal = exponent.into();
        let n: Decimal = base.try_pow(exponent)?;
        let n_n_scaled_product = n.try_mul(product)?;
        let first_order_coeff = amp
            .try_mul(n)?
            .try_sub(Decimal::one())?
            .try_mul(n_n_scaled_product)?;
        let polynomial_third_term = amp
            .try_mul(n)?
            .try_mul(sum)?
            .try_mul(n_n_scaled_product)?
            .try_div(scl_down_coef)?;

        Ok(Self {
            first_order_coeff,
            exponent,
            initial_guess,
            scl_down_coef,
            polynomial_third_term,
        })
    }

    fn compute(self) -> Result<Decimal> {
        // acts as a threshold for the difference between successive
        // approximations
        let admissible_error: Decimal =
            Decimal::from(1u64).try_div(Decimal::from(2u64)).unwrap();

        // our initial guess is the scaled down sum of token reserve balances
        let mut prev_val = self.initial_guess;

        // current iteration of Newton-Raphson method
        let mut new_val = prev_val;

        for _ in 0..MAX_ITERATIONS {
            prev_val = new_val;
            new_val = self.newton_method_single_iteration(&prev_val)?;

            // We proved by algebraic manipulations that given a first initial
            // guess coinciding with the sum of token reserve
            // balances, then sum(x_i) >= positive_zero where
            // positive_zero is the positive zero of the stable swap
            // polynomial. Moreover, the method is decreasing on
            // each iteration. Therefore, in order to check that the method
            // converges, we only need to check that (prev_iter - next_iter) <=
            // adm_err. Given this assumption, it is impossible that prev_val <
            // new_val and the only case where equality holds is when
            // prev_val is a precise root of the polynomial.
            // Notice also that if x is a root of the stable polynomial,
            // applying Newton method to it will result in getting x again,
            // and the reciprocal statement holds true, so it is an equivalence.
            // Thus, the following checks are sufficient to guarantee
            // full logic coverage.
            if prev_val <= new_val {
                let is_val_root_stable_poly = self
                    .get_stable_swap_polynomial(&prev_val)?
                    == Decimal::zero();

                if is_val_root_stable_poly {
                    return prev_val.try_mul(self.scl_down_coef);
                } else {
                    // in this case, prev_val is not a root of the polynomial,
                    // and therefore having prev_val <=
                    // new_val would violate our
                    // mathematical assumptions
                    msg!(
                        "Invalid mathematical assumption: \
                        previous value cannot be less or equal to new value"
                    );
                    return Err(error!(AmmError::InvariantViolation));
                }
            }

            // assuming that prev_val >= new_val, we just need to check that
            // prev_val - new_val <= adm_error
            if prev_val.try_sub(new_val)? <= admissible_error {
                break;
            }
        }

        new_val.try_mul(self.scl_down_coef)
    }

    fn newton_method_single_iteration(
        &self,
        initial_guess: &Decimal,
    ) -> Result<Decimal> {
        let stable_swap_poly =
            self.get_stable_swap_polynomial(initial_guess)?;
        let derivative_stable_swap_poly =
            self.get_derivate_stable_swap_polynomial(initial_guess)?;

        initial_guess
            .try_sub(stable_swap_poly.try_div(derivative_stable_swap_poly)?)
    }

    // Stable swap polynomial to be found in README.md under AMM - Equations
    fn get_stable_swap_polynomial(&self, val: &Decimal) -> Result<Decimal> {
        // D^(n+1) + D(An^n -1)\prod_i x_i n^n + A(n^n)^2\sum_i x_i \prod_i x_i
        let first_term = val.try_pow(self.exponent + 1)?;

        let second_term = val.try_mul(self.first_order_coeff)?;

        first_term
            .try_add(second_term)?
            .try_sub(self.polynomial_third_term)
    }

    // Derivative of stable swap polynomial to be found in README.md under AMM -
    // Equations
    fn get_derivate_stable_swap_polynomial(
        &self,
        val: &Decimal,
    ) -> Result<Decimal> {
        let first_term = Decimal::from(self.exponent)
            .try_add(Decimal::one())?
            .try_mul(val.try_pow(self.exponent)?)?;
        let second_term = self.first_order_coeff;

        first_term.try_add(second_term)
    }
}

struct ScaleDownOutput {
    scale_down: Decimal,
    exponent: u32,
}

fn scale_down_value(mut val: Decimal) -> Result<ScaleDownOutput> {
    let mut n = 0u32;
    let bound = Decimal::from(1000u64);

    while val > bound {
        val = val.try_div(Decimal::from(1000u64))?;
        n += 1u32;
    }

    Ok(ScaleDownOutput {
        scale_down: val,
        exponent: n,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::*;

    #[test]
    fn fails_if_amplifier_is_zero() {
        let amp = 0u64;
        let token_reserves_amount: [TokenAmount; 2] =
            [100u64.into(), 10u64.into()];

        assert!(compute(amp, &token_reserves_amount)
            .unwrap_err()
            .to_string()
            .contains("InvalidArg"));
    }

    #[test]
    fn stable_swap_polynomial_fails_with_overflow() {
        let amp = 2u64;
        let token_reserves_amount: Vec<TokenAmount> =
            vec![2u64.into(), 2u64.into(), 2u64.into()];
        let state =
            StableCurveInvariant::new(amp, &token_reserves_amount).unwrap();

        let val: Decimal = 1u64.into();
        assert!(state.get_stable_swap_polynomial(&val).is_err());
    }

    #[test]
    fn derivate_stable_swap_polynomial_fails_with_overflow() {
        let amp = 2u64.into();
        let token_reserves_amount: Vec<TokenAmount> =
            vec![2u64.into(), 2u64.into(), 2u64.into()];
        let state =
            StableCurveInvariant::new(amp, &token_reserves_amount).unwrap();

        let val = Decimal::from_scaled_val(u128::MAX);
        assert!(state.get_derivate_stable_swap_polynomial(&val).is_err());
    }

    #[test]
    fn it_works_for_large_numbers_with_two_reserves() -> Result<()> {
        // since most stable coins have 6 decimal places, we need to take into
        // account that a product could be huge

        let amp = 10u64;

        for amount in [
            // $0.2
            0_100000u64,
            // $2
            1_000000u64,
            // $20
            10_000000u64,
            // $20k
            10_000_000000u64,
            // $20m
            10_000_000_000000u64,
            // $1bn
            500_000_000_000000u64,
            // $10bn
            10_000_000_000_000000u64,
        ] {
            match compute(amp, &vec![(amount).into(), (amount).into()]) {
                Ok(invariant) => {
                    assert_eq!(invariant, Decimal::from(amount * 2));
                }
                Err(e) => {
                    panic!(
                        "Stable curve invariant calc fails for \
                        amount of {} due to {}",
                        amount, e
                    );
                }
            };
        }

        Ok(())
    }

    #[test]
    fn it_works_for_large_numbers_with_three_reserves() {
        // since most stable coins have 6 decimal places, we need to take into
        // account that a product could be huge

        let amp = 10_u64;

        for amount in [
            // $0.2
            0_100000u64,
            // $2
            1_000000u64,
            // $20
            10_000000u64,
            // $2k
            1_000000u64,
            // $20k
            10_000_000000u64,
            // $20m// $0.2
            0_100000u64,
            // $2
            1_000000u64,
            // $20
            10_000000u64,
            // $2k
            1_000000u64,
            // $20k
            10_000_000000u64,
            // $20m
            10_000_000_000000u64,
            // $1bn
            500_000_000_000000u64,
            // $10bn
            10_000_000_000_000000u64,
        ] {
            match compute(
                amp,
                &vec![(amount).into(), (amount).into(), (amount).into()],
            ) {
                Ok(invariant) => {
                    assert_eq!(invariant, Decimal::from(amount * 3));
                }
                Err(e) => {
                    panic!(
                        "Stable curve invariant calc fails for \
                        amount of {} due to {}",
                        amount, e
                    );
                }
            }
        }
    }

    #[test]
    fn it_works_for_large_numbers_with_four_reserves() {
        // since most stable coins have 6 decimal places, we need to take into
        // account that a product could be huge

        let amp = 10_u64;

        for amount in [
            // $0.2
            0_100000u64,
            // $2
            1_000000u64,
            // $20
            10_000000u64,
            // $20k
            10_000_000000u64,
            // $20m
            10_000_000_000000u64,
            // $1bn
            500_000_000_000000u64,
            // $10bn
            10_000_000_000_000000u64,
            // $100bn
            100_000_000_000_000000u64,
        ] {
            match compute(
                amp,
                &vec![
                    (amount).into(),
                    (amount).into(),
                    (amount).into(),
                    (amount).into(),
                ],
            ) {
                Ok(invariant) => {
                    assert_eq!(invariant, Decimal::from(amount * 4));
                }
                Err(e) => {
                    panic!(
                        "Stable curve invariant calc fails for \
                        amount of {} due to {}",
                        amount, e
                    );
                }
            }
        }
    }

    #[test]
    fn stable_swap_polynomial_works() {
        let amp = 10u64;
        let token_reserves_amount: Vec<TokenAmount> =
            vec![(100u64).into(), (10u64).into()];
        let state =
            StableCurveInvariant::new(amp, &token_reserves_amount).unwrap();

        let val: Decimal = (110u64).into();
        let result = state.get_stable_swap_polynomial(&val).unwrap();

        assert_eq!(result, Decimal::from_scaled_val(891000000000000000000000));
    }

    #[test]
    fn derivate_stable_swap_polynomial_works() {
        let amp = 10u64;
        let token_reserves_amount: Vec<TokenAmount> =
            vec![(100u64).into(), (10u64).into()];
        let state =
            StableCurveInvariant::new(amp, &token_reserves_amount).unwrap();

        let val: Decimal = 110u64.into();
        let result = state.get_derivate_stable_swap_polynomial(&val).unwrap();

        assert_eq!(result, Decimal::from_scaled_val(192300000000000000000000));
    }

    #[test]
    fn stable_swap_polynomial_works_second() {
        let amp = 10u64;
        let token_reserves_amount: Vec<TokenAmount> =
            vec![(100u64).into(), (10u64).into(), (250u64).into()];
        let state =
            StableCurveInvariant::new(amp, &token_reserves_amount).unwrap();

        let val: Decimal = (360u64).into();
        let result = state.get_stable_swap_polynomial(&val).unwrap();

        assert_eq!(
            result,
            Decimal::from_scaled_val(14366160000000000000000000000)
        );
    }

    #[test]
    fn derivate_stable_swap_polynomial_works_second() {
        let amp = 10u64;
        let token_reserves_amount: Vec<TokenAmount> =
            vec![(100u64).into(), (10u64).into(), (250u64).into()];
        let state =
            StableCurveInvariant::new(amp, &token_reserves_amount).unwrap();

        let val: Decimal = (360u64).into();
        let result = state.get_derivate_stable_swap_polynomial(&val).unwrap();

        assert_eq!(
            result,
            Decimal::from_scaled_val(2002374000000000000000000000)
        );
    }

    #[test]
    fn newton_method_single_iteration_overflows() {
        let amp = 2u64.into();
        let token_reserves_amount: Vec<TokenAmount> =
            vec![2u64.into(), 2u64.into(), 2u64.into()];
        let state =
            StableCurveInvariant::new(amp, &token_reserves_amount).unwrap();

        let val: Decimal = u128::MAX.into();
        assert!(state.newton_method_single_iteration(&val).is_err());
    }

    #[test]
    fn newton_method_single_iteration_works() {
        let amp = 10u64;
        let token_reserves_amount: Vec<TokenAmount> =
            vec![(100u64).into(), (10u64).into()];
        let state =
            StableCurveInvariant::new(amp, &token_reserves_amount).unwrap();

        let val: Decimal = (110u64).into();
        let result = state.newton_method_single_iteration(&val).unwrap();

        assert_eq!(result, Decimal::from_scaled_val(105366614664586583464));
    }

    #[test]
    fn newton_method_single_iteration_works_second() {
        let amp = 10u64;
        let token_reserves_amount: Vec<TokenAmount> =
            vec![(100u64).into(), (10u64).into(), (250u64).into()];
        let state =
            StableCurveInvariant::new(amp, &token_reserves_amount).unwrap();

        let val: Decimal = (360u64).into();
        let result = state.newton_method_single_iteration(&val).unwrap();

        assert_eq!(result, Decimal::from_scaled_val(352825436207222027454));
    }

    #[test]
    fn newton_method_overflows() {
        let amp = u64::MAX.into();

        let token_reserves_amount: Vec<TokenAmount> =
            vec![u64::MAX.into(), u64::MAX.into(), 2u64.into()];

        assert!(compute(amp, &token_reserves_amount,).is_err());
    }

    #[test]
    fn newton_method_works() {
        let amp = 10u64;

        let token_reserves_amount: Vec<TokenAmount> =
            vec![(100u64).into(), (10u64).into()];

        let result = compute(amp, &token_reserves_amount).unwrap();

        assert_eq!(result, Decimal::from_scaled_val(105329716513966933807));
    }

    #[test]
    fn newton_method_works_second() {
        let amp = 10u64;

        let token_reserves_amount: Vec<TokenAmount> =
            vec![(100u64).into(), (10u64).into(), (250u64).into()];

        let result = compute(amp, &token_reserves_amount).unwrap();

        assert_eq!(result, Decimal::from_scaled_val(352805602632122973013));
    }

    proptest! {
        #[test]
        fn successfully_computes_invariant_with_two_reserves(
            amp in 2..200u64,
            first_reserve_amount in 1..10_000_000_000_000_000u64,
            second_reserve_amount in 1..10_000_000_000_000_000u64,
        ) {
            let token_reserves_amount = vec![
                TokenAmount::new(first_reserve_amount),
                TokenAmount::new(second_reserve_amount),
            ];

            assert!(compute(amp, &token_reserves_amount).is_ok());
        }

        #[test]
        fn successfully_computes_invariant_with_three_reserves(
            amp in 2..200u64,
            first_reserve_amount in 1..10_000_000_000_000_000u64,
            second_reserve_amount in 1..10_000_000_000_000_000u64,
            third_reserve_amount in 1..10_000_000_000_000_000u64
        ) {
            let token_reserves_amount = vec![
                TokenAmount::new(first_reserve_amount),
                TokenAmount::new(second_reserve_amount),
                TokenAmount::new(third_reserve_amount),
            ];

            assert!(compute(amp, &token_reserves_amount).is_ok());
        }

        #[test]
        fn successfully_computes_invariant_with_four_reserves(
            amp in 2..200u64,
            first_reserve_amount in 1..10_000_000_000_000_000u64,
            second_reserve_amount in 1..10_000_000_000_000_000u64,
            third_reserve_amount in 1..10_000_000_000_000_000u64,
            forth_reserve_amount in 1..10_000_000_000_000_000u64
        ) {
            let token_reserves_amount = vec![
                TokenAmount::new(first_reserve_amount),
                TokenAmount::new(second_reserve_amount),
                TokenAmount::new(third_reserve_amount),
                TokenAmount::new(forth_reserve_amount),
            ];

            assert!(compute(amp, &token_reserves_amount).is_ok());
        }
    }

    #[test]
    fn regression_test_1() {
        let token_reserves_amount = vec![
            TokenAmount::new(323937059261502),
            TokenAmount::new(307818470989694),
            TokenAmount::new(409053424216126),
        ];

        compute(36, &token_reserves_amount).unwrap();
    }

    #[test]
    fn regression_test_2() {
        let token_reserves_amount = vec![
            TokenAmount::new(323937059261502),
            TokenAmount::new(307818470989694),
            TokenAmount::new(362813707275663),
        ];

        compute(36, &token_reserves_amount).unwrap();
    }

    #[test]
    fn regression_test_3() {
        let amp = 2u64;
        let first_reserve_amount = 6801827978;
        let second_reserve_amount = 670789431u64;
        let third_reserve_amount = 2631887715u64;

        let token_reserves_amount = vec![
            TokenAmount::new(first_reserve_amount),
            TokenAmount::new(second_reserve_amount),
            TokenAmount::new(third_reserve_amount),
        ];

        assert!(compute(amp, &token_reserves_amount).is_ok());
    }
}
