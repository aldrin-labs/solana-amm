import { Keypair, PublicKey } from "@solana/web3.js";
import { getAccount, createMint } from "@solana/spl-token";
import { Farm } from "../farm";
import { assert, expect } from "chai";
import { airdrop, errLogs, payer, provider, sleep } from "../helpers";
import { BN } from "bn.js";
import { token } from "@project-serum/anchor/dist/cjs/utils";

export function test() {
  describe("set_tokens_per_slot", () => {
    const admin = Keypair.generate();
    let farm: Farm;

    let harvestMint: PublicKey;
    let harvestVault: PublicKey;

    let validFromSlot: number = 0;
    let tokensPerSlot: number = 100;

    before("airdrop to admin", async () => {
      await airdrop(admin.publicKey);
    });

    beforeEach("create farm", async () => {
      farm = await Farm.init({ adminKeypair: admin });

      let harvestData = await farm.addHarvest({ tokensPerSlot: 10 });

      harvestMint = harvestData.mint;
      harvestVault = harvestData.vault;
    });

    it("fails if admin signer mismatches farm admin", async () => {
      const fakeAdmin = Keypair.generate();
      await airdrop(fakeAdmin.publicKey);

      const logs = await errLogs(
        farm.setTokensPerSlot(harvestMint, validFromSlot, tokensPerSlot, {
          admin: fakeAdmin,
        })
      );

      expect(logs).to.contain("FarmAdminMismatch");
    });

    it("fails if admin is not signer", async () => {
      await expect(
        farm.setTokensPerSlot(harvestMint, validFromSlot, tokensPerSlot, {
          skipAdminSignature: true,
        })
      ).to.be.rejected;
    });

    it("fails if harvest mint is not valid", async () => {
      const fakeHarvestMint = Keypair.generate().publicKey;

      const logs = await errLogs(
        farm.setTokensPerSlot(fakeHarvestMint, validFromSlot, tokensPerSlot, {
          admin,
        })
      );

      expect(logs).to.contain("UnknownHarvestMintPubKey");
    });

    it("fail if valueFromSlot is in the past", async () => {
      let currentSlot = await provider.connection.getSlot();
      let validFromSlot = currentSlot - 1;

      const logs = await errLogs(
        farm.setTokensPerSlot(harvestMint, validFromSlot, tokensPerSlot, {
          admin,
        })
      );
      expect(logs).to.contain("InvalidSlot");
    });

    it("default valueFromSlot = 0 applies changes at current slot", async () => {
      let farmInfoBefore = await farm.fetch();
      await farm.setTokensPerSlot(harvestMint, validFromSlot, tokensPerSlot, {
        admin,
      });

      let farmInfoAfter = await farm.fetch();

      let currentSlot = await provider.connection.getSlot();
      let harvestsBefore = farmInfoBefore.harvests as any[];
      let harvestsAfter = farmInfoAfter.harvests as any[];

      // expect that harvestMint identifies correctly the harvest at index 0,
      // both before and after setTokensPerSlot operation
      expect(harvestsBefore[0].mint).to.deep.eq(harvestMint);
      expect(harvestsAfter[0].mint).to.deep.eq(harvestMint);

      // assert that latest harvest was correctly updated
      expect(
        harvestsAfter[0].tokensPerSlot[0].at.slot.toNumber()
      ).to.be.approximately(currentSlot, 3); // this test ensures that validFromSlot is correctly updated to currentSlot, in the case where validFromSlot = 0

      expect(harvestsAfter[0].tokensPerSlot[0].value.amount.toNumber()).to.eq(
        tokensPerSlot
      );

      // everything other field should not change
      delete farmInfoAfter.harvests;
      delete farmInfoBefore.harvests;
      expect(farmInfoAfter).to.deep.eq(farmInfoBefore);
    });

    it("succeeds with latest snapshot being updated in case of validFromSlot being place in the `future`", async () => {
      let farmInfoBefore = await farm.fetch();
      let latestSlot =
        farmInfoBefore.harvests[0].tokensPerSlot[0].at.slot.toNumber();
      validFromSlot = latestSlot + 1;

      await farm.setTokensPerSlot(harvestMint, validFromSlot, tokensPerSlot, {
        admin,
      });

      let farmInfoAfter = await farm.fetch();
      let harvestsBefore = farmInfoBefore.harvests as any[];
      let harvestsAfter = farmInfoAfter.harvests as any[];

      // expect that harvestMint identifies correctly the harvest at index 0,
      // both before and after setTokensPerSlot operation
      expect(harvestsBefore[0].mint).to.deep.eq(harvestMint);
      expect(harvestsAfter[0].mint).to.deep.eq(harvestMint);

      // assert that latest harvest was correctly updated
      expect(
        harvestsAfter[0].tokensPerSlot[0].at.slot.toNumber()
      ).to.be.approximately(validFromSlot, 3);
      expect(harvestsAfter[0].tokensPerSlot[0].value.amount.toNumber()).to.eq(
        tokensPerSlot
      );

      // everything other field should not change
      delete farmInfoAfter.harvests;
      delete farmInfoBefore.harvests;
      expect(farmInfoAfter).to.deep.eq(farmInfoBefore);
    });

    it("fails if oldest token history slot occurs after oldest snapshot slot", async () => {
      // we default to validFromSlot equal currentSlot
      validFromSlot = 0;
      // not 10 because we already set it once when adding harvest
      const setTokensCallCount = 9;
      for (let i = 0; i < setTokensCallCount; i++) {
        // update valid slots
        await farm.setTokensPerSlot(harvestMint, validFromSlot, tokensPerSlot);
        // we update the current slot
        await sleep(1000);
      }

      validFromSlot = (await provider.connection.getSlot()) + 1;

      const logs = await errLogs(
        farm.setTokensPerSlot(harvestMint, validFromSlot, tokensPerSlot)
      );

      expect(logs).to.contain("ConfigurationUpdateLimitExceeded");
    });

    it("succeeds if oldest token history slot occurs before oldest snapshot slot", async () => {
      // we default to validFromSlot equal currentSlot
      validFromSlot = 0;

      // will contain the updated values
      let tokensPerSlotHistoryArr = [];
      for (let i = 0; i < 5; i++) {
        // set tokens per slot
        await farm.setTokensPerSlot(
          harvestMint,
          validFromSlot,
          tokensPerSlot + 100 * i
        );
        // register values
        tokensPerSlotHistoryArr.push({
          value: { amount: new BN(tokensPerSlot + 100 * i) },
          at: { slot: new BN(await provider.connection.getSlot()) },
        });
        // update the current slot
        await sleep(1000);
      }

      // fetch farm data from blockchain
      let farmInfoAfter = await farm.fetch();
      // check correctness of token per slot history values
      for (let i = 4; i > -1; i--) {
        expect(
          tokensPerSlotHistoryArr[4 - i].value.amount.toNumber()
        ).to.deep.eq(
          farmInfoAfter.harvests[0].tokensPerSlot[i].value.amount.toNumber()
        );
        expect(
          tokensPerSlotHistoryArr[4 - i].at.slot.toNumber(),
          farmInfoAfter.harvests[0].tokensPerSlot[i].at.slot.toNumber()
        );
      }
    });
  });
}
