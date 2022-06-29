import { expect } from "chai";
import { amm, payer } from "../../helpers";
import { createDiscountSettings } from "../amm";

export function test() {
  describe("create_discount_settings", () => {
    it("works", async () => {
      const settings = await createDiscountSettings();

      const { authority } = await amm.account.discountSettings.fetch(settings);

      expect(authority).to.deep.equal(payer.publicKey);
    });
  });
}
