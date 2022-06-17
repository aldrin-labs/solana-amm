import { Keypair, PublicKey } from "@solana/web3.js";
import { Farm } from "../farm";
import { expect } from "chai";
import {
  airdrop,
  assertApproxCurrentSlot,
  getCurrentSlot,
  errLogs,
} from "../../helpers";

export function test() {
  describe("new_harvest_period", () => {
    const defTps = 10;

    let farm: Farm, harvestMint: PublicKey;

    beforeEach("creates farm", async () => {
      farm = await Farm.init();
    });

    beforeEach("creates harvest", async () => {
      const harvestData = await farm.addHarvest();

      harvestMint = harvestData.mint;
    });

    it("fails if admin signer mismatches farm admin", async () => {
      const admin = Keypair.generate();
      await airdrop(admin.publicKey);
      await farm.setFarmOwner(admin);

      const fakeAdmin = Keypair.generate();
      await airdrop(fakeAdmin.publicKey);

      const logs = await errLogs(
        farm.newHarvestPeriod(harvestMint, 0, 100, defTps, {
          admin: fakeAdmin,
        })
      );

      expect(logs).to.contain("FarmAdminMismatch");
    });

    it("fails if admin is not a signer", async () => {
      await farm.setFarmOwner(Keypair.generate());
      await expect(
        farm.newHarvestPeriod(harvestMint, 0, 100, defTps, {
          skipAdminSignature: true,
        })
      ).to.be.rejected;
    });

    it("fails if harvest mint is not valid", async () => {
      const fakeHarvestMint = Keypair.generate().publicKey;

      const logs = await errLogs(
        farm.newHarvestPeriod(fakeHarvestMint, 0, 100, defTps, {
          harvestVault: await farm.harvestVault(harvestMint),
          harvestWallet: await farm.adminHarvestWallet(harvestMint),
          depositTokens: false,
        })
      );
      expect(logs).to.contain("ConstraintSeeds");
    });

    it("fails if from slot is in the past", async () => {
      const currentSlot = await getCurrentSlot();

      const logs = await errLogs(
        farm.newHarvestPeriod(
          harvestMint,
          currentSlot - 5,
          currentSlot + 10,
          10
        )
      );
      expect(logs).to.contain("InvalidSlot");
    });

    it("fails with wrong signer pda", async () => {
      const fakePda = Keypair.generate().publicKey;

      const logs = await errLogs(
        farm.newHarvestPeriod(
          harvestMint,
          0,
          (await getCurrentSlot()) + 100,
          defTps,
          {
            signerPda: fakePda,
          }
        )
      );

      expect(logs).to.contain("A seeds constraint was violated");
    });

    it("interprets 0 as to start from current slot", async () => {
      const tps = defTps;
      const farmInfoBefore = await farm.fetch();
      await farm.newHarvestPeriod(
        harvestMint,
        0,
        (await getCurrentSlot()) + 100,
        tps
      );
      const farmInfoAfter = await farm.fetch();

      const harvestsBefore = farmInfoBefore.harvests as any[];
      const harvestsAfter = farmInfoAfter.harvests as any[];

      // expect that harvestMint identifies correctly the harvest at index 0,
      // both before and after newHarvestPeriod operation
      expect(harvestsBefore[0].mint).to.deep.eq(harvestMint);
      expect(harvestsAfter[0].mint).to.deep.eq(harvestMint);

      await assertApproxCurrentSlot(harvestsAfter[0].periods[0].startsAt);

      expect(harvestsAfter[0].periods[0].tps.amount.toNumber()).to.eq(tps);

      // everything other field should not change
      delete farmInfoAfter.harvests;
      delete farmInfoBefore.harvests;
      expect(farmInfoAfter).to.deep.eq(farmInfoBefore);
    });

    it("adds scheduled launch", async () => {
      const tps = defTps;
      const period1Length = 100;

      const currentSlot = await getCurrentSlot();
      const vaultBefore = await farm.harvestVaultAccount(harvestMint);
      expect(Number(vaultBefore.amount)).to.eq(0);
      await farm.newHarvestPeriod(
        harvestMint,
        currentSlot + 50,
        currentSlot + 50 + period1Length - 1,
        tps
      );
      const vaultAfterPeriod1 = await farm.harvestVaultAccount(harvestMint);
      expect(Number(vaultAfterPeriod1.amount)).to.eq(tps * period1Length);

      // deposits more tokens from admin wallet to harvest vault if the new
      // scheduled launch period is longer
      const period2Length = 200;
      await farm.newHarvestPeriod(
        harvestMint,
        currentSlot + 200,
        currentSlot + 200 + period2Length - 1,
        tps
      );
      const vaultAfterPeriod2 = await farm.harvestVaultAccount(harvestMint);
      expect(Number(vaultAfterPeriod2.amount)).to.eq(tps * period2Length);

      // returns funds if the period is shorter
      const period3Length = 50;
      await farm.newHarvestPeriod(
        harvestMint,
        currentSlot + 100,
        currentSlot + 100 + period3Length - 1,
        tps
      );
      const vaultAfterPeriod3 = await farm.harvestVaultAccount(harvestMint);
      expect(Number(vaultAfterPeriod3.amount)).to.eq(tps * period3Length);
    });
  });
}
