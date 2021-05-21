#![deny(missing_docs)]

//! An Uniswap-like program for the Solana blockchain.
solana_program::declare_id!("8AQsHn1JdNoBmj6fGGy64Y4eu1xYyoAXhEZ8c72xDqvJ");

pub mod constraints;
pub mod curve;
pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;
mod yield_farming;

#[cfg(not(feature = "no-entrypoint"))]
mod entrypoint;

// Export current sdk types for downstream users building with a different sdk version
pub use solana_program;