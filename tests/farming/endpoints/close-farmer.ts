import { Keypair } from "@solana/web3.js";
import { expect } from "chai";
import { Farm } from "../farm";
import { Farmer } from "../farmer";
import { errLogs, sleep } from "../../helpers";

export function test() {
  describe("close_farmer", () => {
    let farm: Farm, farmer: Farmer;

    beforeEach("create farm", async () => {
      farm = await Farm.init();
    });

    beforeEach("create farmer", async () => {
      farmer = await Farmer.init(farm);
    });

    it("fails if farmer account does not exist", async () => {
      const otherAuthority = Keypair.generate();

      const [otherPda, _bumpSeed] = await Farmer.signerFrom(
        farm.id,
        otherAuthority.publicKey
      );

      const logs = await errLogs(
        farmer.close({
          authority: otherAuthority,
          farmer: otherPda,
        })
      );
      expect(logs).to.contain("AccountNotInitialized");
    });

    it("fails on wrong authority signer", async () => {
      const otherAuthority = Keypair.generate();

      const logs = await errLogs(
        farmer.close({
          authority: otherAuthority,
        })
      );
      expect(logs).to.contain("Authority does not own this farmer");
    });

    it("fails if authority isn't signer", async () => {
      await expect(
        farmer.close({
          skipAuthoritySignature: true,
        })
      ).to.be.rejected;
    });

    it("fails if there's some unclaimed harvest", async () => {
      const tokensPerSlot = 10;
      await farm.setMinSnapshotWindow(1);
      const harvest = await farm.addHarvest();
      await farm.newHarvestPeriod(harvest.mint, 0, 1000, tokensPerSlot);
      await farm.takeSnapshot();

      await farmer.airdropStakeTokens(10);
      await farmer.startFarming(10);
      await sleep(1000);
      await farm.takeSnapshot();
      await farmer.stopFarming(10);

      const logs = await errLogs(farmer.close());
      expect(logs).to.contain("Claim all farmer's harvest");
    });

    it("fails if there're staked tokens", async () => {
      await farmer.airdropStakeTokens(100);
      await farmer.startFarming(100);

      const logs = await errLogs(farmer.close());
      expect(logs).to.contain("Unstake all farmer's tokens");
    });

    it("works", async () => {
      await farmer.close();

      await expect(farmer.fetch()).to.be.rejected;
    });
  });
}
