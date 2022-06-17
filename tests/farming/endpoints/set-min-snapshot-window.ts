import { airdrop, errLogs } from "../helpers";
import { Keypair } from "@solana/web3.js";
import { expect } from "chai";
import { Farm } from "../farm";

export function test() {
  describe("set_min_snapshot_window", () => {
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
        farm.setMinSnapshotWindow(
          2, // minSnapshotWindow
          { admin: fakeAdmin }
        )
      );
      expect(logs).to.contain("FarmAdminMismatch");
    });

    it("fails if admin is not signer", async () => {
      const farmInfoBefore = await farm.fetch();

      await expect(farm.setMinSnapshotWindow(2, { skipAdminSignature: true }))
        .to.be.rejected;

      const farmInfoAfter = await farm.fetch();

      // Assert that nothing else changed on the Farm account
      delete farmInfoBefore.minSnapshotWindowSlots;
      delete farmInfoAfter.minSnapshotWindowSlots;
      expect(farmInfoBefore).to.deep.eq(farmInfoAfter);
    });

    it("works", async () => {
      const farmInfoBefore = await farm.fetch();
      const minSnapshotWindow = 2;

      await farm.setMinSnapshotWindow(minSnapshotWindow);

      const farmInfoAfter = await farm.fetch();

      expect(farmInfoBefore.minSnapshotWindowSlots.toNumber()).to.eq(0);
      expect(farmInfoAfter.minSnapshotWindowSlots.toNumber()).to.eq(2);
    });
  });
}
