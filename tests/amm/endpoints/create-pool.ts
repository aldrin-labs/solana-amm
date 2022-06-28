import { expect } from "chai";
import { Pool } from "../pool";

export function test() {
  describe("create_pool", () => {
    it("creates constant product curve", async () => {
      const pool = await Pool.init();

      const info = await pool.fetch();

      expect(info.curve).to.deep.eq({ constProd: {} });
      expect(info.dimension.toNumber()).to.eq(2);
    });

    it("creates stable curve", async () => {
      const pool = await Pool.init(2);

      const info = await pool.fetch();

      expect(info.curve)
        .to.have.property("stable")
        .which.has.property("amplifier");
      expect(info.dimension.toNumber()).to.eq(2);
    });
  });
}
