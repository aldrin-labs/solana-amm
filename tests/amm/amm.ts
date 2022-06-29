import { amm, payer } from "../helpers";
import { PublicKey, Keypair } from "@solana/web3.js";
import { BN } from "@project-serum/anchor";

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

/**
 * Creates/updates a discount for the given user.
 *
 * Will create the necessary discount settings if it doesn't exist yet.
 */
export async function putDiscount(
  user: PublicKey,
  discountAmountPermillion: number,
  validUntilSlot: number,
  discountSettingsAuthority: Keypair = payer
): Promise<PublicKey> {
  const settings = await discountSettingsAddress();
  // create settings if it doesn't exist yet
  let discountSettingsInfo;
  try {
    discountSettingsInfo = await amm.account.discountSettings.fetch(settings);
  } catch {
    await createDiscountSettings(discountSettingsAuthority.publicKey);
  }
  if (
    discountSettingsInfo &&
    discountSettingsInfo.authority.toBase58() !==
      discountSettingsAuthority.publicKey.toBase58()
  ) {
    throw new Error("Discount settings authorities don't match");
  }

  const address = discountAddress(user);

  await amm.methods
    .putDiscount(
      user,
      {
        permillion: new BN(discountAmountPermillion),
      },
      { slot: new BN(validUntilSlot) }
    )
    .accounts({
      authority: discountSettingsAuthority.publicKey,
      discount: address,
      discountSettings: settings,
    })
    .signers([discountSettingsAuthority])
    .rpc();

  return address;
}

export function discountAddress(user: PublicKey): PublicKey {
  const [discountSettings, _bumpSeed] = PublicKey.findProgramAddressSync(
    [Buffer.from("discount"), user.toBuffer()],
    amm.programId
  );
  return discountSettings;
}
