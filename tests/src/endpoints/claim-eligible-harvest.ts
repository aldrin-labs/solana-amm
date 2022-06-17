import { expect } from "chai";
import { Keypair, PublicKey } from "@solana/web3.js";
import { getAccount } from "@solana/spl-token";
import { Farm } from "../farm";
import { Farmer } from "../farmer";
import { errLogs, getCurrentSlot, provider, sleep } from "../helpers";

export function test() {
  describe("claim_eligible_harvest", () => {
    const tokensPerSlot1 = 100,
      tokensPerSlot2 = 200;

    let farm: Farm,
      farmer: Farmer,
      harvest1: { mint: PublicKey; vault: PublicKey },
      harvest2: { mint: PublicKey; vault: PublicKey },
      farmerVaultWalletPairs: [PublicKey, PublicKey][];

    beforeEach("create farm", async () => {
      farm = await Farm.init();

      await farm.setMinSnapshotWindow(1);
    });

    beforeEach("create farmer", async () => {
      farmer = await Farmer.init(farm);
      await farmer.airdropStakeTokens();
    });

    beforeEach("create harvests", async () => {
      harvest1 = await farm.addHarvest();
      await farm.newHarvestPeriod(
        harvest1.mint,
        0,
        (await getCurrentSlot()) + 100,
        tokensPerSlot1
      );

      harvest2 = await farm.addHarvest();
      await farm.newHarvestPeriod(
        harvest2.mint,
        0,
        (await getCurrentSlot()) + 100,
        tokensPerSlot2
      );

      farmerVaultWalletPairs = [
        [harvest1.vault, await farmer.harvestWalletPubkey(harvest1.mint)],
        [harvest2.vault, await farmer.harvestWalletPubkey(harvest2.mint)],
      ];
    });

    it("fails if authority doesn't sign", async () => {
      await expect(
        farmer.claimEligibleHarvest(farmerVaultWalletPairs, {
          skipAuthoritySignature: true,
        })
      ).to.be.rejectedWith(/signature verification failed/i);
    });

    it("fails if authority doesn't own farmer", async () => {
      const logs = await errLogs(
        farmer.claimEligibleHarvest(farmerVaultWalletPairs, {
          authority: Keypair.generate(),
        })
      );

      expect(logs).to.contain("A seeds constraint was violated");
    });

    it("fails if staking vault is used as harvest vault", async () => {
      const harvest = await farm.addHarvest({
        harvestMint: farm.stakeMint,
      });
      await farm.newHarvestPeriod(
        harvest.mint,
        0,
        (await getCurrentSlot()) + 100,
        100
      );

      await farmer.startFarming(10);
      await sleep(1000);
      await farm.takeSnapshot();
      await farmer.stopFarming(10);

      const logs = await errLogs(
        farmer.claimEligibleHarvest([
          [await farm.stakeVault(), (await farmer.stakeWallet()).address],
        ])
      );

      expect(logs).to.contain("[InvalidAccountInput] Harvest vault");
    });

    it("fails if wrong singer pda is provided", async () => {
      const logs = await errLogs(
        farmer.claimEligibleHarvest(farmerVaultWalletPairs, {
          farmSignerPda: Keypair.generate().publicKey,
        })
      );

      expect(logs).to.contain("A seeds constraint was violated");
    });

    it("fails if remaining accounts are empty", async () => {
      const logs = await errLogs(farmer.claimEligibleHarvest([]));

      expect(logs).to.contain(
        "[InvalidAccountInput] Remaining accounts must come in pairs"
      );
    });

    it("works", async () => {
      await farm.takeSnapshot();

      await farmer.airdropStakeTokens(10);
      await farmer.startFarming(10);

      await sleep(1000);
      await farm.takeSnapshot();
      await sleep(1000);
      await farm.takeSnapshot();

      await farmer.stopFarming(10);

      const VaultWalletPairsBefore = await Promise.all(
        farmerVaultWalletPairs.map(async ([vault, wallet]) => {
          const vaultInfo = await getAccount(provider.connection, vault);
          const walletInfo = await getAccount(provider.connection, wallet);

          return [Number(vaultInfo.amount), Number(walletInfo.amount)];
        })
      );

      const farmerInfoBefore = await farmer.fetch();
      const harvestsBefore = farmerInfoBefore.harvests as any[];

      await farmer.claimEligibleHarvest(farmerVaultWalletPairs);

      const farmerInfoAfter = await farmer.fetch();
      const harvestsAfter = farmerInfoAfter.harvests as any[];

      expect(
        harvestsAfter
          .find((h) => h.mint.toBase58() === harvest1.mint.toBase58())
          .tokens.amount.toNumber()
      ).to.eq(0);
      expect(
        harvestsAfter
          .find((h) => h.mint.toBase58() === harvest2.mint.toBase58())
          .tokens.amount.toNumber()
      ).to.eq(0);

      await Promise.all(
        farmerVaultWalletPairs.map(async ([vault, wallet], i) => {
          const vaultInfo = await getAccount(provider.connection, vault);
          const walletInfo = await getAccount(provider.connection, wallet);

          const [vaultAmountBefore, walletAmountBefore] =
            VaultWalletPairsBefore[i];

          const [vaultAmountAfter, walletAmountAfter] = [
            Number(vaultInfo.amount),
            Number(walletInfo.amount),
          ];

          expect(walletAmountBefore).to.eq(0);
          expect(vaultAmountBefore).to.eq(vaultAmountAfter + walletAmountAfter);

          expect(
            harvestsBefore
              .find((h) => h.mint.toBase58() === vaultInfo.mint.toBase58())
              .tokens.amount.toNumber()
          ).to.eq(walletAmountAfter);
        })
      );
    });
  });
}
