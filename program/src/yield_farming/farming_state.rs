use solana_program::{
    program_pack::{IsInitialized, Sealed, Pack},
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::UnixTimestamp,
};

use hex_literal::hex;
use arrayref::{array_ref,array_refs, array_mut_ref, mut_array_refs};

use crate::yield_farming::snapshots::SnapshotQueue;
use crate::yield_farming::farming_ticket::FarmingTicket;

pub const FARMING_STATE_DISCRIMINATOR: [u8;8] = hex!("183A2D454BEF7B11");
pub const NO_WITHDRAWAL_TIME: UnixTimestamp = 60 * 60 * 24 * 30 * 6; ///Nearly 6 months


#[derive(Debug, Default, PartialEq)]
pub struct FarmingState {
    pub discriminator: u64,
    pub is_initialized: bool,
    pub tokens_unlocked: u64,   
    pub tokens_per_period: u64,
    pub tokens_total: u64,
    pub period_length: u64,
    pub start_time: UnixTimestamp,
    pub current_time: UnixTimestamp,
    pub attached_swap_account: Pubkey,
    pub farming_token_account: Pubkey,
    pub farming_snapshots: SnapshotQueue,
}


impl FarmingState {
    /// Special check to be done before any instruction processing
    pub fn is_initialized(input: &[u8]) -> Result<bool, ProgramError> {
        let bool_slice = input
            .split_at(8)
            .1
            .first()
            .expect("Wrong slice size");

        match bool_slice {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ProgramError::InvalidAccountData.into())
        }
    }

    /// Check if no withdrawal time period passed
    pub fn is_no_withdrawal_period_passed(&self, current_time: UnixTimestamp) -> bool {
        if current_time > self.start_time + NO_WITHDRAWAL_TIME {
            return true
        }
        false
    }

    pub fn calculate_withdraw_tokens(&self, farming_ticket: &FarmingTicket) -> Option<(u128, UnixTimestamp)> {
        let mut max_tokens : u128 = 0;
        let mut last_timestamp = farming_ticket.start_time;
        let mut last_snapshot_tokens = 0;
        for snapshot in self.farming_snapshots.snapshots.iter() {
            if snapshot.time > farming_ticket.start_time && snapshot.time < farming_ticket.end_time {
                let tokens = (snapshot.farming_tokens as u128)
                    .checked_sub(last_snapshot_tokens as u128)?
                    .checked_mul(farming_ticket.tokens_frozen as u128)?
                    .checked_div(snapshot.tokens_frozen as u128)?;
                max_tokens = max_tokens.checked_add(tokens)?;
                last_timestamp = snapshot.time;
                last_snapshot_tokens = snapshot.farming_tokens;
            }
        }
        Some((max_tokens, last_timestamp))
    }
}

impl Sealed for FarmingState {}
impl IsInitialized for FarmingState {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for FarmingState {
    const LEN: usize = 3879;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 3879];
        let (
             discriminator,
             is_initialized,
             tokens_unlocked,
             tokens_per_period,
             tokens_total,
             period_length,
             start_time,
             current_time,
             attached_swap_account,
             farming_token_account,
             farming_snapshots,
        ) = mut_array_refs![output, 8, 1, 8, 8, 8, 8, 8, 8, 32, 32, 3758];
        discriminator.copy_from_slice(&self.discriminator.to_le_bytes());
        is_initialized[0] = self.is_initialized as u8;
        tokens_unlocked.copy_from_slice(&self.tokens_unlocked.to_le_bytes());
        tokens_per_period.copy_from_slice(&self.tokens_per_period.to_le_bytes());
        tokens_total.copy_from_slice(&self.tokens_total.to_le_bytes());
        period_length.copy_from_slice(&self.period_length.to_le_bytes());
        start_time.copy_from_slice(&self.start_time.to_le_bytes());
        current_time.copy_from_slice(&self.current_time.to_le_bytes());
        attached_swap_account.copy_from_slice(&self.attached_swap_account.as_ref());
        farming_token_account.copy_from_slice(&self.farming_token_account.as_ref());
        self.farming_snapshots.pack_into_slice(farming_snapshots);
    }

    /// Unpacks a byte buffer into a [SwapV1](struct.SwapV1.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 3879];
        #[allow(clippy::ptr_offset_with_cast)]
            let (
            discriminator,
            is_initialized,
            tokens_unlocked,
            tokens_per_period,
            tokens_total,
            period_length,
            start_time,
            current_time,
            attached_swap_account,
            farming_token_account,
            farming_snapshots,
        ) = array_refs![input, 8, 1, 8, 8, 8, 8, 8, 8, 32, 32, 3758];
        Ok(Self {
            discriminator: match discriminator {
                &FARMING_STATE_DISCRIMINATOR => u64::from_le_bytes(FARMING_STATE_DISCRIMINATOR),
                _ => return Err(ProgramError::InvalidAccountData),
            },
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            tokens_unlocked: u64::from_le_bytes(*tokens_unlocked),
            tokens_per_period: u64::from_le_bytes(*tokens_per_period),
            tokens_total: u64::from_le_bytes(*tokens_total),
            period_length:  u64::from_le_bytes(*period_length),
            start_time: UnixTimestamp::from_le_bytes(*start_time),
            current_time: UnixTimestamp::from_le_bytes(*current_time),
            attached_swap_account: Pubkey::new_from_array(*attached_swap_account),
            farming_token_account: Pubkey::new_from_array(*farming_token_account),
            farming_snapshots: SnapshotQueue::unpack_from_slice(farming_snapshots.as_ref())?
        })
    }
}