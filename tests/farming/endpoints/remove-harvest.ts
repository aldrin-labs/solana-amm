import { airdrop, errLogs, payer, provider } from "../../helpers";
import { Keypair, PublicKey } from "@solana/web3.js";
import { getAccount, mintTo } from "@solana/spl-token";
import { expect } from "chai";
import { Farm } from "../farm";

export function test() {
  describe("remove_harvest", () => {
    const admin = Keypair.generate();
    let farm: Farm, harvest: { mint: PublicKey; vault: PublicKey };

    before("airdrop to admin", async () => {
      await airdrop(admin.publicKey);
    });

    beforeEach("create farm", async () => {
      farm = await Farm.init({ adminKeypair: admin });
    });

    beforeEach("create harvest", async () => {
      harvest = await farm.addHarvest();
    });

    it("fails if admin signer mismatches farm", async () => {
      const fakeAdmin = Keypair.generate();
      await airdrop(fakeAdmin.publicKey);

      const logs = await errLogs(
        farm.removeHarvest(harvest.mint, {
          admin: fakeAdmin,
        })
      );
      expect(logs).to.contain("FarmAdminMismatch");
    });

    it("fails if admin is not signer", async () => {
      await expect(
        farm.removeHarvest(harvest.mint, { skipAdminSignature: true })
      ).to.be.rejected;
    });

    it("fails if farm signer pda is wrong", async () => {
      const logs = await errLogs(
        farm.removeHarvest(harvest.mint, {
          pda: Keypair.generate().publicKey,
        })
      );
      expect(logs).to.contain("seeds constraint was violated");
    });

    it("fails if harvest vault is does not have the expected seed", async () => {
      await expect(
        farm.removeHarvest(harvest.mint, {
          harvestVault: Keypair.generate().publicKey,
        })
      ).to.be.rejected;
    });

    it("fails if harvest mint doesn't exist", async () => {
      const logs = await errLogs(
        farm.removeHarvest(Keypair.generate().publicKey)
      );
      expect(logs).to.contain("AccountNotInitialized");
    });

    it("fails if there are tokens in the vault", async () => {
      const { mint, vault } = await farm.addHarvest();

      const mintToHarvestVaultAmount = 1_000;
      await mintTo(
        provider.connection,
        payer,
        mint,
        vault,
        admin,
        mintToHarvestVaultAmount
      );

      const logs = await errLogs(farm.removeHarvest(mint));

      expect(logs).to.contain(
        "Cannot remove harvest which users haven't yet claimed"
      );
    });

    it("works", async () => {
      const farmInfoBefore = await farm.fetch();

      const { mint, vault } = await farm.addHarvest();
      await farm.removeHarvest(mint);

      const farmInfo = await farm.fetch();
      expect(farmInfoBefore).to.deep.eq(farmInfo);

      await expect(getAccount(provider.connection, vault)).to.be.rejected;
    });
  });
}
