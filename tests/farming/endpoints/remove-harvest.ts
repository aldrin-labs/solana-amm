import { airdrop, errLogs, payer, provider } from "../../helpers";
import { Keypair, PublicKey } from "@solana/web3.js";
import { createAccount, getAccount, mintTo } from "@solana/spl-token";
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
      expect(logs).to.contain("Invalid Mint");
    });

    it("fails if admin harvest wallet mint mismatches harvest mint", async () => {
      const stakeWallet = await createAccount(
        provider.connection,
        payer,
        farm.stakeMint,
        admin.publicKey
      );

      const logs = await errLogs(
        farm.removeHarvest(harvest.mint, {
          adminHarvestWallet: stakeWallet,
        })
      );
      expect(logs).to.contain("Account not associated with this Mint");
    });

    it("works", async () => {
      const farmInfoBefore = await farm.fetch();

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

      const adminHarvestWallet = await farm.removeHarvest(mint);

      const farmInfo = await farm.fetch();
      expect(farmInfoBefore).to.deep.eq(farmInfo);

      const { amount } = await getAccount(
        provider.connection,
        adminHarvestWallet
      );
      expect(Number(amount)).to.eq(mintToHarvestVaultAmount);

      await expect(getAccount(provider.connection, vault)).to.be.rejected;
    });
  });
}
