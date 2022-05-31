use crate::prelude::*;

#[derive(Accounts)]
pub struct UpdateEligibleHarvest<'info> {
    #[account(mut)]
    pub farmer: Account<'info, Farmer>,
    pub farm: AccountLoader<'info, Farm>,
}

pub fn handle(_ctx: Context<UpdateEligibleHarvest>) -> Result<()> {
    Ok(())
}
