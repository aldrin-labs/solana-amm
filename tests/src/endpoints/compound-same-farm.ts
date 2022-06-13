import { amm, payer, provider, airdrop, errLogs, sleep } from "../helpers";
import { Keypair, PublicKey } from "@solana/web3.js";
import { createMint, mintTo, getAccount } from "@solana/spl-token";
import { expect } from "chai";
import { Farm } from "../farm";
import { Farmer } from "../farmer";

export function test() {
  describe("compound_same_farm", () => {
    const admin = Keypair.generate();
    let farm: Farm;

    before("airdrop to admin", async () => {
      await airdrop(admin.publicKey);
    });

    beforeEach("create farm", async () => {
      farm = await Farm.init({ adminKeypair: admin });
    });

    it("fails if farm is not whitelisted", async () => {
      const farmer = await Farmer.init(farm);
      const stakeVault = await farm.stakeVault();

      const harvest = await farm.addHarvest({
        harvestMint: farm.stakeMint,
      });

      const logs = await errLogs(
        farm.compoundSameFarm(farm.stakeMint, {
          farmer: await farmer.id(),
          harvestVault: harvest.vault,
          stakeVault,
        })
      );

      expect(logs).to.contain(
        "AnchorError caused by account: whitelist_compounding"
      );
      expect(logs).to.contain("AccountNotInitialized.");
    });

    it("fails if wrong whitelist pda is provided", async () => {
      const [wrongPda, _signerBumpSeed] = await PublicKey.findProgramAddress(
        [Buffer.from("wrong_prefix"), farm.id.toBytes(), farm.id.toBytes()],
        amm.programId
      );

      const farmer = await Farmer.init(farm);
      const stakeVault = await farm.stakeVault();

      const harvest = await farm.addHarvest({
        harvestMint: farm.stakeMint,
      });

      await farm.WhitelistFarmForCompounding({
        targetFarm: farm.id,
      });

      const logs = await errLogs(
        farm.compoundSameFarm(farm.stakeMint, {
          farmer: await farmer.id(),
          harvestVault: harvest.vault,
          whitelistCompounding: wrongPda,
          stakeVault,
        })
      );

      expect(logs).to.contain(
        "AnchorError caused by account: whitelist_compounding"
      );
      expect(logs).to.contain("AccountNotInitialized");
    });

    it("fails if wrong farm signer pda is provided", async () => {
      const [wrongPda, _correctBumpSeed] = await PublicKey.findProgramAddress(
        [Buffer.from("wrong prefix"), farm.id.toBytes()],
        amm.programId
      );

      const farmer = await Farmer.init(farm);
      const stakeVault = await farm.stakeVault();

      const harvest = await farm.addHarvest({
        harvestMint: farm.stakeMint,
      });

      await farm.WhitelistFarmForCompounding({
        targetFarm: farm.id,
      });

      const logs = await errLogs(
        farm.compoundSameFarm(farm.stakeMint, {
          farmer: await farmer.id(),
          harvestVault: harvest.vault,
          farmSignerPda: wrongPda,
          stakeVault,
        })
      );

      expect(logs).to.contain("AnchorError caused by account: farm_signer_pda");
      expect(logs).to.contain("ConstraintSeeds.");
    });

    it("fails if stake vault mint differs from harvest vault mint", async () => {
      const farmer = await Farmer.init(farm);
      const stakeVault = await farm.stakeVault();

      const wrongMint = await createMint(
        provider.connection,
        payer,
        Keypair.generate().publicKey,
        null,
        6
      );

      const harvest = await farm.addHarvest({
        harvestMint: wrongMint,
      });

      await farm.WhitelistFarmForCompounding({
        targetFarm: farm.id,
      });

      const logs = await errLogs(
        farm.compoundSameFarm(farm.stakeMint, {
          farmer: await farmer.id(),
          harvestVault: harvest.vault,
          stakeVault,
        })
      );

      expect(logs).to.contain(
        "Compounding is only possible if stake mint " +
          "is a harvestable mint of the farm as well"
      );
    });

    it("fails if farmer is setup for different farm", async () => {
      const anotherFarm = await Farm.init({
        keypair: Keypair.generate(),
        adminKeypair: admin,
      });

      const farmer = await Farmer.init(anotherFarm);
      const stakeVault = await farm.stakeVault();

      const harvest = await farm.addHarvest({
        harvestMint: farm.stakeMint,
      });

      await farm.WhitelistFarmForCompounding({
        targetFarm: farm.id,
      });

      const logs = await errLogs(
        farm.compoundSameFarm(farm.stakeMint, {
          farmer: await farmer.id(),
          harvestVault: harvest.vault,
          stakeVault,
        })
      );

      expect(logs).to.contain("Farmer is set up for a different farm");
    });

    it("works even if no tokens eligible to claim", async () => {
      const farmer = await Farmer.init(farm);
      const stakeVault = await farm.stakeVault();

      const harvest = await farm.addHarvest({
        harvestMint: farm.stakeMint,
      });

      await farm.WhitelistFarmForCompounding({
        targetFarm: farm.id,
      });

      // Airdrop fresh minted tokens to harvest.vault
      await mintTo(
        provider.connection,
        payer,
        harvest.mint,
        harvest.vault,
        admin,
        1_000
      );

      await farmer.airdropStakeTokens();

      let harvestVaultInfo = await getAccount(
        provider.connection,
        harvest.vault
      );

      let stakeVaultInfo = await getAccount(provider.connection, stakeVault);

      expect(Number(harvestVaultInfo.amount)).to.eq(1_000);
      expect(Number(stakeVaultInfo.amount)).to.eq(0);

      const tokensPerSlot = 10;
      await farm.setTokensPerSlot(harvest.mint, undefined, tokensPerSlot);
      await farm.setMinSnapshotWindow(1);
      await farm.takeSnapshot();

      await sleep(1000);
      await farm.takeSnapshot();
      await sleep(1000);
      await farm.takeSnapshot();

      await farm.compoundSameFarm(farm.stakeMint, {
        farmer: await farmer.id(),
        harvestVault: harvest.vault,
        stakeVault,
      });

      harvestVaultInfo = await getAccount(provider.connection, harvest.vault);

      stakeVaultInfo = await getAccount(provider.connection, stakeVault);

      expect(Number(harvestVaultInfo.amount)).to.eq(1_000);
      expect(Number(stakeVaultInfo.amount)).to.eq(0);
    });

    it("works", async () => {
      const farmer = await Farmer.init(farm);
      const stakeVault = await farm.stakeVault();

      const harvest = await farm.addHarvest({
        harvestMint: farm.stakeMint,
      });

      await farm.WhitelistFarmForCompounding({
        targetFarm: farm.id,
      });

      // Airdrop fresh minted tokens to harvest.vault
      await mintTo(
        provider.connection,
        payer,
        harvest.mint,
        harvest.vault,
        admin,
        1_000
      );

      await farmer.airdropStakeTokens();

      let harvestVaultInfo = await getAccount(
        provider.connection,
        harvest.vault
      );

      let stakeVaultInfo = await getAccount(provider.connection, stakeVault);

      expect(Number(harvestVaultInfo.amount)).to.eq(1_000);
      expect(Number(stakeVaultInfo.amount)).to.eq(0);

      const tokensPerSlot = 10;
      await farm.setTokensPerSlot(harvest.mint, undefined, tokensPerSlot);
      await farm.setMinSnapshotWindow(1);
      await farm.takeSnapshot();

      await farmer.startFarming(10);
      await sleep(1000);
      await farm.takeSnapshot();
      const earningRewardsFromSlot = await provider.connection.getSlot();
      await sleep(1000);
      await farm.takeSnapshot();
      await sleep(1000);
      await farm.takeSnapshot();

      await farmer.stopFarming(10);

      const earnedRewardsToSlot = await provider.connection.getSlot();

      const farmerInfo = await farmer.fetch();

      const harvests = farmerInfo.harvests as any[];
      const { tokens } = harvests.find(
        (h) => h.mint.toString() === harvest.mint.toString()
      );

      const estimatedRewards =
        (earnedRewardsToSlot - earningRewardsFromSlot) * tokensPerSlot;
      const actuaRewards = tokens.amount.toNumber();

      expect(actuaRewards).to.be.approximately(
        estimatedRewards,
        // there's a possibility that we will get different slot in our call
        // than the one that was active during the start farming
        tokensPerSlot
      );

      await farm.compoundSameFarm(farm.stakeMint, {
        farmer: await farmer.id(),
        harvestVault: harvest.vault,
        stakeVault,
      });

      harvestVaultInfo = await getAccount(provider.connection, harvest.vault);

      stakeVaultInfo = await getAccount(provider.connection, stakeVault);

      expect(Number(harvestVaultInfo.amount)).to.eq(1_000 - actuaRewards);
      expect(Number(stakeVaultInfo.amount)).to.eq(actuaRewards);
    });
  });
}
