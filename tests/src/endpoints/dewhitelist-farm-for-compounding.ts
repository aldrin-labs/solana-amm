import { airdrop, provider, amm, errLogs } from "../helpers";
import { Keypair, PublicKey } from "@solana/web3.js";
import { expect } from "chai";
import { Farm } from "../farm";

export function test() {
  describe("dewhitelist_farm_for_compounding", () => {
    const admin = Keypair.generate();
    const targetAdmin = Keypair.generate();
    let sourceFarm: Farm;
    let targetFarm: Farm;

    before("airdrop to admin", async () => {
      await airdrop(admin.publicKey);
      await airdrop(targetAdmin.publicKey);
    });

    beforeEach("create farm", async () => {
      sourceFarm = await Farm.init({ adminKeypair: admin });
    });

    beforeEach("whitelist farm", async () => {
      targetFarm = await Farm.init({ adminKeypair: targetAdmin });

      await sourceFarm.WhitelistFarmForCompounding({
        targetFarm: targetFarm.id,
        admin,
      });
    });

    it("fails if admin signer mismatches farm", async () => {
      const fakeAdmin = Keypair.generate();
      await airdrop(fakeAdmin.publicKey);

      const logs = await errLogs(
        sourceFarm.DewhitelistFarmForCompounding({
          targetFarm: targetFarm.id,
          admin: fakeAdmin,
        })
      );

      expect(logs).to.contain("FarmAdminMismatch");

      // Assert that PDA account has not been removed
      const whitelistPda = await sourceFarm.findWhitelistPda(targetFarm.id);
      const whitelistPdaAccount = await provider.connection.getAccountInfo(
        whitelistPda
      );

      expect(whitelistPdaAccount).not.to.eq(null);
    });

    it("fails if admin has not signed transaction", async () => {
      await expect(
        sourceFarm.DewhitelistFarmForCompounding({
          skipAdminSignature: true,
          targetFarm: targetFarm.id,
        })
      ).to.be.rejected;

      // Assert that PDA account has not been created
      const whitelistPda = await sourceFarm.findWhitelistPda(targetFarm.id);
      const whitelistPdaAccount = await provider.connection.getAccountInfo(
        whitelistPda
      );

      expect(whitelistPdaAccount).not.to.eq(null);
    });

    it("works", async () => {
      await sourceFarm.DewhitelistFarmForCompounding({
        targetFarm: targetFarm.id,
      });

      const whitelistPda = await sourceFarm.findWhitelistPda(targetFarm.id);
      const whitelistPdaAccount = await provider.connection.getAccountInfo(
        whitelistPda
      );

      expect(whitelistPdaAccount).to.eq(null);
    });
  });
}
