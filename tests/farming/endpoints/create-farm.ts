import { errLogs, provider } from "../../helpers";
import { Keypair, PublicKey } from "@solana/web3.js";
import { getAccount } from "@solana/spl-token";
import { expect } from "chai";
import { Farm } from "../farm";

export function test() {
  describe("create_farm", () => {
    it("fails if farm account isn't initialized with system program", async () => {
      const logs = await errLogs(Farm.init({ skipCreateFarm: true }));
      expect(logs).to.contain("range end index 8");
    });

    it("fails if farm account already exists", async () => {
      const farm = await Farm.init();

      const logs = await errLogs(Farm.init({ keypair: farm.keypair }));
      expect(logs).to.contain("already in use");
    });

    it("fails if provided with incorrect PDA signer address", async () => {
      const logs = await errLogs(
        Farm.init({
          pda: Keypair.generate().publicKey,
        })
      );
      expect(logs).to.contain("seeds constraint was violated");
    });

    it("fails if admin isn't signer", async () => {
      await expect(Farm.init({ skipAdminSignature: true })).to.be.rejected;
    });

    it("fails if stake vault PDA is invalid", async () => {
      const logs = await errLogs(
        Farm.init({ stakeVault: Keypair.generate().publicKey })
      );
      expect(logs).to.contain("unauthorized signer");
    });

    it("works", async () => {
      const farm = await Farm.init();
      const farmInfo = await farm.fetch();

      expect(farmInfo.admin).to.deep.eq(farm.admin.publicKey);
      expect(farmInfo.stakeMint).to.deep.eq(farm.stakeMint);
      expect(farmInfo.stakeVault).to.deep.eq(await farm.stakeVault());

      const stakeVault = await getAccount(
        provider.connection,
        farmInfo.stakeVault
      );
      expect(stakeVault.mint).to.deep.eq(farm.stakeMint);
      expect(stakeVault.owner).to.deep.eq((await farm.signer())[0]);
      expect(stakeVault.closeAuthority).to.eq(null);
      expect(stakeVault.isInitialized).to.eq(true);

      expect(farmInfo.harvests).to.be.lengthOf(10);
      (farmInfo.harvests as any[]).forEach((h) => {
        expect(h.mint).to.deep.eq(PublicKey.default);
        expect(h.vault).to.deep.eq(PublicKey.default);

        expect(h.periods).to.be.lengthOf(10);
        h.periods.forEach(({ tps, startsAt, endsAt }) => {
          expect(tps.amount.toNumber()).to.eq(0);
          expect(startsAt.slot.toNumber()).to.eq(0);
          expect(endsAt.slot.toNumber()).to.eq(0);
        });
      });

      expect(farmInfo.snapshots.ringBufferTip.toNumber()).to.eq(0);
      expect(farmInfo.snapshots.ringBuffer).to.be.lengthOf(1_000);
      (farmInfo.snapshots.ringBuffer as any[]).forEach(
        ({ staked, startedAt }) => {
          expect(staked.amount.toNumber()).to.eq(0);
          expect(startedAt.slot.toNumber()).to.eq(0);
        }
      );

      expect(farmInfo.minSnapshotWindowSlots.toNumber()).to.eq(0);
    });
  });
}
