/**
 * CLI which enables us to set up some AMM accounts. Currently, it supports:
 * - `create_program_toll`
 * - `create_discount_settings`
 *
 * Reads `ANCHOR_WALLET` and `DEPLOYMENT_CLUSTER` from .env file.
 *
 * # Usage
 * ```
 * npm run ts-node bin/amm/create.ts toll-authority \
 *  <toll authority defaults to ANCHOR_WALLET pubkey>
 * ```
 *
 * ```
 * npm run ts-node bin/amm/create.ts discount-settings \
 *  <discounts authority defaults to ANCHOR_WALLET pubkey>
 * ```
 */

require("dotenv").config();

import { Amm } from "../../target/types/amm";
import { AnchorProvider, Program } from "@project-serum/anchor";
import { PublicKey } from "@solana/web3.js";
import { start as startPrompt, get as waitForPrompt } from "prompt";
import * as anchor from "@project-serum/anchor";
import NodeWallet from "@project-serum/anchor/dist/cjs/nodewallet";

const TOLL_AUTHORITY_CMD = "toll-authority";
const DISCOUNTS_SETTINGS_CMD = "discount-settings";
const subcmd = process.argv[2];

// Either program toll authority or discounts settings authority, based on
// whichever are we creating. That's determined by argv[2]
const delegatedAuthorityOptArg = process.argv[3];

// reads wallet path from ANCHOR_WALLET
const deploymentCluster = process.env.DEPLOYMENT_CLUSTER;
const provider = AnchorProvider.local(deploymentCluster);
anchor.setProvider(provider);
const owner = (provider.wallet as NodeWallet).payer;
const amm = anchor.workspace.Amm as Program<Amm>;

async function main() {
  const delegatedAuthority = delegatedAuthorityOptArg
    ? new PublicKey(delegatedAuthorityOptArg)
    : owner.publicKey;

  // https://docs.rs/solana-program/latest/solana_program/bpf_loader_upgradeable/enum.UpgradeableLoaderState.html
  const ammData = await provider.connection.getAccountInfo(amm.programId);
  const ammMetadata = new PublicKey(ammData.data.slice(4));

  const accounts = {
    amm: amm.programId,
    ammMetadata,
    programAuthority: owner.publicKey,
  };

  console.table({
    "Wallet path": process.env.ANCHOR_WALLET,
    "Cluster url": deploymentCluster,
    "AMM program id": amm.programId.toBase58(),
    "AMM program buffer": ammMetadata.toBase58(),
    "Program authority": owner.publicKey.toBase58(),
    "Delegated authority": delegatedAuthority.toBase58(),
    Action: subcmd,
  });

  await promptToContinue();

  switch (subcmd) {
    case TOLL_AUTHORITY_CMD:
      await createProgramToll(accounts, delegatedAuthority);
      break;
    case DISCOUNTS_SETTINGS_CMD:
      await createDiscountSettings(accounts, delegatedAuthority);
      break;
    default:
      console.log(
        `The first argument must be either '${TOLL_AUTHORITY_CMD}' or '${DISCOUNTS_SETTINGS_CMD}'`
      );
      process.exit(1);
  }
}

main();

async function promptToContinue() {
  startPrompt();
  const { shouldContinue } = await waitForPrompt({
    properties: {
      shouldContinue: {
        message: "Continue? (Y/n)",
      },
    },
  });

  if (shouldContinue.toString().toLowerCase() !== "y") {
    console.log("Aborting...");
    process.exit(0);
  }
}

async function createProgramToll(accounts, programTollAuthority: PublicKey) {
  console.log("Creating program toll...");

  const [programToll, _] = PublicKey.findProgramAddressSync(
    [Buffer.from("toll")],
    amm.programId
  );

  const tx = await amm.methods
    .createProgramToll()
    .accounts({
      programTollAuthority,
      programToll,
      ...accounts,
    })
    .signers([owner])
    .rpc();

  console.log(tx);
}

async function createDiscountSettings(
  accounts,
  discountSettingsAuthority: PublicKey
) {
  console.log("Creating discounts settings...");

  const [discountSettings, _] = PublicKey.findProgramAddressSync(
    [Buffer.from("discount_settings")],
    amm.programId
  );

  const tx = await amm.methods
    .createDiscountSettings()
    .accounts({
      discountSettingsAuthority,
      discountSettings,
      ...accounts,
    })
    .signers([owner])
    .rpc();

  console.log(tx);
}
