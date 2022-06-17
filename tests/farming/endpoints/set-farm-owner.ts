import { Keypair } from "@solana/web3.js";
import { Farm } from "../farm";
import { expect } from "chai";
import { airdrop, errLogs } from "../../helpers";

export function test() {
  describe("set_farm_owner", () => {
    const admin = Keypair.generate();
    let farm: Farm;

    before("airdrop to admin", async () => {
      await airdrop(admin.publicKey);
    });

    beforeEach("create farm", async () => {
      farm = await Farm.init({ adminKeypair: admin });
    });

    it("fails if admin signer mismatches farm", async () => {
      const fakeAdmin = Keypair.generate();
      await airdrop(fakeAdmin.publicKey);

      const logs = await errLogs(
        farm.setFarmOwner(Keypair.generate(), {
          admin: fakeAdmin,
        })
      );
      expect(logs).to.contain("FarmAdminMismatch");
    });

    it("fails if admin has not signed transaction", async () => {
      await expect(
        farm.setFarmOwner(Keypair.generate(), { skipAdminSignature: true })
      ).to.be.rejected;
    });

    it("fails if new admin has not signed transaction", async () => {
      await expect(
        farm.setFarmOwner(Keypair.generate(), { skipNewAdminSignature: true })
      ).to.be.rejected;
    });

    it("set new farm admin", async () => {
      const farmInfoBefore = await farm.fetch();

      const newFarmAdmin = Keypair.generate();
      await farm.setFarmOwner(newFarmAdmin);
      const farmInfoAfter = await farm.fetch();

      // farm admin should be updated to newAdmin
      expect(farmInfoAfter.admin).to.deep.eq(newFarmAdmin.publicKey);

      // everything else should not change
      delete farmInfoAfter.admin;
      delete farmInfoBefore.admin;
      expect(farmInfoAfter).to.deep.eq(farmInfoBefore);
    });
  });
}
