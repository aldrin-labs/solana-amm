// We use zero copy for obligation. Zero copy uses
// [repr(packed)](https://doc.rust-lang.org/nomicon/other-reprs.html). In future
// releases, taking a reference to a field which is packed will not compile.
// We will need to, eventually, copy out fields we want to use, or create
// pointers [manually](https://github.com/rust-lang/rust/issues/82523).
#![allow(unaligned_references, renamed_and_removed_lints, safe_packed_borrows)]

pub mod consts;
pub mod endpoints;
pub mod err;
pub mod models;
pub mod prelude;

use crate::prelude::*;
use endpoints::*;

// TODO: conditionally compile this based on feature "prod"
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod amm {
    use super::*;

    pub fn create_farm(ctx: Context<CreateFarm>) -> Result<()> {
        endpoints::farming::create_farm::handle(ctx)
    }

    pub fn add_harvest(
        ctx: Context<AddHarvest>,
        tokens_per_slot: TokenAmount,
    ) -> Result<()> {
        endpoints::farming::add_harvest::handle(ctx, tokens_per_slot)
    }

    pub fn remove_harvest(
        ctx: Context<RemoveHarvest>,
        harvest_mint: Pubkey,
    ) -> Result<()> {
        endpoints::farming::remove_harvest::handle(ctx, harvest_mint)
    }

    pub fn set_farm_owner(ctx: Context<SetFarmOwner>) -> Result<()> {
        endpoints::set_farm_owner::handle(ctx)
    }

    pub fn set_tokens_per_slot(
        ctx: Context<SetTokensPerSlot>,
        harvest_mint: Pubkey,
        valid_from_slot: Slot,
        tokens_per_slot: TokenAmount,
    ) -> Result<()> {
        endpoints::farming::set_tokens_per_slot::handle(
            ctx,
            harvest_mint,
            valid_from_slot,
            tokens_per_slot,
        )
    }

    pub fn take_snapshot(ctx: Context<TakeSnapshot>) -> Result<()> {
        endpoints::farming::take_snapshot::handle(ctx)
    }

    pub fn set_min_snapshot_window(
        ctx: Context<SetMinSnapshotWindow>,
        min_snapshot_window_slots: u64,
    ) -> Result<()> {
        endpoints::farming::set_min_snapshot_window::handle(
            ctx,
            min_snapshot_window_slots,
        )
    }

    pub fn create_farmer(ctx: Context<CreateFarmer>) -> Result<()> {
        endpoints::farming::create_farmer::handle(ctx)
    }

    pub fn close_farmer(ctx: Context<CloseFarmer>) -> Result<()> {
        endpoints::farming::close_farmer::handle(ctx)
    }

    pub fn start_farming(
        ctx: Context<StartFarming>,
        stake: TokenAmount,
    ) -> Result<()> {
        endpoints::farming::start_farming::handle(ctx, stake)
    }

    pub fn stop_farming(
        ctx: Context<StopFarming>,
        unstake_max: TokenAmount,
    ) -> Result<()> {
        endpoints::farming::stop_farming::handle(ctx, unstake_max)
    }

    pub fn update_eligible_harvest(
        ctx: Context<UpdateEligibleHarvest>,
    ) -> Result<()> {
        endpoints::farming::update_eligible_harvest::handle(ctx)
    }

    pub fn claim_eligible_harvest(
        ctx: Context<ClaimEligibleHarvest>,
    ) -> Result<()> {
        endpoints::farming::claim_eligible_harvest::handle(ctx)
    }
}
