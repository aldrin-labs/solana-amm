import { expect } from "chai";
import { amm, getCurrentSlot } from "../../helpers";
import { Keypair } from "@solana/web3.js";
import { putDiscount } from "../amm";

export function test() {
  describe("put_discount", () => {
    it("creates one if it doesn't exist", async () => {
      const user = Keypair.generate();
      const permillion = 100_000;
      const validUntilSlot = (await getCurrentSlot()) + 100;

      const discount = await putDiscount(
        user.publicKey,
        permillion,
        validUntilSlot
      );

      const info = await amm.account.discount.fetch(discount);
      expect(info.amount.permillion.toNumber()).to.eq(permillion);
      expect(info.validUntil.slot.toNumber()).to.eq(validUntilSlot);
    });

    it("updates an existing one", async () => {
      const user = Keypair.generate();
      const permillion1 = 100_000;
      const validUntilSlot1 = (await getCurrentSlot()) + 100;
      const permillion2 = 120_000;
      const validUntilSlot2 = (await getCurrentSlot()) + 120;

      const discount = await putDiscount(
        user.publicKey,
        permillion1,
        validUntilSlot1
      );
      await putDiscount(user.publicKey, permillion2, validUntilSlot2);

      const info = await amm.account.discount.fetch(discount);
      expect(info.amount.permillion.toNumber()).to.eq(permillion2);
      expect(info.validUntil.slot.toNumber()).to.eq(validUntilSlot2);
    });
  });
}
