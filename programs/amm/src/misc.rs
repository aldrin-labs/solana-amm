use crate::prelude::*;
use anchor_spl::token::Mint;

/// Reloads the mint, gets supply and prints it in a predictable way which
/// can then be parsed from tx logs.
///
/// This enables our backend to easily track the total amount of LP tokens.
pub fn print_lp_supply(lp_mint: &mut Account<Mint>) -> Result<()> {
    lp_mint.reload()?;
    msg!("lp-supply={}", lp_mint.supply);

    Ok(())
}
