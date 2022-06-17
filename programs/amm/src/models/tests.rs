//! This could be moved to integration tests at `amm/tests/` eventually.

use crate::prelude::*;

#[test]
fn it_updates_eligible_harvest_for_multiple_farmers() -> Result<()> {
    let mut farm = Farm {
        min_snapshot_window_slots: 1,
        ..Default::default()
    };

    let harvest = Pubkey::new_unique();
    let tps = 100;

    farm.add_harvest(harvest, Pubkey::new_unique())?;
    farm.new_harvest_period(
        Slot::new(0),
        harvest,
        (Slot::new(1), Slot::new(u64::MAX)),
        TokenAmount::new(tps),
    )?;
    farm.take_snapshot(Slot::new(1), TokenAmount::new(0))?;

    let mut farmer1 = Farmer {
        calculate_next_harvest_from: Slot::new(1),
        ..Default::default()
    };
    let mut farmer2 = Farmer {
        calculate_next_harvest_from: Slot::new(1),
        ..Default::default()
    };
    let mut farmer3 = Farmer {
        calculate_next_harvest_from: Slot::new(1),
        ..Default::default()
    };
    let total_staked = 100;
    farmer1.add_to_vested(Slot::new(1), TokenAmount::new(40))?;
    farmer2.add_to_vested(Slot::new(1), TokenAmount::new(40))?;
    farmer3.add_to_vested(Slot::new(1), TokenAmount::new(20))?;

    let mut farmers = vec![farmer1, farmer2, farmer3];

    // start earning harvest from slot 4
    farm.take_snapshot(Slot::new(4), TokenAmount::new(total_staked))?;
    farm.take_snapshot(Slot::new(8), TokenAmount::new(total_staked))?;

    farmers.iter_mut().for_each(|f| {
        f.check_vested_period_and_update_harvest(&farm, Slot::new(8))
            .unwrap()
    });

    farm.take_snapshot(Slot::new(12), TokenAmount::new(total_staked))?;

    // last slot to earn harvest for is 14
    farmers.iter_mut().for_each(|f| {
        f.check_vested_period_and_update_harvest(&farm, Slot::new(14))
            .unwrap();
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
