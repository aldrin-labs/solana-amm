use crate::models::Permillion;

/// TODO: why
pub const MAX_RESERVES: usize = 4;

/// The pool's admin can change the swap fee with
/// [`crate::endpoints::set_pool_swap_fee`] endpoint. However, we limit this
/// update to a maximum fee given by this constant.
pub const MAX_SWAP_FEE: Permillion = Permillion {
    // 1%
    permillion: 10_000,
};
