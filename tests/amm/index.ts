import chaiAsPromised from "chai-as-promised";
import chai from "chai";
chai.use(chaiAsPromised);

import * as createProgramToll from "./endpoints/create-program-toll";
import * as createDiscountSettings from "./endpoints/create-discount-settings";
import * as createPool from "./endpoints/create-pool";

import { airdrop, provider } from "../helpers";

describe("amm", () => {
  createProgramToll.test();
  createPool.test();
  createDiscountSettings.test();

  before("airdrop SOL to provider wallet", async () => {
    await airdrop(provider.wallet.publicKey);
  });
});
