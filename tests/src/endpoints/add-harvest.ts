import { airdrop, errLogs, provider } from "../helpers";
import { Keypair, PublicKey } from "@solana/web3.js";
import { getAccount } from "@solana/spl-token";
import { expect } from "chai";
import { Farm } from "../farm";

export function test() {
  describe("add_harvest", () => {
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
        farm.addHarvest({
          admin: fakeAdmin,
        })
      );
      expect(logs).to.contain("FarmAdminMismatch");
    });

    it("fails if admin is not signer", async () => {
      await expect(farm.addHarvest({ skipAdminSignature: true })).to.be
        .rejected;
    });

    it("fails if farm signer pda is wrong", async () => {
      const logs = await errLogs(
        farm.addHarvest({
          pda: Keypair.generate().publicKey,
        })
      );
      expect(logs).to.contain("seeds constraint was violated");
    });

    it("fails if farm signer bump seed is wrong", async () => {
      const [_, correctBumpSeed] = await farm.signer();

      const logs = await errLogs(
        farm.addHarvest({
          bumpSeed: correctBumpSeed === 0 ? 1 : 0,
        })
      );
      expect(logs).to.contain("seeds constraint was violated");
    });

    it("fails if harvest mint is not token program account", async () => {
      const logs = await errLogs(
        farm.addHarvest({
          harvestMint: farm.id,
        })
      );
      expect(logs).to.contain("owned by a different program");
    });

    it("fails if harvest vault is does not have the expected seed", async () => {
      await expect(
        farm.addHarvest({
          harvestVault: Keypair.generate().publicKey,
        })
      ).to.be.rejected;
    });

    it("fails if harvest mint already exists", async () => {
      const { mint } = await farm.addHarvest();

      const logs = await errLogs(
        farm.addHarvest({
          harvestMint: mint,
        })
      );
      expect(logs).to.contain("already in use");
    });

    it("fails if harvest mints are full", async () => {
      for (let i = 0; i < 10; i++) {
        await farm.addHarvest();
      }

      const logs = await errLogs(farm.addHarvest());
      expect(logs).to.contain("Reached maximum harvest mints");
    });

    it("works", async () => {
      const farmInfoBefore = await farm.fetch();

      // create two harvests, one with default (0) rho and one with 10
      const harvest1 = await farm.addHarvest();
      const harvest2Rho = 10;
      const harvest2 = await farm.addHarvest({
        tokensPerSlot: harvest2Rho,
      });

      const farmInfo = await farm.fetch();

      const currentSlot = await provider.connection.getSlot();

      const harvests = farmInfo.harvests as any[];
      expect(harvests).to.be.lengthOf(10);

      // expect for harvests, which should change, everything else on the
      // farm account should remain the same
      delete farmInfo.harvests;
      delete farmInfoBefore.harvests;
      expect(farmInfo).to.deep.eq(farmInfoBefore);

      // The slot of update should be approximately current slot and the
      // rho should be as provided in the argument. All the other fields in the
      // tokens per slot history array should be default.
      expect(
        harvests[0].tokensPerSlot[0].at.slot.toNumber()
      ).to.be.approximately(currentSlot, 3);
      expect(harvests[0].tokensPerSlot[0].value.amount.toNumber()).to.eq(0);
      harvests[0].tokensPerSlot.slice(1).forEach(({ value, at }) => {
        expect(value.amount.toNumber()).to.eq(0);
        expect(at.slot.toNumber()).to.eq(0);
      });

      expect(
        harvests[1].tokensPerSlot[0].at.slot.toNumber()
      ).to.be.approximately(currentSlot, 3);
      expect(harvests[1].tokensPerSlot[0].value.amount.toNumber()).to.eq(
        harvest2Rho
      );
      harvests[1].tokensPerSlot.slice(1).forEach(({ value, at }) => {
        expect(value.amount.toNumber()).to.eq(0);
        expect(at.slot.toNumber()).to.eq(0);
      });

      delete farmInfo.harvests;
      delete farmInfoBefore.harvests;
      expect(farmInfo).to.deep.eq(farmInfoBefore);

      harvests.slice(2).forEach((h) => {
        expect(h.mint).to.deep.eq(PublicKey.default);
        expect(h.vault).to.deep.eq(PublicKey.default);

        expect(h.tokensPerSlot).to.be.lengthOf(10);
        h.tokensPerSlot.forEach(({ value, at }) => {
          expect(value.amount.toNumber()).to.eq(0);
          expect(at.slot.toNumber()).to.eq(0);
        });
      });

      await Promise.all(
        [harvest1, harvest2].map(async ({ mint, vault }, i) => {
          // harvest vaults should be initialized and owned by the farm signer
          const h = await getAccount(provider.connection, vault);
          expect(h.mint).to.deep.eq(mint);
          expect(h.owner).to.deep.eq((await farm.signer())[0]);
          expect(h.closeAuthority).to.be.null;
          expect(h.isInitialized).to.be.true;

          // and the harvest mint and vault pubkeys should match
          expect(harvests[i].mint).to.deep.eq(mint);
          expect(harvests[i].vault).to.deep.eq(vault);
        })
      );
    });
  });
}
