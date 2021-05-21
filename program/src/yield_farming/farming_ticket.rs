use solana_program::{
    program_pack::{IsInitialized, Sealed, Pack},
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::UnixTimestamp,
};

use hex_literal::hex;
use arrayref::{array_ref,array_refs, array_mut_ref, mut_array_refs};

pub const TICKET_DISCRIMINATOR: [u8;8] = hex!("071EADFFAFE10C22");

#[derive(Debug, Default, PartialEq)]
pub struct FarmingTicket {
    pub discriminator: u64,
    pub is_initialized: bool,
    pub tokens_frozen: u64,
    pub start_time: UnixTimestamp,
    pub token_authority: Pubkey,
    pub farming_state: Pubkey,
}

impl FarmingTicket {
    /// Special check to be done before any instruction processing
    pub fn is_initialized(input: &[u8]) -> bool {
        match Self::unpack(input) {
            Ok(ticket) => ticket.is_initialized,
            Err(_) => false,
        }
    }
}

impl Sealed for FarmingTicket {}
impl IsInitialized for FarmingTicket {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for FarmingTicket {
    const LEN: usize = 89;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 89];
        let (
            discriminator,
            is_initialized,
            tokens_frozen,
            start_time,
            token_authority,
            farming_state,
        ) = mut_array_refs![output, 8, 1, 8, 8, 32, 32];
        discriminator.copy_from_slice(&self.discriminator.to_le_bytes());
        is_initialized[0] = self.is_initialized as u8;
        tokens_frozen.copy_from_slice(&self.tokens_frozen.to_le_bytes());
        start_time.copy_from_slice(&self.start_time.to_le_bytes());
        token_authority.copy_from_slice(self.token_authority.as_ref());
        farming_state.copy_from_slice(self.farming_state.as_ref());
    }

    /// Unpacks a byte buffer into a [SwapV1](struct.SwapV1.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 89];
        #[allow(clippy::ptr_offset_with_cast)]
            let (
            discriminator,
            is_initialized,
            tokens_frozen,
            start_time,
            token_authority,
            farming_state,
        ) = array_refs![input, 8, 1, 8, 8, 32, 32];
        Ok(Self {
            discriminator: match discriminator {
                &TICKET_DISCRIMINATOR => u64::from_le_bytes(TICKET_DISCRIMINATOR),
                _ => return Err(ProgramError::InvalidAccountData),
            },
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            tokens_frozen: u64::from_le_bytes(*tokens_frozen),
            start_time: UnixTimestamp::from_le_bytes(*start_time),
            token_authority: Pubkey::new_from_array(*token_authority),
            farming_state: Pubkey::new_from_array(*farming_state),
        })
    }
}