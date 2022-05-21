use crate::prelude::*;

#[derive(Accounts)]
pub struct CreateFarm<'info> {
    pub admin: Signer<'info>,
}

pub fn handle(_ctx: Context<CreateFarm>) -> Result<()> {
    Ok(())
}
