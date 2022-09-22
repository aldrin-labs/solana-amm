import {
  farming,
  payer,
  provider,
  airdrop,
  errLogs,
  sleep,
  getCurrentSlot,
} from "../../helpers";
import { Keypair, PublicKey } from "@solana/web3.js";
import { createMint, getAccount } from "@solana/spl-token";
import { expect } from "chai";
import { Farm } from "../farm";
import { Farmer } from "../farmer";

export function test() {
  describe("compound_across_farm", () => {
    const admin = Keypair.generate();
    let sourceFarm: Farm;
    let targetFarm: Farm;

    before("airdrop to admin", async () => {
      await airdrop(admin.publicKey);
    });

    beforeEach("create farms", async () => {
      sourceFarm = await Farm.init({ adminKeypair: admin });
      targetFarm = await Farm.init({ adminKeypair: admin });
    });

    it("fails if farm is not whitelisted", async () => {
      const sourceFarmer = await Farmer.init(sourceFarm);
      const targetFarmer = await Farmer.init(targetFarm);
      const stakeVault = await targetFarm.stakeVault();

      const harvest = await sourceFarm.addHarvest({
        harvestMint: targetFarm.stakeMint,
      });

      const logs = await errLogs(
        sourceFarm.compoundAcrossFarms(targetFarm.stakeMint, {
          targetFarm: targetFarm.id,
          sourceFarmer: await sourceFarmer.id(),
          targetFarmer: await targetFarmer.id(),
          sourceHarvestVault: harvest.vault,
          targetStakeVault: stakeVault,
        })
      );

      expect(logs).to.contain(
        "AnchorError caused by account: whitelist_compounding"
      );
      expect(logs).to.contain("AccountNotInitialized.");
    });

    it("fails if wrong whitelist pda is provided", async () => {
      const [wrongPda, _signerBumpSeed] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("wrong_prefix"),
          sourceFarm.id.toBytes(),
          targetFarm.id.toBytes(),
        ],
        farming.programId
      );

      const sourceFarmer = await Farmer.init(sourceFarm);
      const targetFarmer = await Farmer.init(targetFarm);
      const stakeVault = await targetFarm.stakeVault();

      const harvest = await sourceFarm.addHarvest({
        harvestMint: targetFarm.stakeMint,
      });

      await sourceFarm.whitelistFarmForCompounding({
        targetFarm: targetFarm.id,
      });

      const logs = await errLogs(
        sourceFarm.compoundAcrossFarms(targetFarm.stakeMint, {
          targetFarm: targetFarm.id,
          sourceFarmer: await sourceFarmer.id(),
          targetFarmer: await targetFarmer.id(),
          sourceHarvestVault: harvest.vault,
          targetStakeVault: stakeVault,
          whitelistCompounding: wrongPda,
        })
      );

      expect(logs).to.contain(
        "AnchorError caused by account: whitelist_compounding"
      );
      expect(logs).to.contain("AccountNotInitialized");
    });

    it("fails if wrong farm signer pda is provided", async () => {
      const [wrongPda, _wrongBumpSeed] = PublicKey.findProgramAddressSync(
        [Buffer.from("wrong prefix"), sourceFarm.id.toBytes()],
        farming.programId
      );

      const sourceFarmer = await Farmer.init(sourceFarm);
      const targetFarmer = await Farmer.init(targetFarm);
      const stakeVault = await targetFarm.stakeVault();

      const harvest = await sourceFarm.addHarvest({
        harvestMint: targetFarm.stakeMint,
      });

      await sourceFarm.whitelistFarmForCompounding({
        targetFarm: targetFarm.id,
      });

      const logs = await errLogs(
        sourceFarm.compoundAcrossFarms(targetFarm.stakeMint, {
          targetFarm: targetFarm.id,
          sourceFarmer: await sourceFarmer.id(),
          targetFarmer: await targetFarmer.id(),
          sourceHarvestVault: harvest.vault,
          targetStakeVault: stakeVault,
          sourceFarmSignerPda: wrongPda,
        })
      );

      expect(logs).to.contain(
        "AnchorError caused by account: source_farm_signer_pda"
      );
      expect(logs).to.contain("ConstraintSeeds.");
    });

    it("fails if stake vault mint differs from harvest vault mint", async () => {
      const sourceFarmer = await Farmer.init(sourceFarm);
      const targetFarmer = await Farmer.init(targetFarm);
      const stakeVault = await targetFarm.stakeVault();

      const wrongMint = await createMint(
        provider.connection,
        payer,
        Keypair.generate().publicKey,
        null,
        6
      );

      const harvest = await sourceFarm.addHarvest({
        harvestMint: wrongMint,
      });

      await sourceFarm.whitelistFarmForCompounding({
        targetFarm: targetFarm.id,
      });

      const logs = await errLogs(
        sourceFarm.compoundAcrossFarms(targetFarm.stakeMint, {
          targetFarm: targetFarm.id,
          sourceFarmer: await sourceFarmer.id(),
          targetFarmer: await targetFarmer.id(),
          sourceHarvestVault: harvest.vault,
          targetStakeVault: stakeVault,
        })
      );

      expect(logs).to.contain(
        "Compounding is only possible if stake mint " +
          "is a harvestable mint of the farm as well"
      );
    });

    it("fails if farms are the same", async () => {
      const sourceFarmer = await Farmer.init(sourceFarm);
      const targetFarmer = await Farmer.init(sourceFarm);
      const stakeVault = await targetFarm.stakeVault();

      const harvest = await sourceFarm.addHarvest({
        harvestMint: sourceFarm.stakeMint,
      });

      await sourceFarm.whitelistFarmForCompounding({
        targetFarm: sourceFarm.id,
      });

      const logs = await errLogs(
        sourceFarm.compoundAcrossFarms(sourceFarm.stakeMint, {
          targetFarm: sourceFarm.id,
          sourceFarmer: await sourceFarmer.id(),
          targetFarmer: await targetFarmer.id(),
          sourceHarvestVault: harvest.vault,
          targetStakeVault: stakeVault,
        })
      );

      expect(logs).to.contain(
        "This endpoint cannot be used to compound the same farm"
      );
    });

    it("fails if farmers have different owners", async () => {
      const sourceFarmer = await Farmer.init(sourceFarm, {
        authority: admin,
      });
      const targetFarmer = await Farmer.init(targetFarm, {
        authority: Keypair.generate(),
      });

      const targetStakeVault = await targetFarm.stakeVault();

      const sourceHarvest = await sourceFarm.addHarvest({
        harvestMint: targetFarm.stakeMint,
      });

      await sourceFarm.whitelistFarmForCompounding({
        targetFarm: targetFarm.id,
      });

      const logs = await errLogs(
        sourceFarm.compoundAcrossFarms(targetFarm.stakeMint, {
          targetFarm: targetFarm.id,
          sourceFarmer: await sourceFarmer.id(),
          targetFarmer: await targetFarmer.id(),
          sourceHarvestVault: sourceHarvest.vault,
          targetStakeVault,
        })
      );

      expect(logs).to.contain(
        "Source and target farmer must be of the same user"
      );
    });

    it("works even if no tokens eligible to claim", async () => {
      const sourceFarmer = await Farmer.init(sourceFarm, {
        authority: admin,
      });
      const targetFarmer = await Farmer.init(targetFarm, {
        authority: admin,
      });
      const sourceStakeVault = await sourceFarm.stakeVault();
      const targetStakeVault = await targetFarm.stakeVault();

      const sourceHarvest = await sourceFarm.addHarvest({
        harvestMint: targetFarm.stakeMint,
      });

      await sourceFarm.whitelistFarmForCompounding({
        targetFarm: targetFarm.id,
      });

      await sourceFarmer.airdropStakeTokens();

      const sourceStakeVaultInfo = await getAccount(
        provider.connection,
        sourceStakeVault
      );
      expect(Number(sourceStakeVaultInfo.amount)).to.eq(0);

      const tps = 10;
      await sourceFarm.newHarvestPeriod(sourceHarvest.mint, 0, 100, tps);
      await sourceFarm.setMinSnapshotWindow(1);
      await sourceFarm.takeSnapshot();

      await sleep(1000);
      await sourceFarm.takeSnapshot();

      const sourceHarvestVaultBeforeInfo = await getAccount(
        provider.connection,
        sourceHarvest.vault
      );
      await sourceFarm.compoundAcrossFarms(targetFarm.stakeMint, {
        targetFarm: targetFarm.id,
        sourceFarmer: await sourceFarmer.id(),
        targetFarmer: await targetFarmer.id(),
        sourceHarvestVault: sourceHarvest.vault,
        targetStakeVault,
      });

      const sourceHarvestVaultAfterInfo = await getAccount(
        provider.connection,
        sourceHarvest.vault
      );
      expect(sourceHarvestVaultBeforeInfo.amount).to.eq(
        sourceHarvestVaultAfterInfo.amount
      );

      const targetStakeVaultInfo = await getAccount(
        provider.connection,
        targetStakeVault
      );
      expect(Number(targetStakeVaultInfo.amount)).to.eq(0);
    });

    it("works", async () => {
      const sourceFarmer = await Farmer.init(sourceFarm, {
        authority: admin,
      });
      const targetFarmer = await Farmer.init(targetFarm, {
        authority: admin,
      });
      const sourceStakeVault = await sourceFarm.stakeVault();
      const targetStakeVault = await targetFarm.stakeVault();

      const sourceHarvest = await sourceFarm.addHarvest({
        harvestMint: targetFarm.stakeMint,
      });

      await sourceFarm.whitelistFarmForCompounding({
        targetFarm: targetFarm.id,
      });

      await sourceFarmer.airdropStakeTokens();

      const sourceStakeVaultInfo = await getAccount(
        provider.connection,
        sourceStakeVault
      );

      expect(Number(sourceStakeVaultInfo.amount)).to.eq(0);

      const tps = 10;
      await sourceFarm.newHarvestPeriod(sourceHarvest.mint, 0, 100, tps);
      const sourceHarvestVaultBeforeInfo = await getAccount(
        provider.connection,
        sourceHarvest.vault
      );

      await sourceFarm.setMinSnapshotWindow(1);
      await sourceFarm.takeSnapshot();

      await sourceFarmer.startFarming(10);
      await sleep(1000);
      await sourceFarm.takeSnapshot();
      const earningRewardsFromSlot = await getCurrentSlot();
      await sleep(1000);
      await sourceFarm.takeSnapshot();
      await sleep(1000);
      await sourceFarm.takeSnapshot();

      await sourceFarmer.stopFarming(10);

      const earnedRewardsToSlot = await getCurrentSlot();

      const sourceFarmerInfo = await sourceFarmer.fetch();

      const harvests = sourceFarmerInfo.harvests as any[];
      const { tokens } = harvests.find(
        (h) => h.mint.toString() === sourceHarvest.mint.toString()
      );

      const estimatedRewards =
        (earnedRewardsToSlot - earningRewardsFromSlot) * tps;
      const actualRewards = tokens.amount.toNumber();

      expect(actualRewards).to.be.approximately(
        estimatedRewards,
        // there's a possibility that we will get different slot in our call
        // than the one that was active during the start farming
        tps
      );

      await sourceFarm.compoundAcrossFarms(targetFarm.stakeMint, {
        targetFarm: targetFarm.id,
        sourceFarmer: await sourceFarmer.id(),
        targetFarmer: await targetFarmer.id(),
        sourceHarvestVault: sourceHarvest.vault,
        targetStakeVault,
      });

      const sourceHarvestVaultAfterInfo = await getAccount(
        provider.connection,
        sourceHarvest.vault
      );
      const targetStakeVaultInfo = await getAccount(
        provider.connection,
        targetStakeVault
      );

      expect(Number(sourceHarvestVaultBeforeInfo.amount)).to.eq(
        Number(sourceHarvestVaultAfterInfo.amount) + actualRewards
      );
      expect(Number(targetStakeVaultInfo.amount)).to.eq(actualRewards);
    });
  });
}
