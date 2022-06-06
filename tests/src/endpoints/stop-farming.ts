import { Keypair } from "@solana/web3.js";
import { expect } from "chai";
import { Farm } from "../farm";
import { Farmer } from "../farmer";
import { getAccount, getMint } from "@solana/spl-token";
import { errLogs, provider, sleep, amm } from "../helpers";

export function test() {
  describe("stop_farming", () => {
    let farm: Farm, farmer: Farmer;

    beforeEach("create farm", async () => {
      farm = await Farm.init();
    });

    beforeEach("create farmers", async () => {
      farmer = await Farmer.init(farm);
    });

    it("fails if farmer doesn't match farm", async () => {
      const anotherFarm = await Farm.init();

      const logs = await errLogs(
        farmer.stopFarming(10, { farm: anotherFarm.id })
      );

      expect(logs).to.contain("A seeds constraint was violated");
    });

    it("fails if authority does not sign transaction", async () => {
      await expect(
        farmer.stopFarming(10, {
          skipAuthoritySignature: true,
        })
      ).to.be.rejected;
    });

    it("fails if stake vault doesn't match farm", async () => {
      const logs = await errLogs(
        farmer.stopFarming(10, { stakeVault: Keypair.generate().publicKey })
      );

      expect(logs).to.contain("A seeds constraint was violated");
    });

    it("is fails if stake amount is zero", async () => {
      const logs = await errLogs(farmer.stopFarming(0));

      expect(logs).to.contain(
        "The provided unstake maximum amount needs to be bigger than zero"
      );
    });

    it("if updates even if unstake max amount > stake amount", async () => {
      await farmer.airdropStakeTokens(10);

      await farmer.startFarming(10);
      const farmerInfoBefore = await farmer.fetch();

      await farmer.stopFarming(100);
      const farmerInfoAfter = await farmer.fetch();

      expect(farmerInfoBefore.vested.amount.toNumber()).to.eq(10);
      expect(farmerInfoBefore.staked.amount.toNumber()).to.eq(0);

      expect(farmerInfoAfter.staked.amount.toNumber()).to.eq(0);
      expect(farmerInfoAfter.staked.amount.toNumber()).to.eq(0);
    });

    it("correctly refreshes when stoping farming", async () => {
      await farmer.airdropStakeTokens(20);
      await farm.setMinSnapshotWindow(1);

      await farm.takeSnapshot();

      await farmer.startFarming(20);
      const farmerInfo1 = await farmer.fetch();
      expect(farmerInfo1.vested.amount.toNumber()).to.eq(20);
      const { amount: amount1 } = await farm.stakeVaultInfo();
      expect(Number(amount1)).to.eq(20);

      await sleep(1000);
      await farm.takeSnapshot();

      await farmer.stopFarming(10);
      const farmerInfo2 = await farmer.fetch();

      expect(farmerInfo2.vested.amount.toNumber()).to.eq(0);
      expect(farmerInfo2.staked.amount.toNumber()).to.eq(10);
    });

    it("updates farmer's eligible harvest", async () => {
      await farmer.airdropStakeTokens(20);
      const tokensPerSlot = 10;
      await farm.setMinSnapshotWindow(1);
      const harvest = await farm.addHarvest();
      await farm.setTokensPerSlot(harvest.mint, undefined, tokensPerSlot);
      await farm.takeSnapshot();

      await farmer.startFarming(20);

      await sleep(1000);
      await farm.takeSnapshot();
      const earningRewardsFromSlot = await provider.connection.getSlot();
      await sleep(1000);
      await farm.takeSnapshot();
      await sleep(1000);
      await farm.takeSnapshot();

      await farmer.stopFarming(10);
      const earnedRewardsToSlot = await provider.connection.getSlot();
      sleep(1000);
      await farm.takeSnapshot();

      const farmerInfoAfter = await farmer.fetch();
      expect(farmerInfoAfter.staked.amount.toNumber()).to.eq(10);
      expect(farmerInfoAfter.vested.amount.toNumber()).to.eq(0);

      const harvests = farmerInfoAfter.harvests as any[];
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

    it("it fails if wrong farm_signer_pda is provided", async () => {
      await farmer.airdropStakeTokens(10);

      await farmer.startFarming(10);

      const logs = await errLogs(
        farmer.stopFarming(10, {
          farmSignerPda: Keypair.generate().publicKey,
        })
      );

      expect(logs).to.contain("A seeds constraint was violated.");
    });

    it("it works", async () => {
      let stakeMint = await getMint(provider.connection, await farm.stakeMint);
      let stakeWallet = await getAccount(
        provider.connection,
        await (
          await farmer.stakeWallet()
        ).address
      );
      let stakeVault = await getAccount(
        provider.connection,
        await farm.stakeVault()
      );

      expect(Number(stakeVault.amount)).to.eq(0);
      expect(Number(stakeWallet.amount)).to.eq(0);
      expect(Number(stakeMint.supply)).to.eq(0);

      await farmer.airdropStakeTokens(10);

      stakeMint = await getMint(provider.connection, await farm.stakeMint);
      stakeWallet = await getAccount(
        provider.connection,
        await (
          await farmer.stakeWallet()
        ).address
      );
      stakeVault = await getAccount(
        provider.connection,
        await farm.stakeVault()
      );

      expect(Number(stakeVault.amount)).to.eq(0);
      expect(Number(stakeWallet.amount)).to.eq(10);
      expect(Number(stakeMint.supply)).to.eq(10);

      await farmer.startFarming(10);
      const farmerInfoBefore = await farmer.fetch();

      stakeVault = await getAccount(
        provider.connection,
        await farm.stakeVault()
      );
      stakeWallet = await getAccount(
        provider.connection,
        await (
          await farmer.stakeWallet()
        ).address
      );
      stakeMint = await getMint(provider.connection, await farm.stakeMint);

      expect(Number(stakeVault.amount)).to.eq(10);
      expect(Number(stakeWallet.amount)).to.eq(0);
      expect(Number(stakeMint.supply)).to.eq(10);

      await farmer.stopFarming(10);
      const farmerInfoAfter = await farmer.fetch();

      stakeVault = await getAccount(
        provider.connection,
        await farm.stakeVault()
      );
      stakeWallet = await getAccount(
        provider.connection,
        await (
          await farmer.stakeWallet()
        ).address
      );
      stakeMint = await getMint(provider.connection, await farm.stakeMint);

      expect(Number(stakeVault.amount)).to.eq(0);
      expect(Number(stakeWallet.amount)).to.eq(10);
      expect(Number(stakeMint.supply)).to.eq(10);

      expect(farmerInfoBefore.vested.amount.toNumber()).to.eq(10);
      expect(farmerInfoBefore.staked.amount.toNumber()).to.eq(0);

      expect(farmerInfoAfter.staked.amount.toNumber()).to.eq(0);
      expect(farmerInfoAfter.staked.amount.toNumber()).to.eq(0);
    });
  });
}
