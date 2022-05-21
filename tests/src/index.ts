import chaiAsPromised from "chai-as-promised";
import chai from "chai";

chai.use(chaiAsPromised);

import * as createFarm from "./endpoints/create-farm";
import { provider } from "./helpers";

describe("farming", () => {
  createFarm.test();

  before("airdrop SOL", async () => {
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(
        provider.wallet.publicKey,
        100_000_000_000
      ),
      "confirmed"
    );
  });
});
