/**
 * Prints information about a given farmer.
 */

require("dotenv").config();

import { Farming } from "../../target/types/farming";
import { AnchorProvider, Program } from "@project-serum/anchor";
import { PublicKey } from "@solana/web3.js";
import * as anchor from "@project-serum/anchor";

const farmerPubkey = new PublicKey(process.argv[2]);

if (!farmerPubkey) {
  throw new Error("Farmer's pubkey must be provided as 1st arg");
}
if (!process.env.ANCHOR_WALLET) {
  throw new Error("Please set up ANCHOR_WALLET env");
}

// reads wallet path from ANCHOR_WALLET
const deploymentCluster = process.env.DEPLOYMENT_CLUSTER;
const provider = AnchorProvider.local(deploymentCluster);
anchor.setProvider(provider);
const farming = anchor.workspace.Farming as Program<Farming>;

async function main() {
  console.log("Farming program:", farming.programId.toBase58());

  const {
    authority,
    farm,
    staked,
    vested,
    vestedAt,
    calculateNextHarvestFrom,
    harvests,
  } = await farming.account.farmer.fetch(farmerPubkey);

  console.log(
    `
    Farmer ${farmerPubkey} with authority ${authority}
    belongs to farm ${farm}. It stakes ${staked.amount} tokens and
    vests ${vested.amount} tokens (vested at slot ${vestedAt.slot}.)
    Next harvest should be calculated from slot ${calculateNextHarvestFrom.slot}.`
  );

  const initialisedHarvests = (harvests as any[]).filter(
    ({ mint }) => !PublicKey.default.equals(mint)
  );

  if (initialisedHarvests.length > 0) {
    console.log(`
    Harvest for mint`);

    initialisedHarvests.forEach(({ mint, tokens }) => {
      if (PublicKey.default.equals(mint)) {
        return;
      }

      console.log(`
      *  ${mint} is ${tokens.amount} tokens`);
    });
  } else {
    console.log("There are no harvests setup yet!");
  }
  console.log();
}

main();
