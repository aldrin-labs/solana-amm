use crate::prelude::*;
use helpers::scale_down_value;
use std::collections::BTreeMap;

/// Consider an LP, with two token reserves A and B and indexed
/// by a stable swap curve. Name the amounts of tokens A and B
/// in the LP, by x and y, respectively. The following method
/// computes the total amount of new tokens B in the LP, after
/// swapping a certain value of delta(x) tokens of token A.
/// In order to do so, we need to solve a quadratic equation,
/// in the case of a stable swap curve indexing prices in the LP.
///
/// inputs:
///     num_reserves - the number of tokens in the LP
///     amp          - amplifier of the stable swap curve
///     d            - invariant of the stable swap curve
///     sum          - sum of all token reserves, except
///         for the quote token being swapped
///     product      - product of all token reserves, except
///         for the quote token being swapped
///
/// output:
///     Total value of new quote tokens in the LP after swap
pub fn get_buy_reserve_balance_after_swap(
    num_reserves: u64,
    amp: &Decimal,
    d: &Decimal,
    sum: Decimal,
    product: Decimal,
) -> Result<Decimal> {
    // the linear term of the quadratic equation is
    // A n^n sum_{i != k} x_i - D(n^n A - 1)

    // since we are dealing with Decimal types,
    // which rely on U320, we split the computation
    // of the linear term as a first term
    // A n^n sum_{i != k} x_i
    // and a second term
    // D(n^n A - 1)
    let scale_down_out = scale_down_value(*d)?;
    let d = scale_down_out.scale_down;
    let exp = scale_down_out.exponent;

    let product = product
        .try_div(Decimal::from(1000u64.pow(exp)).try_pow(num_reserves - 1)?)?;
    let sum = sum.try_div(Decimal::from(1000u64.pow(exp)))?;

    let n_pow_n = Decimal::from(num_reserves).try_pow(num_reserves)?;

    // D(n^n A - 1)
    let linear_first_term =
        d.try_mul(n_pow_n.try_mul(*amp)?.try_sub(Decimal::one())?)?;

    // A n^n sum_{i != k} x_i
    let linear_second_term = amp.try_mul(n_pow_n)?.try_mul(sum)?;

    // b^2 = [A n^n sum_{i != k} x_i - D(n^n A - 1)]^2
    // since we take a square power, we can compute the absolute value
    // of the base and take its square
    let mut is_symmetric = false;
    let b = match linear_second_term.try_sub(linear_first_term) {
        // Math overflow error, due to the existence of a negative value
        Err(_) => {
            is_symmetric = true;
            linear_first_term.try_sub(linear_second_term)?
        }
        Ok(val) => val,
    };

    // get the value of constant term = D^(n+1) / n^n prod_{i != k} x_i
    let constant_term = d
        .try_pow(num_reserves + 1)?
        .try_div(n_pow_n.try_mul(product)?)?;

    let quadratic_term = amp.try_mul(n_pow_n)?;

    // sqrt(b^2 - 4ac) / 2a
    // notice that constant_term = -c, therefore, we get
    // sqrt(quadratic_term + 4 * quadratic_term * constant_term)
    let sqrt_discriminator = b
        .try_pow(2)?
        .try_add(
            Decimal::from(4_u64)
                .try_mul(quadratic_term)?
                .try_mul(constant_term)?,
        )?
        .try_sqrt()?;

    // finally, the root of the polynomial is given by
    // (sqrt(b^2 - 4ac) - b) / 2a
    // this value should always be positive, because
    // c = - c', where c' is positive and a is also positive
    // sqrt(b^2 + 4ac') / 2a > sqrt(b^2) / 2a = b / 2a
    // thus, (sqrt(b^2 + 4ac') - b) / 2a > 0
    let two_a = Decimal::from(2_u64).try_mul(quadratic_term)?;

    if is_symmetric {
        sqrt_discriminator
            .try_add(b)?
            .try_div(two_a)?
            .try_mul(Decimal::from(1_000u64).try_pow(exp as u64)?)
    } else {
        match sqrt_discriminator.try_sub(b) {
            Err(_) => {
                msg!(
                    "Bought token balance after swapped cannot be negative,
                    invalid inputs"
                );
                Err(error!(AmmError::InvariantViolation))
            }
            Ok(val) => val
                .try_div(two_a)?
                .try_mul(Decimal::from(1_000u64).try_pow(exp as u64)?),
        }
    }
}

