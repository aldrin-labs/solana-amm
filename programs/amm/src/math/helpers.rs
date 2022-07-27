use crate::prelude::*;

pub fn fold_product(values: &[Decimal]) -> Result<Decimal> {
    let result = values
        .iter()
        .try_fold(Decimal::one(), |acc, el| acc.try_mul(*el))?;

    Ok(result)
}

pub fn fold_sum(values: &[Decimal]) -> Result<Decimal> {
    let result = values
        .iter()
        .try_fold(Decimal::zero(), |acc, el| acc.try_add(*el))?;

    Ok(result)
}

pub struct ScaleDownOutput {
    pub scale_down: Decimal,
    pub exponent: u32,
}

pub fn scale_down_value(mut val: Decimal) -> Result<ScaleDownOutput> {
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

/// This function receives a number in Decimal type form and will return the
/// exponent of the number. We find the exponent using a naive method of
/// counting the number of digits in the Decimal type.
///
/// Input `num` in scientific notation follows: num = x . 10^exponent
///
/// The exponent of an integer number can be naively obtained by counting its
/// digits and subtracting one. We can do this since Decimal type can be though
/// of as very big integer number that represents a smaller number that might
/// not be integer at all.
///
/// In Decimal type the number 7 will be represented as
/// 7_000_000_000_000_000_000. This means that positive orders of magnitude will
/// start at 19 and everything below that will have a negative order of
/// magnitude.
///
/// Since we don't want to return negative values, Decimal type values
/// representing numbers between 0 >= num > 1 will invert the computation. In
/// this cases we will flag the exponent returned as being a negative exponent.
///
/// This function returns a tuple Result of (u64, bool) representing the
/// exponent and its sign (positive if true, negative if false)
pub fn base_two_exponent(num: Decimal) -> u32 {
    let Decimal(decimal::U192([lowest, middle, upper])) = num;

    let leading_zeroes = if upper == 0 {
        if middle == 0 {
            lowest.leading_zeros().min(4) + u64::BITS + u64::BITS
        } else {
            middle.leading_zeros() + u64::BITS
        }
    } else {
        upper.leading_zeros()
    };

    // ~2^60 is reserved for decimals (10^18 precisely)
    let max_exponent = 3 * 64 - 60;
    let exponent_in_binary = max_exponent - leading_zeroes;

    exponent_in_binary
}

/// Given the equation a . b / c, we are performing the computation
/// using a computation path that minimizes risk of overflow. There are three
/// computation paths we can follow:
///
/// a * b -> / c
/// a / c -> * b
/// b / c -> * a
///
/// Naturally we prefer to perform the computations that increase the order
/// of magnitude because the Decimal type has a ceiling of 10^39 on the upper
/// bound, whilst only having  a floor of 10^-18 on the lower bound. We only
/// do not favour such computations first if the result exponent is bigger than
/// 38 (we give 1 order of magnitude slack for safety). In such case we will
/// try to reduce the exponent before the multiplication by diving the biggest
/// numerator (a or b) by c, as long as c is bigger than one, since this will
/// decrease the orders of magnitude prior to the multiplication and therefore
/// decrease the risk of overflow.
pub fn try_mul_div(a: Decimal, b: Decimal, c: Decimal) -> Result<Decimal> {
    // In case c is less than one, the division will always increase
    // the number computed therefore we just follow normally. There is
    // risk of overflow but we cannot do anything to mitigate that risk.
    if c < Decimal::one() {
        return a.try_mul(b)?.try_div(c);
    }

    msg!("before try_mul_div base_two_exponent");
    anchor_lang::solana_program::log::sol_log_compute_units();
    let a_exponent = base_two_exponent(a);
    let b_exponent = base_two_exponent(b);
    msg!("after try_mul_div base_two_exponent");
    anchor_lang::solana_program::log::sol_log_compute_units();

    let res = if a_exponent + b_exponent >= 130 {
        // This means that multiplying a and b will lead to a very high number,
        // potentially bigger than 1*10^39 and therefore to decrease risk of
        // overflow we divide first the highest numerator by c to decrease the
        // exponent
        if a_exponent >= b_exponent {
            // In this case a is bigger than or equal to b, so we will
            // divide a by c before multiplying it by b
            a.try_div(c)?.try_mul(b)
        } else {
            // In this case a is smaller than b, so we will divide
            // b by c before multiplying it with a
            b.try_div(c)?.try_mul(a)
        }
    } else {
        // This meanst that it is safe to multiply a and b because it will
        // never be bigger than 1*10^39 and therefore it should not overflow
        a.try_mul(b)?.try_div(c)
    };

    msg!("after try_mul_div branches");
    anchor_lang::solana_program::log::sol_log_compute_units();

    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn it_computes_try_mul_div() -> Result<()> {
        let a = Decimal::from(7_u64);
        let b = Decimal::from(9_u64);
        let c = Decimal::from(5_u64);
        let ab_c = try_mul_div(a, b, c)?;

        // We use scaled value when we want to have precision
        // on the lower bound
        assert_eq!(
            ab_c,
            Decimal::from_scaled_val(12_600_000_000_000_000_000_u128)
        );

        let a = Decimal::from(18_446_744_073_709_500_000_u64);
        let b = Decimal::from(9_u64);
        let c = Decimal::from(5_u64);
        let ab_c = try_mul_div(a, b, c)?;

        // We use scaled value when we want to have precision
        // on the lower bound
        assert_eq!(
            ab_c,
            Decimal::from_scaled_val(
                33_204_139_332_677_100_000_000_000_000_000_000_000_u128
            )
        );

        let a = Decimal::from(9_u64);
        let b = Decimal::from(18_446_744_073_709_500_000_u64);
        let c = Decimal::from(5_u64);
        let ab_c = try_mul_div(a, b, c)?;

        // We use scaled value when we want to have precision
        // on the lower bound
        assert_eq!(
            ab_c,
            Decimal::from_scaled_val(
                33_204_139_332_677_100_000_000_000_000_000_000_000_u128
            )
        );

        let a = Decimal::from(7_u64);
        let b = Decimal::from(9_u64);
        let c = Decimal::from(18_446_744_073_709_500_000_u64);
        let ab_c = try_mul_div(a, b, c)?;

        // We use scaled value when we want to have precision
        // on the lower bound
        assert_eq!(ab_c, Decimal::from_scaled_val(3));

        let a = Decimal::from(18_446_744_073_709_551_615_u64);
        let b = Decimal::from(18_446_744_073_709_551_615_u64);
        let c = Decimal::from(1_u64);
        let ab_c = try_mul_div(a, b, c)?;

        // Here we do not used scaled value because we are testing
        // the upper bound
        assert_eq!(
            ab_c,
            Decimal::from(
                340_282_366_920_938_463_426_481_119_284_349_108_225_u128
            )
        );

        let a = Decimal::from(18_446_744_073_709_551_615_u64);
        let b = Decimal::from(18_446_744_073_709_551_615_u64);
        let c = Decimal::from(18_446_744_073_709_551_615_u64);
        let ab_c = try_mul_div(a, b, c)?;

        // Here we do not used scaled value because we are testing
        // the upper bound
        assert_eq!(ab_c, Decimal::from(18_446_744_073_709_551_615_u64));

        Ok(())
    }

    #[test]
    fn it_finds_exponent() -> Result<()> {
        let num = Decimal::from(1_u64);
        let exponent = base_two_exponent(num);

        assert_eq!(exponent, 0);

        let num = Decimal::from(10_u64);
        let exponent = base_two_exponent(num);

        // assert_eq!(exponent, 1);

        let num = Decimal::from(100_u64);
        let exponent = base_two_exponent(num);

        // assert_eq!(exponent, 2);

        let num = Decimal::from(1_000u128);
        let exponent = base_two_exponent(num);

        // assert_eq!(exponent, 3);

        let num = Decimal::from(100_000_000_u64);
        let exponent = base_two_exponent(num);

        // assert_eq!(exponent, 8);

        let num = Decimal::from(18_446_744_073_709_551_615_u64);
        let exponent = base_two_exponent(num);

        // assert_eq!(exponent, 19);

        // Testing on small numbers
        // 1_000_000_000_000_000_000_u128 => 1
        let num = Decimal::from_scaled_val(1_000_000_000_000_000_000_u128);
        let exponent = base_two_exponent(num);

        // assert_eq!(exponent, 0);

        // 100_000_000_000_000_000_u128 => 0.1
        let num = Decimal::from_scaled_val(100_000_000_000_000_000_u128);
        let exponent = base_two_exponent(num);

        // assert_eq!(exponent, 1);

        // 10_000_000_000_000_000_u128 => 0.01
        let num = Decimal::from_scaled_val(10_000_000_000_000_000_u128);
        let exponent = base_two_exponent(num);

        // assert_eq!(exponent, 2);

        // 1_000 => 0.000_000_000_000_001
        let num = Decimal::from_scaled_val(1_000u128);
        let exponent = base_two_exponent(num);

        // assert_eq!(exponent, 15);

        // 1 => 0.000_000_000_000_000_001
        let num = Decimal::from_scaled_val(1u128);
        let exponent = base_two_exponent(num);

        // assert_eq!(exponent, 18);

        // Tesing very large amounts
        let num = Decimal::from(u64::MAX / 2);
        let exponent = base_two_exponent(num);

        // assert_eq!(exponent, 18);

        let num = Decimal::from(u128::MAX / 2);
        let exponent = base_two_exponent(num);

        // assert_eq!(exponent, 38);

        Ok(())
    }

    proptest! {
        #[test]
        fn successfully_returns_positive_exponent(
            num in 1..100_000_000_000_000_000_000_u128,
        ) {
            let actual_result = base_two_exponent(Decimal::from(num));

            let num = num * 1_000_000_000_000_000_000;
            let expected_result = (0..)
                .take_while(|i| {
                    Decimal::from(2_u64).try_pow(*i).unwrap() <= Decimal::from(num)
                })
                .count() as u32 - 60;

            assert_eq!(
                actual_result,
                expected_result
            );
        }
    }
}
