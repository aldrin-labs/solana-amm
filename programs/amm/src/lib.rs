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
mod prelude;

use crate::prelude::*;
use endpoints::*;

// TODO: conditionally compile this based on feature "prod"
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod amm {
    use super::*;

    pub fn create_farm(
        ctx: Context<CreateFarm>,
        farm_signer_bump_seed: u8,
    ) -> Result<()> {
        endpoints::farming::create_farm::handle(ctx, farm_signer_bump_seed)
    }

    pub fn add_harvest(
        ctx: Context<AddHarvest>,
        farm_signer_bump_seed: u8,
        tokens_per_slot: TokenAmount,
    ) -> Result<()> {
        endpoints::farming::add_harvest::handle(
            ctx,
            farm_signer_bump_seed,
            tokens_per_slot,
        )
    }

    pub fn remove_harvest(
        ctx: Context<RemoveHarvest>,
        farm_signer_bump_seed: u8,
        harvest_mint: Pubkey,
    ) -> Result<()> {
        endpoints::farming::remove_harvest::handle(
            ctx,
            farm_signer_bump_seed,
            harvest_mint,
        )
    }
}
