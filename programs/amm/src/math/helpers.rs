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

/// This function returns leading zeroes after the decimal point. The decimal
/// point is after 10^18. However, this number cannot be translated directly to
/// binary. Therefore, we get the next power of two, which is 2^60, and count
/// leading zeroes from that number.
///
/// In another words, first 2^60 is considered point decimal and is ignored, so
/// 10^18 (1 WAD) has 192 - 60 = 132 leading zeroes.
fn integer_leading_zeros(num: Decimal) -> u32 {
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

    max_exponent - leading_zeroes
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
/// bound, whilst only having  a floor of 10^-18 on the lower bound.
pub fn try_mul_div(a: Decimal, b: Decimal, c: Decimal) -> Result<Decimal> {
    // In case `c` is less than one, the division will always increase
    // the number computed therefore we just follow normally. There is
    // risk of overflow but we cannot do anything to mitigate that risk.
    if c < Decimal::one() {
        return a.try_mul(b)?.try_div(c);
    }

    let a_lz = integer_leading_zeros(a);
    let b_lz = integer_leading_zeros(b);

    // 192 - 60 (see the fn integer_leading_zeros) is the max exponent. We cut
    // some slack in this heuristic and chose 130 as the point until which we
    // can still multiply a and b.
    if a_lz + b_lz >= 130 {
        // This means that multiplying `a` and `b` will lead to a very high
        // number, potentially bigger than 1*10^39 and therefore to decrease
        // risk of overflow we divide first the highest numerator by c to
        // decrease the exponent
        if a_lz >= b_lz {
            // In this case `a` is bigger than or equal to `b`, so we will
            // divide `a` by `c` before multiplying it by `b`
            a.try_div(c)?.try_mul(b)
        } else {
            // In this case `a` is smaller than `b`, so we will divide
            // `b` by `c` before multiplying it with `a`
            b.try_div(c)?.try_mul(a)
        }
    } else {
        // This means that it is safe to multiply `a` and `b` because it will
        // never be bigger than 1*10^39 and therefore it should not overflow.
        //
        // It also means that if `a` is a very small number and `c` is a very
        // big it is better to multiply `a` with `b` to reduce the decimal cases
        // before dividing by `c`
        //
        // If both `a` and `b` are very small numbers and `c` is a very big
        // number, then there is nothing we can do to reduce the risk of
        // overflow and we follow with this order
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
        let exponent = integer_leading_zeros(num);

        assert_eq!(exponent, 0);

        let num = Decimal::from(10_u64);
        let exponent = integer_leading_zeros(num);

        // 10 in binary is 1010
        assert_eq!(exponent, 4);

        let num = Decimal::from(100_u64);
        let exponent = integer_leading_zeros(num);

        // 100 in binary is 1_100_100
        assert_eq!(exponent, 7);

        let num = Decimal::from(1_000u128);
        let exponent = integer_leading_zeros(num);

        // 1_000 in binary is 1_111_101_000
        assert_eq!(exponent, 10);

        let num = Decimal::from(100_000_000_u64);
        let exponent = integer_leading_zeros(num);

        // 100_000_000 in binary is 101_111_101_011_110_000_100_000_000
        assert_eq!(exponent, 27);

        let num = Decimal::from(18_446_744_073_709_551_615_u64);
        let exponent = integer_leading_zeros(num);

        // 18_446_744_073_709_551_615_u64 in binary is
        // 1_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111
        assert_eq!(exponent, 64);

        // Testing on small numbers -> should always return zero
        // 1_000_000_000_000_000_000_u128 => 1
        let num = Decimal::from_scaled_val(1_000_000_000_000_000_000_u128);
        let exponent = integer_leading_zeros(num);

        assert_eq!(exponent, 0);

        // 100_000_000_000_000_000_u128 => 0.1
        let num = Decimal::from_scaled_val(100_000_000_000_000_000_u128);
        let exponent = integer_leading_zeros(num);

        assert_eq!(exponent, 0);

        // 10_000_000_000_000_000_u128 => 0.01
        let num = Decimal::from_scaled_val(10_000_000_000_000_000_u128);
        let exponent = integer_leading_zeros(num);

        assert_eq!(exponent, 0);

        // 1_000 => 0.000_000_000_000_001
        let num = Decimal::from_scaled_val(1_000u128);
        let exponent = integer_leading_zeros(num);

        assert_eq!(exponent, 0);

        // 1 => 0.000_000_000_000_000_001
        let num = Decimal::from_scaled_val(1u128);
        let exponent = integer_leading_zeros(num);

        assert_eq!(exponent, 0);

        // Testing very large amounts

        // 9_223_372_036_854_775_807 in binary is
        // 111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111
        let num = Decimal::from(u64::MAX / 2);
        let exponent = integer_leading_zeros(num);
        assert_eq!(exponent, 63);

        // 170_141_183_460_469_231_731_687_303_715_884_105_727 in binary is
        // 1_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111_111
        let num = Decimal::from(u128::MAX / 2);
        let exponent = integer_leading_zeros(num);

        assert_eq!(exponent, 127);

        Ok(())
    }

    proptest! {
        #[test]
        fn successfully_returns_positive_exponent(
            num in 1..100_000_000_000_000_000_000_u128,
        ) {
            let actual_result = integer_leading_zeros(Decimal::from(num));

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
