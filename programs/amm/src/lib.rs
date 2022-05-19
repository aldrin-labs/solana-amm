pub mod consts;
pub mod endpoints;
pub mod err;
pub mod models;
mod prelude;

use crate::prelude::*;
use endpoints::*;

// TODO: conditionally compile this based on feature "prod"
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod amm {
    use super::*;

    pub fn create_lp_vault(ctx: Context<CreateLpVault>) -> Result<()> {
        endpoints::farming::create_lp_vault::handle(ctx)
    }
}
