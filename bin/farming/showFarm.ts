/**
 * Prints information about a given farm.
 */

require("dotenv").config();

import { Farming } from "../../target/types/farming";
import { AnchorProvider, Program } from "@project-serum/anchor";
import { PublicKey } from "@solana/web3.js";
import * as anchor from "@project-serum/anchor";

const farmPubkey = new PublicKey(process.argv[2]);

if (!farmPubkey) {
  throw new Error("Farm's pubkey must be provided as 1st arg");
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
    admin,
    stakeMint,
    stakeVault,
    harvests,
    snapshots,
    minSnapshotWindowSlots,
  } = await farming.account.farm.fetch(farmPubkey);
  const ringBuffer = snapshots.ringBuffer as any[];
  const latestSnapshot = ringBuffer[Number(snapshots.ringBufferTip)];
  const initialisedHarvests = (harvests as any[]).filter(
    ({ mint }) => !PublicKey.default.equals(mint)
  );

  console.log(
    `
    Farm ${farmPubkey} with admin ${admin} stakes mint ${stakeMint} into vault
    ${stakeVault}.
    Minimum snapshot window is ${minSnapshotWindowSlots} slots and current tip
    is on ${snapshots.ringBufferTip}. The length of the history is
    ${ringBuffer.length} and the latest snapshot started at slot
    ${latestSnapshot.startedAt.slot} with ${latestSnapshot.staked.amount} tokens
    staked.`
  );

  if (initialisedHarvests.length > 0) {
    console.log(`
    Harvest for mint`);

    initialisedHarvests.forEach(({ mint, vault }) => {
      if (PublicKey.default.equals(mint)) {
        return;
      }

      console.log(`
      *  ${mint} is stored in vault ${vault}`);
    });
  } else {
    console.log("There are no harvests setup yet!");
  }
  console.log();
}

main();
