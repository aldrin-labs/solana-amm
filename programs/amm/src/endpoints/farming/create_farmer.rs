use crate::prelude::*;

#[derive(Accounts)]
pub struct CreateFarmer<'info> {
    pub authority: Signer<'info>,
    pub farmer: Account<'info, Farmer>,
    pub farm: AccountLoader<'info, Farm>,
    pub system_program: Program<'info, System>,
}

pub fn handle(_ctx: Context<CreateFarmer>) -> Result<()> {
    Ok(())
}
