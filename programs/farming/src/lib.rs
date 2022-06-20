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
declare_id!("DFarMhaRkdYqhK5jZsexMftaJuWHrY7VzAfkXx5ZmxqZ");

#[program]
pub mod farming {
    use super::*;

    pub fn create_farm(ctx: Context<CreateFarm>) -> Result<()> {
        endpoints::create_farm::handle(ctx)
    }

    pub fn add_harvest(ctx: Context<AddHarvest>) -> Result<()> {
        endpoints::add_harvest::handle(ctx)
    }

    pub fn remove_harvest(
        ctx: Context<RemoveHarvest>,
        harvest_mint: Pubkey,
    ) -> Result<()> {
        endpoints::remove_harvest::handle(ctx, harvest_mint)
    }

    pub fn set_farm_owner(ctx: Context<SetFarmOwner>) -> Result<()> {
        endpoints::set_farm_owner::handle(ctx)
    }

    pub fn new_harvest_period(
        ctx: Context<NewHarvestPeriod>,
        harvest_mint: Pubkey,
        starts_at: Slot,
        period_length_in_slots: u64,
        tokens_per_slot: TokenAmount,
    ) -> Result<()> {
        endpoints::new_harvest_period::handle(
            ctx,
            harvest_mint,
            starts_at,
            period_length_in_slots,
            tokens_per_slot,
        )
    }

    pub fn take_snapshot(ctx: Context<TakeSnapshot>) -> Result<()> {
        endpoints::take_snapshot::handle(ctx)
    }

    pub fn set_min_snapshot_window(
        ctx: Context<SetMinSnapshotWindow>,
        min_snapshot_window_slots: u64,
    ) -> Result<()> {
        endpoints::set_min_snapshot_window::handle(
            ctx,
            min_snapshot_window_slots,
        )
    }

    pub fn create_farmer(ctx: Context<CreateFarmer>) -> Result<()> {
        endpoints::create_farmer::handle(ctx)
    }

    pub fn close_farmer(ctx: Context<CloseFarmer>) -> Result<()> {
        endpoints::close_farmer::handle(ctx)
    }

    pub fn start_farming(
        ctx: Context<StartFarming>,
        stake: TokenAmount,
    ) -> Result<()> {
        endpoints::start_farming::handle(ctx, stake)
    }

    pub fn stop_farming(
        ctx: Context<StopFarming>,
        unstake_max: TokenAmount,
    ) -> Result<()> {
        endpoints::stop_farming::handle(ctx, unstake_max)
    }

    pub fn update_eligible_harvest(
        ctx: Context<UpdateEligibleHarvest>,
    ) -> Result<()> {
        endpoints::update_eligible_harvest::handle(ctx)
    }

    pub fn claim_eligible_harvest<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimEligibleHarvest<'info>>,
    ) -> Result<()> {
        endpoints::claim_eligible_harvest::handle(ctx)
    }

    pub fn whitelist_farm_for_compounding(
        ctx: Context<WhitelistFarmForCompouding>,
    ) -> Result<()> {
        endpoints::whitelist_farm_for_compounding::handle(ctx)
    }

    pub fn dewhitelist_farm_for_compounding(
        ctx: Context<DewhitelistFarmForCompounding>,
    ) -> Result<()> {
        endpoints::dewhitelist_farm_for_compounding::handle(ctx)
    }

    pub fn compound_same_farm(ctx: Context<CompoundSameFarm>) -> Result<()> {
        endpoints::compound_same_farm::handle(ctx)
    }

    pub fn compound_across_farms(
        ctx: Context<CompoundAcrossFarms>,
    ) -> Result<()> {
        endpoints::compound_across_farms::handle(ctx)
    }
}
