import chaiAsPromised from "chai-as-promised";
import chai from "chai";

chai.use(chaiAsPromised);

import * as createFarm from "./endpoints/create-farm";
import * as addHarvest from "./endpoints/add-harvest";
import * as removeHarvest from "./endpoints/remove-harvest";
import * as takeSnapshot from "./endpoints/take-snapshot";
import * as setMinSnapshotWindow from "./endpoints/set-min-snapshot-window";
import * as setTokensPerSlot from "./endpoints/set-tokens-per-slot";
import * as setFarmOwner from "./endpoints/set-farm-owner";
import * as createFarmer from "./endpoints/create-farmer";

import { airdrop, provider } from "./helpers";

describe("farming", () => {
  createFarm.test();
  addHarvest.test();
  removeHarvest.test();
  takeSnapshot.test();
  setMinSnapshotWindow.test();
  setFarmOwner.test();
  setTokensPerSlot.test();
  createFarmer.test();

  before("airdrop SOL to provider wallet", async () => {
    await airdrop(provider.wallet.publicKey);
  });
});
