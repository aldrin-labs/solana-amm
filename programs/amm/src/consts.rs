/// A limit on how many harvest mints can be associated with a single stake
/// vault. A harvest mint is represented by [`crate::models::Farm`].
///
/// This threshold projects into [`crate::models::Farmer`]'s property
/// `available_harvest` and into [`crate::models::Farm`]'s property
/// `harvests`.
///
/// We opt for a value based on a judgement call with the tokenomics team. In
/// the old program, this value was 10.
///
/// While a design with unlimited number of harvest mints would be possible, it
/// would require many accounts and out goal is to optimize for transaction
/// size.
pub const MAX_HARVEST_MINTS: usize = 10;

/// The admin can change the configurable `tokens_per_slot` only this many
/// times per `[`SNAPSHOTS_LEN`] * [`MIN_SNAPSHOT_WINDOW_SLOTS`]` slots.
pub const TOKENS_PER_SLOT_HISTORY_LEN: usize = 10;

/// How many snapshots are there in the ring buffer. This count multiplied by
/// [`MIN_SNAPSHOT_WINDOW_SLOTS`] gives us for how many slots is the history
/// kept.
pub const SNAPSHOTS_LEN: usize = 1000;

/// There are ~2 slots per second. The
/// [`crate::endpoints::farming::take_snapshot`] endpoint is available for a
/// single [`crate::models::Farm`] at most this often.
pub const MIN_SNAPSHOT_WINDOW_SLOTS: usize = 2 * 3600;
