import { getAccount } from "@solana/spl-token";
import { Keypair, PublicKey } from "@solana/web3.js";
import { expect } from "chai";
import { Farm } from "../farm";
import { Farmer } from "../farmer";
import { errLogs, provider } from "../../helpers";

export function test() {
  describe("airdrop", () => {
    let farm: Farm,
      farmer: Farmer,
      harvest1: { mint: PublicKey; vault: PublicKey },
      harvest2: { mint: PublicKey; vault: PublicKey };

    beforeEach("create farm", async () => {
      farm = await Farm.init();
    });

    beforeEach("create harvests", async () => {
      harvest1 = await farm.addHarvest();
      await farm.newHarvestPeriod(harvest1.mint, 0, 100, 10);

      harvest2 = await farm.addHarvest();
      await farm.newHarvestPeriod(harvest2.mint, 0, 100, 10);
    });

    beforeEach("create farmer", async () => {
      farmer = await Farmer.init(farm);
    });

    it("fails if harvest vault seed is wrong", async () => {
      const logs = await errLogs(
        farmer.airdrop(10, harvest1.mint, {
          harvestVault: Keypair.generate().publicKey,
        })
      );

      expect(logs).to.contain("seeds constraint was violated");
    });

    it("works", async () => {
      const airdropAmount = 123;

      const farmerBefore = await farmer.fetch();
      const { amount: vaultAmountBefore } = await getAccount(
        provider.connection,
        harvest1.vault
      );

      await farmer.airdrop(airdropAmount, harvest1.mint);

      const farmerAfter = await farmer.fetch();

      const harvests = farmerAfter.harvests as any[];

      // nothing else changes but harvests
      delete farmerAfter.harvests;
      delete farmerBefore.harvests;
      expect(farmerBefore).to.deep.eq(farmerAfter);

      const h1 = harvests.find(
        (h) => h.mint.toBase58() === harvest1.mint.toBase58()
      );
      expect(h1.tokens.amount.toNumber()).to.eq(airdropAmount);

      // doesn't change
      const h2 = harvests.find(
        (h) => h.mint.toBase58() === harvest2.mint.toBase58()
      );
      expect(h2.tokens.amount.toNumber()).to.eq(0);

      // deposited to vault
      const { amount: vaultAmountAfter } = await getAccount(
        provider.connection,
        harvest1.vault
      );
      expect(Number(vaultAmountAfter)).to.eq(
        Number(vaultAmountBefore) + airdropAmount
      );
    });
  });
}
