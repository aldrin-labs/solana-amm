pub mod consts;
pub mod endpoints;
pub mod err;
pub mod math;
pub mod misc;
pub mod models;
pub mod prelude;

use crate::endpoints::*;
use crate::prelude::*;

// TODO: conditionally compile this based on feature "dev"
declare_id!("dAMMP3unWqb4u2La1xczx6JSAZsGByo9amHgzkVY7FG");

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

    pub fn put_discount(
        ctx: Context<PutDiscount>,
        user: Pubkey,
        discount_amount: Permillion,
        valid_until: Slot,
    ) -> Result<()> {
        endpoints::put_discount::handle(ctx, user, discount_amount, valid_until)
    }

    pub fn set_pool_swap_fee(
        ctx: Context<SetPoolSwapFee>,
        fee: Permillion,
    ) -> Result<()> {
        endpoints::set_pool_swap_fee::handle(ctx, fee)
    }

    pub fn deposit_liquidity<'info>(
        ctx: Context<'_, '_, '_, 'info, DepositLiquidity<'info>>,
        max_amount_tokens: Vec<TokenLimit>,
    ) -> Result<()> {
        endpoints::deposit_liquidity::handle(ctx, max_amount_tokens)
    }

    pub fn redeem_liquidity<'info>(
        ctx: Context<'_, '_, '_, 'info, RedeemLiquidity<'info>>,
        lp_tokens_to_burn: TokenAmount,
        min_amount_tokens: Vec<TokenLimit>,
    ) -> Result<()> {
        endpoints::redeem_liquidity::handle(
            ctx,
            lp_tokens_to_burn,
            min_amount_tokens,
        )
    }

    pub fn swap<'info>(
        ctx: Context<'_, '_, '_, 'info, Swap<'info>>,
        sell: TokenAmount,
        min_buy: TokenAmount,
    ) -> Result<()> {
        endpoints::swap::handle(ctx, sell, min_buy)
    }
}
