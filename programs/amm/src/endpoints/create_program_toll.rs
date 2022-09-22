//! Program toll defines the authority that owns fee wallets. When a pool is
//! created, one of the inputs is an LP wallet which receives a cut on each
//! swap. In the [`crate::endpoints::create_pool`] endpoint we assert that the
//! wallet has authority given by the singleton toll account.

use crate::prelude::*;

/// Only the program authority can define who receives the toll.
#[cfg(not(feature = "dev"))]
#[derive(Accounts)]
pub struct CreateProgramToll<'info> {
    #[account(mut)]
    pub program_authority: Signer<'info>,
    #[account(
        constraint = amm.programdata_address()? == Some(amm_metadata.key())
            @ err::acc("AMM program metadata account mismatch"),
    )]
    pub amm: Program<'info, crate::program::Amm>,
    #[account(
        constraint = amm_metadata.upgrade_authority_address ==
            Some(program_authority.key())
            @ err::acc("Signer isn't program's authority"),
    )]
    pub amm_metadata: Account<'info, ProgramData>,
    /// CHECK: authority over LP token vaults for the program toll
    pub program_toll_authority: AccountInfo<'info>,
    #[account(
        init,
        payer = program_authority,
        space = ProgramToll::space(),
        seeds = [ProgramToll::PDA_SEED],
        bump,
    )]
    pub program_toll: Account<'info, ProgramToll>,
    pub system_program: Program<'info, System>,
}

/// Due to the way the anchor loads programs on localnet (so that we can use any
/// pubkey and don't have to sign the program deploy), the programs on localnet
/// don't have the same structure in terms of having a data account as with
/// normal deployment.
///
/// That's why for localnet we compile the program under a dev feature and
/// remove the checks which are in the production program. The integration tests
/// still use the production endpoint.
#[cfg(feature = "dev")]
#[derive(Accounts)]
pub struct CreateProgramToll<'info> {
    #[account(mut)]
    pub program_toll_authority: Signer<'info>,
    #[account(
        init,
        payer = program_toll_authority,
        space = ProgramToll::space(),
        seeds = [ProgramToll::PDA_SEED],
        bump,
    )]
    pub program_toll: Account<'info, ProgramToll>,
    pub system_program: Program<'info, System>,
}

pub fn handle(ctx: Context<CreateProgramToll>) -> Result<()> {
    let accounts = ctx.accounts;

    accounts.program_toll.authority = accounts.program_toll_authority.key();

    Ok(())
}
