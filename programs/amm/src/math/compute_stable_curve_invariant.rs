//! Implements Newton-Raphson method for numerical approximation of
//! differentiable function zeroes.
//!
//! https://en.wikipedia.org/wiki/Newton%27s_method

use crate::prelude::*;

// The method should converge within few iterations, due to the fact
// we are approximating positive root from a well positioned first
// initial guess
const MAX_ITERATIONS: usize = 255;

pub fn compute_stable_curve_invariant(
    amp: &Decimal,
    token_reserves_amount: &[Decimal],
) -> Result<Decimal> {
    // acts as a threshold for the difference between successive approximations
    let admissible_error: Decimal =
        Decimal::from(1_u64).try_div(Decimal::from(2_u64)).unwrap();

    // if amplifier is zero, then the invariant of the curve is just the product
    // of tokens
    if *amp <= Decimal::zero() {
        msg!("Input value of amplifier is zero, reduces to constant product curve case");
        return Err(error!(AmmError::InvalidArg));
    }

    // our initial guess is the sum of token reserve balances
    let mut prev_val: Decimal = fold_sum(token_reserves_amount)?;

    // current iteration of Newton-Raphson method
    let mut new_val = prev_val;

    for _ in 0..MAX_ITERATIONS {
        prev_val = new_val;
        new_val = newton_method_single_iteration(
            amp,
            token_reserves_amount,
            &prev_val,
        )?;

        let is_val_root_stable_poly =
            get_stable_swap_polynomial(amp, token_reserves_amount, &prev_val)?
                == Decimal::zero();

        // We proved by algebraic manipulations that given a first initial guess
        // coinciding with the sum of token reserve balances, then sum(x_i) >=
        // positive_zero where positive_zero is the positive zero of the
        // stable swap polynomial. Moreover, the method is decreasing on
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
        if prev_val <= new_val && is_val_root_stable_poly {
            return Ok(prev_val);
        } else if prev_val <= new_val {
            // in this case, prev_val is not a root of the polynomial, and
            // therefore having prev_val <= new_val would violate our
            // mathematical assumptions
            msg!(
                "Invalid mathematical assumption: previous value cannot be \
                less or equal to new value"
            );
            return Err(error!(AmmError::InvariantViolation));
        }

        // assuming that prev_val >= new_val, we just need to check that
        // prev_val - new_val <= adm_error
        if (prev_val.try_sub(new_val)?) <= admissible_error {
            break;
        }
    }

    Ok(new_val)
}

// Stable swap polynomial to be found in README.md under AMM - Equations
fn get_stable_swap_polynomial(
    amp: &Decimal,
    token_reserves_amount: &[Decimal],
    val: &Decimal,
) -> Result<Decimal> {
    let exponent = token_reserves_amount.len() as u64;
    let base: Decimal = exponent.into();

    let n: Decimal = base.try_pow(exponent)?;

    let product_reserves = fold_product(token_reserves_amount)?;

    let sum_reserves = fold_sum(token_reserves_amount)?;
    let first_term = val
        .try_pow(exponent + 1)?
        .try_div(n.try_mul(product_reserves)?)?;
    let second_term = val.try_mul(amp.try_mul(n)?.try_sub(Decimal::one())?)?;
    let third_term = amp.try_mul(n)?.try_mul(sum_reserves)?;

    let result = first_term.try_add(second_term)?.try_sub(third_term)?;

    Ok(result)
}

// Derivative of stable swap polynomial to be found in README.md under AMM -
// Equations
fn get_derivate_stable_swap_polynomial(
    amp: &Decimal,
    token_reserves_amount: &[Decimal],
    val: &Decimal,
) -> Result<Decimal> {
    let exponent = token_reserves_amount.len() as u64;
    let base: Decimal = exponent.into();

    let n: Decimal = base.try_pow(exponent)?;

    let product_reserves = fold_product(token_reserves_amount)?;

    // from now on, this fn differs from the get_stable_swap_polynomial

    let first_term = base
        .try_add(Decimal::one())?
        .try_mul(val.try_pow(exponent)?)?
        .try_div(n.try_mul(product_reserves)?)?;
    let second_term = amp.try_mul(n)?.try_sub(Decimal::one())?;
    let result = first_term.try_add(second_term)?;

    Ok(result)
}

