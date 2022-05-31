use crate::prelude::*;

#[derive(Accounts)]
pub struct CloseFarmer<'info> {
    pub authority: Signer<'info>,
    #[account(mut, close = authority)]
    pub farmer: Account<'info, Farmer>,
}

pub fn handle(_ctx: Context<CloseFarmer>) -> Result<()> {
    Ok(())
}
