import chaiAsPromised from "chai-as-promised";
import chai from "chai";

chai.use(chaiAsPromised);

import * as createFarm from "./endpoints/create-farm";
import * as addHarvest from "./endpoints/add-harvest";
import { airdrop, provider } from "./helpers";

describe("farming", () => {
  createFarm.test();
  addHarvest.test();

  before("airdrop SOL to provider wallet", async () => {
    await airdrop(provider.wallet.publicKey);
  });
});
