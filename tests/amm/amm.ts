import { amm, payer } from "../helpers";
import { PublicKey } from "@solana/web3.js";

/**
 * This is a call to the dev version of this endpoint. Due to the way the anchor
 * loads programs on localnet (so that we can use any pubkey and don't have to
 * sign the program deploy), the programs on localnet don't have the same
 * structure in terms of having a data account as with normal deployment.
 */
export async function createProgramToll(
  programTollAuthority: PublicKey = payer.publicKey
) {
  const programToll = programTollAddress();

  await amm.methods
    .createProgramToll()
    .accounts({
      programTollAuthority,
      programToll,
    })
    .signers([payer])
    .rpc();

  return programToll;
}

export function programTollAddress(): PublicKey {
  const [programToll, _bumpSeed] = PublicKey.findProgramAddressSync(
    [Buffer.from("toll")],
    amm.programId
  );
  return programToll;
}

/**
 * This is a call to the dev version of this endpoint. Due to the way the anchor
 * loads programs on localnet (so that we can use any pubkey and don't have to
 * sign the program deploy), the programs on localnet don't have the same
 * structure in terms of having a data account as with normal deployment.
 */
export async function createDiscountSettings(
  discountSettingsAuthority: PublicKey = payer.publicKey
) {
  const discountSettings = discountSettingsAddress();

  await amm.methods
    .createDiscountSettings()
    .accounts({
      discountSettingsAuthority,
      discountSettings,
    })
    .signers([payer])
    .rpc();

  return discountSettings;
}

export function discountSettingsAddress(): PublicKey {
  const [discountSettings, _bumpSeed] = PublicKey.findProgramAddressSync(
    [Buffer.from("discount_settings")],
    amm.programId
  );
  return discountSettings;
}
