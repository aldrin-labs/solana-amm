use crate::models::Permillion;

/// There is no use case for more than 4 reserves from business perspective, and
/// to enable more than 4 reserves for stable curve, we would have to calculate
/// the invariant in a more complicated fashion. Therefore we limit the number
/// of possible reserves to 4.
pub const MAX_RESERVES: usize = 4;

/// The pool's admin can change the swap fee with
/// [`crate::endpoints::set_pool_swap_fee`] endpoint. However, we limit this
/// update to a maximum fee given by this constant.
pub const MAX_SWAP_FEE: Permillion = Permillion {
    // 1%
    permillion: 1_0000,
};

/// The program owner gets a share of the swap fee defined by this value.
pub const PROGRAM_TOLL_SWAP_FEE_SHARE: Permillion = Permillion {
    // 1/3
    permillion: 33_3333,
};
