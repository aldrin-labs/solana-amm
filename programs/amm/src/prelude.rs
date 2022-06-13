pub use crate::err::{self, AmmError};
pub use crate::models::*;
pub use crate::{consts, endpoints};
pub use anchor_lang::prelude::*;
pub use decimal::{Decimal, TryAdd, TryDiv, TryMul};

#[cfg(test)]
pub mod utils {
    use super::*;

    /// Sets global clock to given slot. During testing, make sure to run
    /// serial test if there are clock dependent tests
    pub fn set_clock(slot: Slot) {
        assert!(slot.slot < 256);
        solana_sdk::program_stubs::set_syscall_stubs(Box::new(
            SyscallStubs::new(slot),
        ));
    }

    pub fn generate_snapshots(
        snapshots_raw_vec: &mut Vec<(u64, u64)>,
    ) -> Vec<Snapshot> {
        if snapshots_raw_vec.len() < consts::SNAPSHOTS_LEN {
            let slack = consts::SNAPSHOTS_LEN - snapshots_raw_vec.len();
            let mut slack_vec: Vec<(u64, u64)> = vec![(0, 0); slack];
            snapshots_raw_vec.append(&mut slack_vec);
        }

        let snapshots_vec = snapshots_raw_vec
            .iter()
            .map(|(slot, staked)| Snapshot {
                started_at: Slot { slot: *slot },
                staked: TokenAmount { amount: *staked },
            })
            .collect();

        snapshots_vec
    }

    pub fn generate_harvests(
        harvest_raw_vec: &mut Vec<(Pubkey, [TokensPerSlotHistory; 10])>,
    ) -> Vec<Harvest> {
        if harvest_raw_vec.len() < consts::MAX_HARVEST_MINTS {
            let slack = consts::MAX_HARVEST_MINTS - harvest_raw_vec.len();
            let mut slack_vec: Vec<(
                Pubkey,
                [TokensPerSlotHistory; consts::TOKENS_PER_SLOT_HISTORY_LEN],
            )> = vec![(Default::default(), Default::default()); slack];
            harvest_raw_vec.append(&mut slack_vec);
        }

        let harvest_vec = harvest_raw_vec
            .iter()
            .map(|(mint, tps)| Harvest {
                mint: *mint,
                tokens_per_slot: *tps,
                ..Default::default()
            })
            .collect();

        harvest_vec
    }

    pub fn generate_farmer_harvests(
        snapshots_raw_vec: &mut Vec<(Pubkey, u64)>,
    ) -> Vec<AvailableHarvest> {
        if snapshots_raw_vec.len() < consts::MAX_HARVEST_MINTS {
            let slack = consts::MAX_HARVEST_MINTS - snapshots_raw_vec.len();
            let mut slack_vec: Vec<(Pubkey, u64)> =
                vec![(Pubkey::default(), 0); slack];
            snapshots_raw_vec.append(&mut slack_vec);
        }

        let snapshots_vec = snapshots_raw_vec
            .iter()
            .map(|(mint, tokens)| AvailableHarvest {
                mint: *mint,
                tokens: TokenAmount { amount: *tokens },
            })
            .collect();

        snapshots_vec
    }

    pub fn generate_tps_history(
        tps_raw_vec: &mut Vec<(u64, u64)>,
    ) -> Vec<TokensPerSlotHistory> {
        if tps_raw_vec.len() < consts::TOKENS_PER_SLOT_HISTORY_LEN {
            let slack = consts::TOKENS_PER_SLOT_HISTORY_LEN - tps_raw_vec.len();
            let mut slack_vec: Vec<(u64, u64)> = vec![(0, 0); slack];
            tps_raw_vec.append(&mut slack_vec);
        }
        if tps_raw_vec.len() > consts::TOKENS_PER_SLOT_HISTORY_LEN {
            tps_raw_vec.truncate(consts::TOKENS_PER_SLOT_HISTORY_LEN)
        }

        let tps_vec = tps_raw_vec
            .iter()
            .map(|(slot, tps)| TokensPerSlotHistory {
                at: Slot::new(*slot),
                value: TokenAmount::new(*tps),
            })
            .collect();

        tps_vec
    }

    struct SyscallStubs {
        clock: Slot,
    }

    impl SyscallStubs {
        fn new(clock: Slot) -> Self {
            Self { clock }
        }
    }

    impl solana_sdk::program_stubs::SyscallStubs for SyscallStubs {
        fn sol_log(&self, message: &str) {
            println!("[LOG] {}", message);
        }

        fn sol_get_clock_sysvar(&self, var_addr: *mut u8) -> u64 {
            unsafe {
                // TODO: Not sure how this works really, but it seems to
                // convert the value to `Clock::get()?.slot`
                *var_addr = self.clock.slot as u8;
            }
            0
        }
    }
}
