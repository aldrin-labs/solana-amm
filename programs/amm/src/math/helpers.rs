use crate::prelude::*;

pub fn fold_product(values: &[LargeDecimal]) -> Result<LargeDecimal> {
    let result = values
        .iter()
        .try_fold(LargeDecimal::one(), |acc, el| acc.try_mul(el))?;

    Ok(result)
}

pub fn fold_sum(values: &[LargeDecimal]) -> Result<LargeDecimal> {
    let result = values
        .iter()
        .try_fold(LargeDecimal::zero(), |acc, el| acc.try_add(el))?;

    Ok(result)
}

/// This function receives a number in Decimal type form and will return the
/// exponent of the number. We find the exponent using a naive method of
/// counting the number of digits in the Decimal type.
///
/// Input `num` in scientific notation follows: num = x . 10^exponent
///
/// The exponenet of an integer number can be naively obtained by counting its
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
/// this cases we will flag the exponenet returned as being a negative exponent.
///
/// This function returns a tuple Result of (u64, bool) representing the
/// exponent and its sign (positive if true, negative if false)
fn find_exponent(num: Decimal) -> Result<(u64, bool)> {
    let num = num.to_scaled_val().unwrap();
    let exponent_in_decimal = num.to_string().chars().count() as u64;

    if exponent_in_decimal >= 19 {
        // In case the number is above or equal to 1, which means the exponent
        // will be positive
        Ok((exponent_in_decimal - 19, true))
    } else {
        // In case the number is below 1, which means the exponent
        // will be negative
        Ok((19 - exponent_in_decimal, false))
    }
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
    let (a_exponent, _) = find_exponent(a)?;
    let (b_exponent, _) = find_exponent(b)?;
    let (_, c_is_bigger_than_or_eq_one) = find_exponent(c)?;
    // In case c is less than one,the division will always increase
    // the number computed therefore we just follow normally. There is
    // risk of overflow but we cannot do anything to mitigate that risk.
    if !c_is_bigger_than_or_eq_one {
        return a.try_mul(b)?.try_div(c);
    }
    if a_exponent + b_exponent >= 38 {
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
    }
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
        let (exponent, is_bigger_than_or_eq_one) = find_exponent(num)?;

        assert_eq!(exponent, 0_u64);
        assert_eq!(is_bigger_than_or_eq_one, true);

        let num = Decimal::from(10_u64);
        let (exponent, is_bigger_than_or_eq_one) = find_exponent(num)?;

        assert_eq!(exponent, 1_u64);
        assert_eq!(is_bigger_than_or_eq_one, true);

        let num = Decimal::from(100_u64);
        let (exponent, is_bigger_than_or_eq_one) = find_exponent(num)?;

        assert_eq!(exponent, 2_u64);
        assert_eq!(is_bigger_than_or_eq_one, true);

        let num = Decimal::from(100_000_000_u64);
        let (exponent, is_bigger_than_or_eq_one) = find_exponent(num)?;

        assert_eq!(exponent, 8_u64);
        assert_eq!(is_bigger_than_or_eq_one, true);

        let num = Decimal::from(18_446_744_073_709_551_615_u64);
        let (exponent, is_bigger_than_or_eq_one) = find_exponent(num)?;

        assert_eq!(exponent, 19_u64);
        assert_eq!(is_bigger_than_or_eq_one, true);

        // Testing on small numbers
        // 1_000_000_000_000_000_000_u128 => 1
        let num = Decimal::from_scaled_val(1_000_000_000_000_000_000_u128);
        let (exponent, is_bigger_than_or_eq_one) = find_exponent(num)?;

        assert_eq!(exponent, 0_u64);
        assert_eq!(is_bigger_than_or_eq_one, true);

        // 100_000_000_000_000_000_u128 => 0.1
        let num = Decimal::from_scaled_val(100_000_000_000_000_000_u128);
        let (exponent, is_bigger_than_or_eq_one) = find_exponent(num)?;

        assert_eq!(exponent, 1_u64);
        assert_eq!(is_bigger_than_or_eq_one, false);

        // 10_000_000_000_000_000_u128 => 0.01
        let num = Decimal::from_scaled_val(10_000_000_000_000_000_u128);
        let (exponent, is_bigger_than_or_eq_one) = find_exponent(num)?;

        assert_eq!(exponent, 2_u64);
        assert_eq!(is_bigger_than_or_eq_one, false);

        // 1 => 0.000_000_000_000_000_001
        let num = Decimal::from_scaled_val(1u128);
        let (exponent, is_bigger_than_or_eq_one) = find_exponent(num)?;

        assert_eq!(exponent, 18_u64);
        assert_eq!(is_bigger_than_or_eq_one, false);

        Ok(())
    }

    proptest! {
        #[test]
        fn successfully_returns_positive_exponent(
            num in 1..100_000_000_000_000_000_000_u128,
        ) {
            assert!(find_exponent(Decimal::from(num)).is_ok());

            let actual_result = find_exponent(Decimal::from(num)).unwrap();

            let num = num * 1_000_000_000_000_000_000;
            let expected_result = (0..)
                .take_while(|i| {
                    Decimal::from(10_u64).try_pow(*i).unwrap() <= Decimal::from(num)
                })
                .count() as u64 - 19;

            // Assert exponent
            assert_eq!(
                actual_result.0,
                expected_result
            );

            // Assert that exponenet is positive
            assert_eq!(
                actual_result.1,
                true
            );
        }

        #[test]
        fn successfully_returns_negative_exponent(
            num in 1..1_000_000_000_000_000_000_u128,
        ) {
            let num_dec = Decimal::from_scaled_val(num);

            let actual_result = find_exponent(Decimal::from(num_dec)).unwrap();

            let expected_result = 19 - (0..)
                .take_while(|i| {
                    Decimal::from(10_u64).try_pow(*i).unwrap()
                    <=
                    Decimal::from(num_dec.to_scaled_val().unwrap())
                })
                .count() as u64;

            // Assert exponent
            assert_eq!(
                actual_result.0,
                expected_result
            );

            // Assert that exponenet is negative
            assert_eq!(
                actual_result.1,
                false
            );
        }
    }
}
