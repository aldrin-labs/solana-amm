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

/// How many times within the snapshot history can a new harvest period be
/// added. We need to keep the harvest history because the harvest calculation
/// can happen after the period ends.
pub const HARVEST_PERIODS_LEN: usize = 10;

/// How many snapshots are there in the ring buffer. This count multiplied by
/// [`MIN_SNAPSHOT_WINDOW_SLOTS`] gives us for how many slots is the history
/// kept.
pub const SNAPSHOTS_LEN: usize = 1000;

/// Automation must wait at least this many slots before it can take a new
/// snapshot.
///
/// There are ~2 slots per second. The
/// [`crate::endpoints::take_snapshot`] endpoint is available for a
/// single [`crate::models::Farm`] at most this often.
pub const MIN_SNAPSHOT_WINDOW_SLOTS: u64 = 2 * 3600;
