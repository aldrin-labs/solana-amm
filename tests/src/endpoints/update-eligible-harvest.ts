import { expect } from "chai";
import { Farm } from "../farm";
import { Farmer } from "../farmer";
import { errLogs, provider, sleep } from "../helpers";

export function test() {
  describe("update_eligible_harvest", () => {
    let farm: Farm, farmer: Farmer;

    beforeEach("create farm", async () => {
      farm = await Farm.init();

      await farm.setMinSnapshotWindow(1);
    });

    beforeEach("create farmers", async () => {
      farmer = await Farmer.init(farm);
      await farmer.airdropStakeTokens();
    });

    it("fails if farmer doesn't match farm", async () => {
      const anotherFarm = await Farm.init();

      const logs = await errLogs(
        farmer.updateEligibleHarvest({ farm: anotherFarm.id })
      );

      expect(logs).to.contain("Farmer is set up for a different farm");
    });

    it("works", async () => {
      const tokensPerSlot = 10;
      const harvest = await farm.addHarvest({ tokensPerSlot });

      await farm.setTokensPerSlot(harvest.mint, undefined, tokensPerSlot);
      await farm.takeSnapshot();

      await farmer.airdropStakeTokens(10);

      await farmer.startFarming(10);
      await sleep(1000);
      await farm.takeSnapshot();
      const earningRewardsFromSlot = await provider.connection.getSlot();

      await sleep(1000);
      await farm.takeSnapshot();
      await sleep(1000);
      await farm.takeSnapshot();

      await farmer.updateEligibleHarvest();
      const earnedRewardsToSlot = await provider.connection.getSlot();

      const farmerInfo = await farmer.fetch();

      expect(farmerInfo.staked.amount.toNumber()).to.eq(10);
      expect(farmerInfo.vested.amount.toNumber()).to.eq(0);

      const harvests = farmerInfo.harvests as any[];
      const { tokens } = harvests.find(
        (h) => h.mint.toString() === harvest.mint.toString()
      );
      const earnedRewardsForSlots =
        earnedRewardsToSlot - earningRewardsFromSlot;
      expect(tokens.amount.toNumber()).to.be.approximately(
        earnedRewardsForSlots * tokensPerSlot,
        // there's a possibility that we will get different slot in our call
        // than the one that was active during the start farming
        tokensPerSlot
      );
    });

    it("works with multiple farmers", async () => {
      const farmer1 = farmer;
      const farmer2 = await Farmer.init(farm);
      await farmer2.airdropStakeTokens();
      const farmer3 = await Farmer.init(farm);
      await farmer3.airdropStakeTokens();

      const farmers = [farmer1, farmer2, farmer3];

      await farmer1.startFarming(10);
      await farmer2.startFarming(10);
      await farmer3.startFarming(20);
      const totalStaked = 40;
      const { amount: stakeVaultAmount } = await farm.stakeVaultInfo();
      expect(Number(stakeVaultAmount)).to.eq(totalStaked);

      const tokensPerSlot = 100;
      const harvest = await farm.addHarvest({ tokensPerSlot });

      // take first snapshot and get its slot
      await farm.takeSnapshot();
      const { snapshots } = await farm.fetch();
      const firstSnapshotStartsAtSlot =
        snapshots.ringBuffer[
          snapshots.ringBufferTip.toNumber()
        ].startedAt.slot.toNumber();

      await sleep(1000);
      await farm.takeSnapshot();
      await sleep(1000);
      await Promise.all(farmers.map((f) => f.updateEligibleHarvest()));

      await sleep(1000);
      await farm.takeSnapshot();
      await sleep(1000);
      await Promise.all(farmers.map((f) => f.updateEligibleHarvest()));

      await Promise.all(
        farmers.map(async (f) => {
          const farmerInfo = await f.fetch();

          const earnedRewardsForSlots =
            farmerInfo.calculateNextHarvestFrom.slot.toNumber() -
            firstSnapshotStartsAtSlot;

          const harvests = farmerInfo.harvests as any[];
          const { tokens } = harvests.find(
            (h) => h.mint.toString() === harvest.mint.toString()
          );
          const share = farmerInfo.staked.amount.toNumber() / totalStaked;
          const totalHarvest = earnedRewardsForSlots * tokensPerSlot;

          expect(tokens.amount.toNumber()).to.eq(share * totalHarvest);
        })
      );
    });
  });
}
