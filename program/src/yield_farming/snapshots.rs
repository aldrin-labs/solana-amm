use solana_program::{
    program_pack::{IsInitialized, Sealed, Pack},
    program_error::ProgramError,
    clock::UnixTimestamp,
};

use arrayref::{array_ref,array_refs, array_mut_ref, mut_array_refs};

pub const QUEUE_LENGTH: usize = 150;

#[derive(Debug, Default, PartialEq, Copy, Clone)]
pub struct Snapshot {
    pub is_initialized: bool,
    pub tokens_frozen: u64,
    pub farming_tokens: u64,
    pub time: UnixTimestamp,
}

#[derive(Debug, PartialEq)]
pub struct SnapshotQueue {
    pub next_index: u64,
    pub snapshots: Vec<Snapshot>,
}

impl Default for SnapshotQueue {
    fn default() -> Self {
        SnapshotQueue{
            next_index: 0,
            snapshots: vec![Snapshot::default(); QUEUE_LENGTH],
        }
    }
}

impl Sealed for Snapshot {}
impl IsInitialized for Snapshot {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for Snapshot {
    const LEN: usize = 25;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 25];
        let (
            is_initialized,
            tokens_frozen,
            farming_tokens,
            time
        ) = mut_array_refs![output, 1, 8, 8, 8];
        is_initialized[0] = self.is_initialized as u8;
        tokens_frozen.copy_from_slice(&self.tokens_frozen.to_le_bytes());
        farming_tokens.copy_from_slice(&self.farming_tokens.to_le_bytes());
        time.copy_from_slice(&self.time.to_le_bytes());
    }

    /// Unpacks a byte buffer into a [SwapV1](struct.SwapV1.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 25];
        #[allow(clippy::ptr_offset_with_cast)]
            let (
            is_initialized,
            tokens_frozen,
            farming_tokens,
            time,
        ) = array_refs![input, 1, 8, 8, 8];
        Ok(Self {
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            tokens_frozen: u64::from_le_bytes(*tokens_frozen),
            farming_tokens: u64::from_le_bytes(*farming_tokens),
            time: UnixTimestamp::from_le_bytes(*time),
        })
    }
}

impl Sealed for SnapshotQueue {}
impl IsInitialized for SnapshotQueue {
    fn is_initialized(&self) -> bool {
        true
    }
}

impl Pack for SnapshotQueue {
    const LEN: usize = 3758;// 9 + 25 * QUEUE_LENGTH;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 3758];
        let (
            next_index,
            snapshots
        ) = mut_array_refs![output, 8, 3750];
        next_index.copy_from_slice(&self.next_index.to_le_bytes());
        pack_snapshot_slice(&self.snapshots, snapshots);
    }

    /// Unpacks a byte buffer into a [SwapV1](struct.SwapV1.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 3758];
        #[allow(clippy::ptr_offset_with_cast)]
            let (
            next_index,
            snapshots
        ) = array_refs![input, 8, 3750];
        let next_index = u64::from_le_bytes( *next_index);
        Ok(Self {
            next_index,
            snapshots: unpack_snapshot_slice(snapshots.as_ref(), next_index as usize)?
        })
    }
}

fn pack_snapshot_slice(snapshots: &Vec<Snapshot>, output: &mut [u8]) {
    let mut pos: usize = 0;
    let byte_size: usize = Snapshot::LEN;
    for snapshot in snapshots.iter() {
        snapshot.pack_into_slice(&mut output[pos..pos + byte_size]);
        pos+= byte_size;
    }
}

fn unpack_snapshot_slice(input: &[u8], length: usize) -> Result<Vec<Snapshot>, ProgramError> {
    let mut i: usize = 0;
    let mut rest = input;
    let mut ret_vec: Vec<Snapshot> = vec![];
    while i < length {
        let (snapshot, rest_input) = rest.split_at(Snapshot::LEN);
        rest = rest_input;
        ret_vec.push(Snapshot::unpack_from_slice(snapshot)?);
        i += 1;
    }
   /* while i < QUEUE_LENGTH {
        ret_vec.push(Snapshot::default());
        i += 1;
    }*/
    Ok(ret_vec)
}