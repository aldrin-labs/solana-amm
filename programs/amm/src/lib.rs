pub mod consts;
pub mod endpoints;
pub mod err;
pub mod math;
pub mod models;
pub mod prelude;

use crate::endpoints::*;
use crate::prelude::*;

// TODO: conditionally compile this based on feature "dev"
declare_id!("DammDkC9TSZvYvggRVAwCRcKm1prRkyu84N1Ph6Qckx");

#[program]
pub mod amm {
    use super::*;

    /// # Important
    /// This endpoint requires different accounts based on whether the program
    /// is compiled with the "dev" feature.
    pub fn create_program_toll(ctx: Context<CreateProgramToll>) -> Result<()> {
        endpoints::create_program_toll::handle(ctx)
    }

    /// # Important
    /// This endpoint requires different accounts based on whether the program
    /// is compiled with the "dev" feature.
    pub fn create_discount_settings(
        ctx: Context<CreateDiscountSettings>,
    ) -> Result<()> {
        endpoints::create_discount_settings::handle(ctx)
    }

    pub fn create_pool(ctx: Context<CreatePool>, amplifier: u64) -> Result<()> {
        endpoints::create_pool::handle(ctx, amplifier)
    }
}
