import { expect } from "chai";
import { amm, payer } from "../../helpers";
import { createProgramToll } from "../amm";

export function test() {
  describe("create_program_toll", () => {
    it("works", async () => {
      const toll = await createProgramToll();

      const { authority } = await amm.account.programToll.fetch(toll);

      expect(authority).to.deep.equal(payer.publicKey);
    });
  });
}
