pub use crate::err::{self, FarmingError};
pub use crate::models::*;
pub use crate::{consts, endpoints};
pub use anchor_lang::prelude::*;
pub use decimal::{Decimal, TryAdd, TryDiv, TryMul, TryRound};

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

        let snapshots_vec: Vec<Snapshot> = snapshots_raw_vec
            .iter()
            .map(|(slot, staked)| Snapshot {
                started_at: Slot { slot: *slot },
                staked: TokenAmount { amount: *staked },
            })
            .collect();

        snapshots_vec
    }

    /// Returns a harvest mint pubkey added to a farm.
    ///
    /// The farm has 4 periods:
    /// 1. (1, 3) with tps 1
    /// 2. (5, 9) with tps 10
    /// 3. (10, 14) with tps 20
    /// 4. (15, 30) with tps 30
    ///
    /// And 4. snapshots:
    /// 1. slot 1 with staked 100
    /// 1. slot 7 with staked 100
    /// 1. slot 10 with staked 200
    /// 1. slot 15 with staked 400
    pub fn dummy_farm_1() -> Result<(Pubkey, Farm)> {
        let harvest_mint = Pubkey::new_unique();
        let mut farm = Farm::default();
        farm.min_snapshot_window_slots = 1;
        farm.add_harvest(harvest_mint, Pubkey::new_unique())?;

        farm.take_snapshot(Slot::new(1), TokenAmount::new(100))?;
        farm.new_harvest_period(
            Slot::new(1),
            harvest_mint,
            (Slot::new(1), Slot::new(3)),
            TokenAmount::new(1),
        )?;

        farm.take_snapshot(Slot::new(7), TokenAmount::new(100))?;
        farm.new_harvest_period(
            Slot::new(5),
            harvest_mint,
            (Slot::new(5), Slot::new(9)),
            TokenAmount::new(10),
        )?;
        farm.take_snapshot(Slot::new(10), TokenAmount::new(200))?;
        farm.new_harvest_period(
            Slot::new(10),
            harvest_mint,
            (Slot::new(10), Slot::new(14)),
            TokenAmount::new(20),
        )?;
        farm.take_snapshot(Slot::new(15), TokenAmount::new(400))?;
        farm.new_harvest_period(
            Slot::new(15),
            harvest_mint,
            (Slot::new(15), Slot::new(30)),
            TokenAmount::new(30),
        )?;

        Ok((harvest_mint, farm))
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

    pub fn generate_harvest_periods(
        periods_raw_vec: &mut Vec<(u64, u64, u64)>,
    ) -> Vec<HarvestPeriod> {
        if periods_raw_vec.len() < consts::HARVEST_PERIODS_LEN {
            let slack = consts::HARVEST_PERIODS_LEN - periods_raw_vec.len();
            let mut slack_vec: Vec<(u64, u64, u64)> = vec![(0, 0, 0); slack];
            periods_raw_vec.append(&mut slack_vec);
        }

        let snapshots_vec: Vec<HarvestPeriod> = periods_raw_vec
            .iter()
            .map(|(tps, starts_at, ends_at)| HarvestPeriod {
                tps: TokenAmount { amount: *tps },
                starts_at: Slot { slot: *starts_at },
                ends_at: Slot { slot: *ends_at },
            })
            .collect();

        snapshots_vec
    }

    pub fn generate_farm_harvests(
        harvests_raw_vec: &mut Vec<(
            Pubkey,
            Pubkey,
            [HarvestPeriod; consts::HARVEST_PERIODS_LEN],
        )>,
    ) -> Vec<Harvest> {
        if harvests_raw_vec.len() < consts::MAX_HARVEST_MINTS {
            let slack = consts::MAX_HARVEST_MINTS - harvests_raw_vec.len();
            let mut slack_vec: Vec<(
                Pubkey,
                Pubkey,
                [HarvestPeriod; consts::HARVEST_PERIODS_LEN],
            )> = vec![
                (
                    Default::default(),
                    Default::default(),
                    [HarvestPeriod::default(); 10]
                );
                slack
            ];
            harvests_raw_vec.append(&mut slack_vec);
        }

        let snapshots_vec: Vec<Harvest> = harvests_raw_vec
            .iter()
            .map(|(mint, vault, periods)| Harvest {
                mint: *mint,
                vault: *vault,
                periods: *periods,
            })
            .collect();

        snapshots_vec
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
