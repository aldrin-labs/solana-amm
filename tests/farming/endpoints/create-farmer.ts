import { PublicKey, Keypair } from "@solana/web3.js";
import { expect } from "chai";
import { Farm } from "../farm";
import { Farmer } from "../farmer";
import { errLogs, farming, payer } from "../../helpers";

export function test() {
  describe("create_farmer", () => {
    let farm: Farm;

    beforeEach("create farm", async () => {
      farm = await Farm.init();
    });

    it("has stable size", () => {
      expect(farming.account.farmer.size).to.eq(504);
    });

    it("fails if farmer already exists", async () => {
      const authority = Keypair.generate();

      await Farmer.init(farm, { authority });

      const logs = await errLogs(Farmer.init(farm, { authority }));
      expect(logs).to.contain("already in use");
    });

    it("works", async () => {
      const farmer = await Farmer.init(farm);
      const farmerInfo = await farmer.fetch();

      expect(farmerInfo.authority).to.deep.eq(farmer.authority.publicKey);
      expect(farmerInfo.farm).to.deep.eq(farmer.farm.id);

      expect(farmerInfo.staked.amount.toNumber()).to.deep.eq(0);
      expect(farmerInfo.vested.amount.toNumber()).to.deep.eq(0);
      expect(farmerInfo.vestedAt.slot.toNumber()).to.deep.eq(0);
      expect(farmerInfo.calculateNextHarvestFrom.slot.toNumber()).to.deep.eq(0);

      expect(farmerInfo.harvests).to.be.lengthOf(10);
      (farmerInfo.harvests as any[]).forEach(({ mint, tokens }) => {
        expect(mint).to.deep.eq(PublicKey.default);
        expect(tokens.amount.toNumber()).to.eq(0);
      });
    });

    it("works on behalf of a user", async () => {
      const authority = Keypair.generate();
      const farmer = await Farmer.init(farm, { authority, payer });
      const farmerInfo = await farmer.fetch();

      expect(farmerInfo.authority).to.deep.eq(authority.publicKey);
    });
  });
}
