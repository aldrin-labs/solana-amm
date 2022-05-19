use crate::prelude::*;

#[derive(Accounts)]
pub struct CreateLpVault {}

pub fn handle(_ctx: Context<CreateLpVault>) -> Result<()> {
    Ok(())
}