fn newton_method_single_iteration(
    amp: &Decimal,
    token_reserves_amount: &[Decimal],
    initial_guess: &Decimal,
) -> Result<Decimal> {
    let stable_swap_poly =
        get_stable_swap_polynomial(amp, token_reserves_amount, initial_guess)?;
    let derivative_stable_swap_poly = get_derivate_stable_swap_polynomial(
        amp,
        token_reserves_amount,
        initial_guess,
    )?;

    initial_guess
        .try_sub(stable_swap_poly.try_div(derivative_stable_swap_poly)?)
}

fn fold_product(values: &[Decimal]) -> Result<Decimal> {
    let result = values
        .iter()
        .try_fold(Decimal::one(), |acc, el| acc.try_mul(*el))?;

    Ok(result)
}

fn fold_sum(values: &[Decimal]) -> Result<Decimal> {
    let result = values
        .iter()
        .try_fold(Decimal::zero(), |acc, el| acc.try_add(*el))?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fails_if_amplifier_is_zero() {
        let amp = Decimal::from(0_u64);

        let token_reserves_amount: [Decimal; 2] =
            [100_u64.into(), 10_u64.into()];

        assert!(compute_stable_curve_invariant(&amp, &token_reserves_amount)
            .unwrap_err()
            .to_string()
            .contains("InvalidArg"));
    }

    #[test]
    fn stable_swap_polynomial_fails_with_overflow() {
        let amp: Decimal = u64::MAX.into();

        let token_reserves_amount: Vec<Decimal> =
            vec![u64::MAX.into(), u64::MAX.into(), (2 as u64).into()];

        let val: Decimal = (1 as u64).into();

        assert!(
            get_stable_swap_polynomial(&amp, &token_reserves_amount, &val)
                .is_err()
        );
    }

    #[test]
    fn derivate_stable_swap_polynomial_fails_with_overflow() {
        let amp: Decimal = u64::MAX.into();

        let token_reserves_amount: Vec<Decimal> =
            vec![u64::MAX.into(), u64::MAX.into(), (2 as u64).into()];

        let val: Decimal = u64::MAX.into();

        assert!(get_derivate_stable_swap_polynomial(
            &amp,
            &token_reserves_amount,
            &val
        )
        .is_err());
    }

    #[test]
    fn stable_swap_polynomial_works() {
        let amp: Decimal = (10 as u64).into();

        let token_reserves_amount: Vec<Decimal> =
            vec![(100 as u64).into(), (10 as u64).into()];

        let val: Decimal = (110 as u64).into();

        let result =
            get_stable_swap_polynomial(&amp, &token_reserves_amount, &val)
                .unwrap();

        assert_eq!(
            result,
            Decimal::from(222 as u64)
                .try_add(
                    Decimal::from(3 as u64)
                        .try_div(Decimal::from(2 as u64))
                        .unwrap()
                        .try_div(Decimal::from(2 as u64))
                        .unwrap()
                )
                .unwrap()
        );
    }

    #[test]
    fn derivate_stable_swap_polynomial_works() {
        let amp: Decimal = (10 as u64).into();

        let token_reserves_amount: Vec<Decimal> =
            vec![(100 as u64).into(), (10 as u64).into()];

        let val: Decimal = (110 as u64).into();

        let result = get_derivate_stable_swap_polynomial(
            &amp,
            &token_reserves_amount,
            &val,
        )
        .unwrap();

        assert_eq!(
            result,
            Decimal::from(48 as u64)
                .try_add(
                    Decimal::from(3 as u64)
                        .try_div(Decimal::from(2 as u64))
                        .unwrap()
                        .try_div(Decimal::from(2 as u64))
                        .unwrap()
                        .try_div(Decimal::from(10 as u64))
                        .unwrap()
                )
                .unwrap()
        );
    }

    #[test]
    fn stable_swap_polynomial_works_second() {
        let amp: Decimal = (10 as u64).into();

        let token_reserves_amount: Vec<Decimal> =
            vec![(100 as u64).into(), (10 as u64).into(), (250 as u64).into()];

        let val: Decimal = (360 as u64).into();

        let result =
            get_stable_swap_polynomial(&amp, &token_reserves_amount, &val)
                .unwrap();

        assert_eq!(result, Decimal::from_scaled_val(2128320000000000000000));
    }

    #[test]
    fn derivate_stable_swap_polynomial_works_second() {
        let amp: Decimal = (10 as u64).into();

        let token_reserves_amount: Vec<Decimal> =
            vec![(100 as u64).into(), (10 as u64).into(), (250 as u64).into()];

        let val: Decimal = (360 as u64).into();

        let result = get_derivate_stable_swap_polynomial(
            &amp,
            &token_reserves_amount,
            &val,
        )
        .unwrap();

        assert_eq!(result, Decimal::from_scaled_val(296648000000000000000));
    }

    #[test]
    fn newton_method_single_iteration_overflows() {
        let amp: Decimal = u64::MAX.into();

        let token_reserves_amount: Vec<Decimal> =
            vec![u64::MAX.into(), u64::MAX.into(), (2 as u64).into()];

        let val: Decimal = u64::MAX.into();

        assert!(newton_method_single_iteration(
            &amp,
            &token_reserves_amount,
            &val
        )
        .is_err());
    }

    #[test]
    fn newton_method_single_iteration_works() {
        let amp: Decimal = (10 as u64).into();

        let token_reserves_amount: Vec<Decimal> =
            vec![(100 as u64).into(), (10 as u64).into()];

        let val: Decimal = (110 as u64).into();

        let result =
            newton_method_single_iteration(&amp, &token_reserves_amount, &val)
                .unwrap();

        assert_eq!(result, Decimal::from_scaled_val(105366614664586583464));
    }

    #[test]
    fn newton_method_single_iteration_works_second() {
        let amp: Decimal = (10 as u64).into();

        let token_reserves_amount: Vec<Decimal> =
            vec![(100 as u64).into(), (10 as u64).into(), (250 as u64).into()];

        let val: Decimal = (360 as u64).into();

        let result =
            newton_method_single_iteration(&amp, &token_reserves_amount, &val)
                .unwrap();

        assert_eq!(result, Decimal::from_scaled_val(352825436207222027454));
    }

    #[test]
    fn newton_method_overflows() {
        let amp: Decimal = u64::MAX.into();

        let token_reserves_amount: Vec<Decimal> =
            vec![u64::MAX.into(), u64::MAX.into(), (2 as u64).into()];

        assert!(
            compute_stable_curve_invariant(&amp, &token_reserves_amount,)
                .is_err()
        );
    }

    #[test]
    fn newton_method_works() {
        let amp: Decimal = (10 as u64).into();

        let token_reserves_amount: Vec<Decimal> =
            vec![(100 as u64).into(), (10 as u64).into()];

        let result =
            compute_stable_curve_invariant(&amp, &token_reserves_amount)
                .unwrap();

        assert_eq!(result, Decimal::from_scaled_val(105329716513966933807));
    }

    #[test]
    fn newton_method_works_second() {
        let amp: Decimal = (10 as u64).into();

        let token_reserves_amount: Vec<Decimal> =
            vec![(100 as u64).into(), (10 as u64).into(), (250 as u64).into()];

        let result =
            compute_stable_curve_invariant(&amp, &token_reserves_amount)
                .unwrap();

        assert_eq!(result, Decimal::from_scaled_val(352805602632122973013),);
    }
}
