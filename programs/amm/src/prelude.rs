pub use crate::err::{self, AmmError};
pub use crate::models::*;
pub use crate::{consts, endpoints};
pub use anchor_lang::prelude::*;
pub use decimal::{Decimal, TryAdd, TryDiv, TryMul};

#[cfg(test)]
pub mod utils {
    use super::*;

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

    pub fn set_clock(slot: Slot) {
        assert!(slot.slot < 256);
        solana_sdk::program_stubs::set_syscall_stubs(Box::new(
            SyscallStubs::new(slot),
        ));
    }
}