/// this function computes the amount of base
/// token being bought by the user, after a
/// swap.
pub fn compute_delta_withdraw_token_amount(
    quote_token_balance_after_swap: Decimal,
    tokens_reserves: BTreeMap<Pubkey, TokenAmount>,
    quote_token_mint: Pubkey,
) -> Result<Decimal> {
    Decimal::from(tokens_reserves.get(&quote_token_mint).unwrap().amount)
        .try_sub(quote_token_balance_after_swap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::*;

    #[test]
    fn it_computes_delta_quote_token_amount() {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();

        let tokens_reserves = [
            (mint1, TokenAmount::new(100)),
            (mint2, TokenAmount::new(10)),
        ]
        .into_iter()
        .collect::<BTreeMap<Pubkey, TokenAmount>>();

        let quote_token_deposit_amount_after_swap = Decimal::from(5_u64);
        let quote_token_mint = mint2;

        assert_eq!(
            compute_delta_withdraw_token_amount(
                quote_token_deposit_amount_after_swap,
                tokens_reserves,
                quote_token_mint
            )
            .unwrap(),
            Decimal::from(5_u64)
        )
    }

    #[test]
    fn it_gets_buy_reserve_balance_after_swap() {
        // python code for additional check:
        //
        // import numpy as np
        //
        // # initial deposit 100 - 10
        // # swap 50 for y ?
        // # therefore sum = 50 and product = 50
        //
        // D = 105.329717
        // b = -(D * (4 * 10 - 1) - (10 * 4 * 50))
        // c = - ((D**3) / (4 * 50))
        // a = 10 * 4
        //
        // roots = np.roots([a, b, c])

        let num_reserves = 2;
        let amp = Decimal::from(10_u64);
        let d = Decimal::from_scaled_val(105329717000000000000);
        let sum = Decimal::from(50_u64);
        let product = Decimal::from(50_u64);

        let root = get_buy_reserve_balance_after_swap(
            num_reserves,
            &amp,
            &d,
            sum,
            product,
        )
        .unwrap();

        assert_eq!(root, Decimal::from_scaled_val(55336168643134277756));
    }

    #[test]
    fn it_computes_root_larger_than_zero() -> Result<()> {
        let x1 = 10u64;
        let x2 = 9928061103u64;

        let num_reserves = 3u64;
        let amp = Decimal::from(2u64);
        let sum = Decimal::from(x1).try_add(Decimal::from(x2))?;
        let d = sum.try_mul(78)?;
        let product = Decimal::from(x1).try_mul(Decimal::from(x2))?;

        let root = get_buy_reserve_balance_after_swap(
            num_reserves,
            &amp,
            &d,
            sum,
            product,
        )?;

        assert_ne!(root, Decimal::zero());

        Ok(())
    }

    proptest! {
        #[test]
        fn computes_positive_root_quadratic_polynomial_passes(
            amp in 2..200u64,
            x1 in 10..10_000_000_000_000_u64,
            x2 in 10..10_000_000_000_000_000u64,
            num in 1..1_00u64, // to generate more realistic inv d
            den in 1..1_00u64, // to generate more realistic inv d
        ) {
            let amp = Decimal::from(amp);
            let sum = Decimal::from(x1).try_add(Decimal::from(x2))?;
            let d = sum.try_mul(num)?.try_div(den)?;
            let product = Decimal::from(x1).try_mul(Decimal::from(x2))?;

            let root = get_buy_reserve_balance_after_swap(
                3u64,
                &amp,
                &d,
                sum,
                product
            );

            match root {
                // case where root is negative, that is negative bought token
                // balance, which should not be allowed
                Err(e) => assert!(e.to_string().contains("InvariantViolation")),
                // in success case, root should be positive
                Ok(val) => assert!(val > Decimal::zero()),
            }
        }

        /// Unfortunately, our computation of the token balances of the swap
        /// don't make the swap polynomial == 0, so we need to make sure
        /// our protocol is decimal precision resistant. We have to guarantee
        /// that our computed root has the same floor as the true root. This
        /// proptest gives us the guarantees that this is indeed the case
        ///
        /// P(x) = Ax^2 + Bx + C = 0
        /// Rust code gives us an approximation x'
        /// so that
        /// A(x')^2 + Bx' + C = epsilon (epsilon > 0)
        ///
        /// Goal: since we are only interested in floor(x') and floor(x) (x' is app and x is true root)
        ///
        /// let y = floor(x')
        ///
        /// and evaluate
        ///
        /// P(y) = t
        ///
        /// if t > 0 then we are in trouble
        ///
        /// otherwise, we are good to go
        ///
        ///
        /// P(x) is increasing near x
        ///
        /// Problem is when floor(x') != floor(x)
        ///
        /// if floor(x) < floor(x')
        ///
        /// x < floor(x')
        /// <=>
        /// 0 = P(x) < P(floor(x')) < P(x')
        ///
        ///
        ///
        #[test]
        fn computes_correct_floor_value(
            amp in 2..200u64,
            x1 in 10..10_000_000_000_000_000u64,
            x2 in 10..10_000_000_000_000_000u64,
            num in 1..1_00u64, // to generate more realistic inv d
            den in 1..1_00u64, // to generate more realistic inv d
        ) {
            let num_reserves = 3u64;
            let amp = Decimal::from(amp);
            let sum = Decimal::from(x1).try_add(Decimal::from(x2))?;
            let d = sum.try_mul(num)?.try_div(den)?;
            let product = Decimal::from(x1).try_mul(Decimal::from(x2))?;

            let scale_down_out = scale_down_value(d)?;
            let d = scale_down_out.scale_down;
            let exp = scale_down_out.exponent;

            let product = product
                .try_div(Decimal::from(1000u64.pow(exp)).try_pow(num_reserves - 1)?)?;
            let sum = sum.try_div(Decimal::from(1000u64.pow(exp)))?;


            let root = get_buy_reserve_balance_after_swap(
                num_reserves,
                &amp,
                &d,
                sum,
                product
            ).unwrap();

            let root: Decimal = root.try_floor()?.into();

            let n_pow_n = Decimal::from(num_reserves).try_pow(num_reserves)?;
            let a = amp.try_mul(n_pow_n)?.try_mul(root.try_pow(2u64)?)?;

            let linear_first_term =
                d.try_mul(n_pow_n.try_mul(amp)?.try_sub(Decimal::one())?)?.try_mul(root)?;

            // A n^n sum_{i != k} x_i
            let linear_second_term = amp.try_mul(n_pow_n)?.try_mul(sum)?.try_mul(root)?;

            // b^2 = [A n^n sum_{i != k} x_i - D(n^n A - 1)]^2
            // since we take a square power, we can compute the absolute value
            // of the base and take its square
            let mut is_symmetric = false;
            let b = match linear_first_term.try_sub(linear_second_term) {
                // Math overflow error, due to the existence of a negative value
                Err(_) => {
                    is_symmetric = true;
                    linear_second_term.try_sub(linear_first_term)?
                }
                Ok(val) => val,
            };

            let c = d
                .try_pow(num_reserves + 1)?
                .try_div(n_pow_n.try_mul(product)?)?;

            // assert we are in the increasing part of the parabola,
            // by analysing the derivative
            if !is_symmetric {
                if root == Decimal::zero() {
                    assert_eq!(b, Decimal::zero());
                } else {
                    // assert!(Decimal::from(2u64).try_mul(a)?.try_div(root)? >= b);
                    assert!(
                        b.try_mul(root)?.try_div(a)? >= Decimal::one()
                        || Decimal::from(2u64).try_mul(a)?.try_div(root)? >= b
                    )
                }
            }

            if is_symmetric {
                if c >= b {
                    assert!(a < c.try_sub(b)?);
                } else {
                    // we should never arrive here
                    assert!(false);
                }
            } else {
                assert!(a < c.try_add(b)?);
            }
        }
    }
}
