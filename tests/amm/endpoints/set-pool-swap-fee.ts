import { expect } from "chai";
import { errLogs } from "../../helpers";
import { Pool } from "../pool";

export function test() {
  describe("set_pool_swap_fee", () => {
    it("fails if fee more than 1%", async () => {
      const pool = await Pool.init();

      const logs = await errLogs(pool.setSwapFee(15_000));
      expect(logs).to.contain("Maximum fee can be");
    });

    it("works", async () => {
      const pool = await Pool.init();

      const infoBefore = await pool.fetch();
      expect(infoBefore.swapFee.permillion.toNumber()).to.eq(0);

      await pool.setSwapFee(5_000);

      const infoAfter = await pool.fetch();
      expect(infoAfter.swapFee.permillion.toNumber()).to.eq(5_000);
    });
  });
}
