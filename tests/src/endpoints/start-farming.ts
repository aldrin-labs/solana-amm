import { getOrCreateAssociatedTokenAccount } from "@solana/spl-token";
import { Keypair } from "@solana/web3.js";
import { expect } from "chai";
import { Farm } from "../farm";
import { Farmer } from "../farmer";
import {
  assertApproxCurrentSlot,
  errLogs,
  getCurrentSlot,
  payer,
  provider,
  sleep,
} from "../helpers";

export function test() {
  describe("start_farming", () => {
    let farm: Farm, farmer: Farmer;

    beforeEach("create farm", async () => {
      farm = await Farm.init();
    });

    beforeEach("create farmers", async () => {
      farmer = await Farmer.init(farm);
      await farmer.airdropStakeTokens();
    });

    it("fails if farmer doesn't match farm", async () => {
      const anotherFarm = await Farm.init();

      const logs = await errLogs(
        farmer.startFarming(10, { farm: anotherFarm.id })
      );

      expect(logs).to.contain("Farmer is set up for a different farm");
    });

    it("fails if authority doesn't own stake wallet", async () => {
      const logs = await errLogs(
        farmer.startFarming(10, { authority: Keypair.generate() })
      );

      expect(logs).to.contain("owner does not match");
    });

    it("fails if stake vault doesn't match farm", async () => {
      const logs = await errLogs(
        farmer.startFarming(10, { stakeVault: Keypair.generate().publicKey })
      );

      expect(logs).to.contain("A seeds constraint was violated");
    });

    it("authority doesn't have to be farmer's authority", async () => {
      const authority = Keypair.generate();
      const stakeWallet = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        payer,
        farm.stakeMint,
        authority.publicKey
      );
      await farm.airdropStakeTokens(stakeWallet.address);

      await farmer.startFarming(10, {
        stakeWallet: stakeWallet.address,
        authority,
      });

      const farmerInfo = await farmer.fetch();

      expect(farmerInfo.vested.amount.toNumber()).to.eq(10);
      await assertApproxCurrentSlot(farmerInfo.vestedAt);

      const { amount } = await farm.stakeVaultInfo();
      expect(Number(amount)).to.eq(10);
    });

    it("is fails if stake amount is zero", async () => {
      const logs = await errLogs(farmer.startFarming(0));

      expect(logs).to.contain(
        "The provided stake amount needs to be bigger than zero"
      );
    });

    it("adds farmer's funds to vested", async () => {
      await farmer.startFarming(10);
      const farmerInfo1 = await farmer.fetch();
      expect(farmerInfo1.vested.amount.toNumber()).to.eq(10);
      await assertApproxCurrentSlot(farmerInfo1.vestedAt);
      const { amount: amount1 } = await farm.stakeVaultInfo();
      expect(Number(amount1)).to.eq(10);

      await farmer.startFarming(10);
      const farmerInfo2 = await farmer.fetch();
      expect(farmerInfo2.vested.amount.toNumber()).to.eq(20);
      await assertApproxCurrentSlot(farmerInfo2.vestedAt);
      const { amount: amount2 } = await farm.stakeVaultInfo();
      expect(Number(amount2)).to.eq(20);
    });

    it("updates farmer's eligible harvest", async () => {
      const { mint: harvestMint } = await farm.addHarvest();

      const tokensPerSlot = 10;
      await farm.setMinSnapshotWindow(1);
      await farm.newHarvestPeriod(
        harvestMint,
        0,
        (await getCurrentSlot()) + 100,
        tokensPerSlot
      );
      await farm.takeSnapshot();

      await farmer.startFarming(10);
      await sleep(1000);
      await farm.takeSnapshot();
      const earningRewardsFromSlot = await getCurrentSlot();
      await sleep(1000);
      await farm.takeSnapshot();
      await sleep(1000);
      await farm.takeSnapshot();

      await farmer.startFarming(10);
      const earnedRewardsToSlot = await getCurrentSlot();

      const farmerInfo = await farmer.fetch();

      expect(farmerInfo.staked.amount.toNumber()).to.eq(10);
      expect(farmerInfo.vested.amount.toNumber()).to.eq(10);
      await assertApproxCurrentSlot(farmerInfo.vestedAt);
      const harvests = farmerInfo.harvests as any[];
      const { tokens } = harvests.find(
        (h) => h.mint.toString() === harvestMint.toString()
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
  });
}
