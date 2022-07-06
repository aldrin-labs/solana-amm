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
