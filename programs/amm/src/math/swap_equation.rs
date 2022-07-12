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
pub fn compute_positive_root_quadratic_polynomial(
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
        .try_div(n_pow_n.try_mul(product)?)?; //todo: should be sum

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
        sqrt_discriminator
            .try_sub(b)?
            .try_div(two_a)?
            .try_mul(Decimal::from(1_000u64).try_pow(exp as u64)?)
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

    #[test]
    fn works_compute_delta_quote_token_amount() {
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
    fn works_compute_positive_root_quadratic_polynomial() {
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

        let root = compute_positive_root_quadratic_polynomial(
            num_reserves,
            &amp,
            &d,
            sum,
            product,
        )
        .unwrap();

        assert_eq!(root, Decimal::from_scaled_val(55336168643134277756));
    }
}
