//! Admin of a pool can change the swap fee to a maximum of
//! [`consts::MAX_SWAP_FEE`].

use crate::prelude::*;

#[derive(Accounts)]
pub struct SetPoolSwapFee<'info> {
    pub admin: Signer<'info>,
    #[account(
        mut,
        constraint = pool.admin.key() == admin.key()
            @ err::acc("The signer must match pool's admin"),
    )]
    pub pool: Account<'info, Pool>,
}

pub fn handle(ctx: Context<SetPoolSwapFee>, fee: Permillion) -> Result<()> {
    let accs = ctx.accounts;

    if fee > consts::MAX_SWAP_FEE {
        return Err(error!(err::arg(format!(
            "Maximum fee can be {} permillion",
            consts::MAX_SWAP_FEE.permillion
        ),)));
    }

    accs.pool.fee = fee;

    Ok(())
}
