import { airdrop, provider, amm, errLogs } from "../helpers";
import { Keypair, PublicKey } from "@solana/web3.js";
import { expect } from "chai";
import { Farm } from "../farm";

export function test() {
  describe("whitelist_farm_for_compounding", () => {
    const admin = Keypair.generate();
    const targetAdmin = Keypair.generate();
    let sourceFarm: Farm;

    before("airdrop to admin", async () => {
      await airdrop(admin.publicKey);
      await airdrop(targetAdmin.publicKey);
    });

    beforeEach("create farm", async () => {
      sourceFarm = await Farm.init({ adminKeypair: admin });
    });

    it("fails if admin signer mismatches farm", async () => {
      const targetFarm = await Farm.init({ adminKeypair: targetAdmin });

      const fakeAdmin = Keypair.generate();
      await airdrop(fakeAdmin.publicKey);

      const logs = await errLogs(
        sourceFarm.WhitelistFarmForCompounding({
          targetFarm: targetFarm.id,
          admin: fakeAdmin,
        })
      );

      expect(logs).to.contain("FarmAdminMismatch");

      // Assert that PDA account has not been created
      const whitelistPda = await sourceFarm.findWhitelistPda(targetFarm.id);
      const whitelistPdaAccount = await provider.connection.getAccountInfo(
        whitelistPda
      );

      expect(whitelistPdaAccount).to.eq(null);
    });

    it("fails if admin has not signed transaction", async () => {
      const targetFarm = await Farm.init({ adminKeypair: targetAdmin });

      await expect(
        sourceFarm.WhitelistFarmForCompounding({
          skipAdminSignature: true,
          targetFarm: targetFarm.id,
        })
      ).to.be.rejected;

      // Assert that PDA account has not been created
      const whitelistPda = await sourceFarm.findWhitelistPda(targetFarm.id);
      const whitelistPdaAccount = await provider.connection.getAccountInfo(
        whitelistPda
      );

      expect(whitelistPdaAccount).to.eq(null);
    });

    it("creates two distinct pdas when farm \
        A whitelists farm B and vice-versa", async () => {
      const targetFarm = await Farm.init({ adminKeypair: targetAdmin });

      await sourceFarm.WhitelistFarmForCompounding({
        targetFarm: targetFarm.id,
      });

      await targetFarm.WhitelistFarmForCompounding({
        targetFarm: sourceFarm.id,
      });

      const sourceToTargetPda = await sourceFarm.findWhitelistPda(
        targetFarm.id
      );
      const sourceToTargetPdaAccount = await provider.connection.getAccountInfo(
        sourceToTargetPda
      );

      const targetToSourcePda = await targetFarm.findWhitelistPda(
        sourceFarm.id
      );
      const targetToSourcePdaAccount = await provider.connection.getAccountInfo(
        targetToSourcePda
      );

      expect(sourceToTargetPdaAccount).not.to.be.eq(targetToSourcePdaAccount);
    });

    it("works when source and target farm as the same", async () => {
      await sourceFarm.WhitelistFarmForCompounding({
        targetFarm: sourceFarm.id,
      });

      const whitelistPda = await sourceFarm.findWhitelistPda(sourceFarm.id);
      const whitelistPdaAccount = await provider.connection.getAccountInfo(
        whitelistPda
      );

      expect(whitelistPdaAccount).not.to.eq(null);
    });

    it("works", async () => {
      const targetFarm = await Farm.init({ adminKeypair: targetAdmin });

      await sourceFarm.WhitelistFarmForCompounding({
        targetFarm: targetFarm.id,
      });

      const whitelistPda = await sourceFarm.findWhitelistPda(targetFarm.id);
      const whitelistPdaAccount = await provider.connection.getAccountInfo(
        whitelistPda
      );

      expect(whitelistPdaAccount).not.to.eq(null);
    });
  });
}
