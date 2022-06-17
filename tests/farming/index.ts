import chaiAsPromised from "chai-as-promised";
import chai from "chai";
chai.use(chaiAsPromised);

import * as createFarm from "./endpoints/create-farm";
import * as addHarvest from "./endpoints/add-harvest";
import * as removeHarvest from "./endpoints/remove-harvest";
import * as takeSnapshot from "./endpoints/take-snapshot";
import * as setMinSnapshotWindow from "./endpoints/set-min-snapshot-window";
import * as newHarvestPeriod from "./endpoints/new-harvest-period";
import * as setFarmOwner from "./endpoints/set-farm-owner";
import * as createFarmer from "./endpoints/create-farmer";
import * as closeFarmer from "./endpoints/close-farmer";
import * as startFarming from "./endpoints/start-farming";
import * as whitelistFarmForCompounding from "./endpoints/whitelist-farm-for-compouding";
import * as dewhitelistFarmForCompounding from "./endpoints/dewhitelist-farm-for-compounding";
import * as compoundSameFarm from "./endpoints/compound-same-farm";
import * as compoundAcrossFarms from "./endpoints/compound-across-farms";
import * as stopFarming from "./endpoints/stop-farming";
import * as updateEligibleHarvest from "./endpoints/update-eligible-harvest";
import * as claimEligibleHarvest from "./endpoints/claim-eligible-harvest";

import { airdrop, provider } from "../helpers";

describe("farming", () => {
  createFarm.test();
  addHarvest.test();
  removeHarvest.test();
  takeSnapshot.test();
  setMinSnapshotWindow.test();
  setFarmOwner.test();
  newHarvestPeriod.test();
  createFarmer.test();
  startFarming.test();
  stopFarming.test();
  updateEligibleHarvest.test();
  claimEligibleHarvest.test();
  closeFarmer.test();
  whitelistFarmForCompounding.test();
  dewhitelistFarmForCompounding.test();
  compoundSameFarm.test();
  compoundAcrossFarms.test();

  before("airdrop SOL to provider wallet", async () => {
    await airdrop(provider.wallet.publicKey);
  });
});
