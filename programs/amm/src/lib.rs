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

// TODO: conditionally compile this based on feature "prod"
// TODO: new dev pubkey
declare_id!("DFarMhaRkdYqhK5jZsexMftaJuWHrY7VzAfkXx5ZmxqZ");

#[program]
pub mod amm {
    //
}
