//! This could be moved to integration tests at `amm/tests/` eventually.

use crate::prelude::utils::set_clock;
use crate::prelude::*;
use serial_test::serial;

#[test]
#[serial]
fn it_updates_eligible_harvest_for_multiple_farmers() -> Result<()> {
    set_clock(Slot::new(0));

    let mut farm = Farm {
        min_snapshot_window_slots: 1,
        ..Default::default()
    };

    let harvest = Pubkey::new_unique();
    let tps = 100;
    farm.add_harvest(harvest, Pubkey::new_unique(), TokenAmount::new(tps))?;

    let mut farmer1 = Farmer::default();
    let mut farmer2 = Farmer::default();
    let mut farmer3 = Farmer::default();

    set_clock(Slot::new(1));
    farm.take_snapshot(Slot::new(1), TokenAmount::new(0))?;

    let total_staked = 100;
    farmer1.add_to_vested(TokenAmount::new(40))?;
    farmer2.add_to_vested(TokenAmount::new(40))?;
    farmer3.add_to_vested(TokenAmount::new(20))?;

    let mut farmers = vec![farmer1, farmer2, farmer3];

    // start earning harvest from slot 4
    set_clock(Slot::new(4));
    farm.take_snapshot(Slot::new(4), TokenAmount::new(total_staked))?;
    set_clock(Slot::new(8));
    farm.take_snapshot(Slot::new(8), TokenAmount::new(total_staked))?;

    farmers
        .iter_mut()
        .for_each(|f| f.check_vested_period_and_update_harvest(&farm).unwrap());

    set_clock(Slot::new(12));
    farm.take_snapshot(Slot::new(12), TokenAmount::new(total_staked))?;
    set_clock(Slot::new(14));

    // last slot to earn harvest for is 14
    farmers.iter_mut().for_each(|f| {
        f.check_vested_period_and_update_harvest(&farm).unwrap();
        assert_eq!(f.calculate_next_harvest_from.slot, 15);
    });

    // 4th, 5th, ..., 13th, 14th
    let harvest_for_slots = 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1;
    let total_harvest = harvest_for_slots * tps;

    farmers.iter_mut().for_each(|f| {
        let harvest = f.harvests.iter().find(|h| h.mint == harvest).unwrap();
        assert_eq!(
            harvest.tokens.amount,
            total_harvest * f.staked.amount / total_staked
        );
    });

    Ok(())
}
