import { amm, payer, provider } from "./helpers";
import { Keypair, PublicKey } from "@solana/web3.js";
import { createAccount, createMint } from "@solana/spl-token";
import { BN } from "@project-serum/anchor";
import { TOKEN_PROGRAM_ID } from "@project-serum/anchor/dist/cjs/utils/token";

export interface InitFarmArgs {
  adminKeypair: Keypair;
  bumpSeed: number;
  keypair: Keypair;
  pda: PublicKey;
  skipAdminSignature: boolean;
  skipCreateFarm: boolean;
  skipKeypairSignature: boolean;
  stakeVault: PublicKey;
  vestingVault: PublicKey;
  stakeMint: PublicKey;
}

export interface AddHarvestArgs {
  admin: Keypair;
  bumpSeed: number;
  harvestMint: PublicKey;
  harvestVault: PublicKey;
  pda: PublicKey;
  skipAdminSignature: boolean;
  tokensPerSlot: number;
}

export interface RemoveHarvestArgs {
  admin: Keypair;
  bumpSeed: number;
  harvestVault: PublicKey;
  pda: PublicKey;
  skipAdminSignature: boolean;
  adminHarvestWallet: PublicKey;
}

export class Farm {
  public get id(): PublicKey {
    return this.keypair.publicKey;
  }

  private constructor(
    public keypair: Keypair,
    public admin: Keypair,
    public stakeMint: PublicKey
  ) {
    //
  }

  public static async init(input: Partial<InitFarmArgs> = {}): Promise<Farm> {
    const adminKeypair = input.adminKeypair ?? payer;
    const farmKeypair = input.keypair ?? Keypair.generate();
    const skipAdminSignature = input.skipAdminSignature ?? false;
    const skipCreateFarm = input.skipCreateFarm ?? false;
    const skipKeypairSignature = input.skipAdminSignature ?? skipCreateFarm;
    const [correctPda, correctBumpSeed] = await PublicKey.findProgramAddress(
      [Buffer.from("signer"), farmKeypair.publicKey.toBytes()],
      amm.programId
    );
    const pda = input.pda ?? correctPda;
    const bumpSeed = input.bumpSeed ?? correctBumpSeed;

    const stakeMint =
      input.stakeMint ??
      (await (async () => {
        return createMint(
          provider.connection,
          payer,
          adminKeypair.publicKey,
          null,
          6
        );
      })());

    const stakeVault =
      input.stakeVault ??
      (await (async () => {
        const [pda, _] = await PublicKey.findProgramAddress(
          [Buffer.from("stake_vault"), farmKeypair.publicKey.toBytes()],
          amm.programId
        );
        return pda;
      })());

    const vestingVault =
      input.vestingVault ??
      (await (async () => {
        const [pda, _] = await PublicKey.findProgramAddress(
          [Buffer.from("vesting_vault"), farmKeypair.publicKey.toBytes()],
          amm.programId
        );
        return pda;
      })());

    const signers = [];
    if (!skipAdminSignature) {
      signers.push(adminKeypair);
    }
    if (!skipKeypairSignature) {
      signers.push(farmKeypair);
    }

    const preInstructions = [];
    if (!skipCreateFarm) {
      preInstructions.push(
        await amm.account.farm.createInstruction(farmKeypair)
      );
    }

    await amm.methods
      .createFarm(bumpSeed)
      .accounts({
        admin: adminKeypair.publicKey,
        farm: farmKeypair.publicKey,
        farmSignerPda: pda,
        stakeMint,
        stakeVault,
        vestingVault,
      })
      .signers(signers)
      .preInstructions(preInstructions)
      .rpc();

    return new Farm(farmKeypair, adminKeypair, stakeMint);
  }

  public async fetch() {
    return amm.account.farm.fetch(this.id);
  }

  public async stakeVault(): Promise<PublicKey> {
    const [pda, _] = await PublicKey.findProgramAddress(
      [Buffer.from("stake_vault"), this.id.toBytes()],
      amm.programId
    );
    return pda;
  }

  public async vestingVault(): Promise<PublicKey> {
    const [pda, _] = await PublicKey.findProgramAddress(
      [Buffer.from("vesting_vault"), this.id.toBytes()],
      amm.programId
    );
    return pda;
  }

  public static async signerFrom(
    publicKey: PublicKey
  ): Promise<[PublicKey, number]> {
    return PublicKey.findProgramAddress(
      [Buffer.from("signer"), publicKey.toBytes()],
      amm.programId
    );
  }

  public async signer(): Promise<[PublicKey, number]> {
    return Farm.signerFrom(this.id);
  }

  public async addHarvest(input: Partial<AddHarvestArgs> = {}): Promise<{
    mint: PublicKey;
    vault: PublicKey;
  }> {
    const tokensPerSlot = { amount: new BN(input.tokensPerSlot ?? 0) };
    const [correctPda, correctBumpSeed] = await PublicKey.findProgramAddress(
      [Buffer.from("signer"), this.id.toBytes()],
      amm.programId
    );
    const pda = input.pda ?? correctPda;
    const bumpSeed = input.bumpSeed ?? correctBumpSeed;
    const admin = input.admin ?? this.admin;
    const skipAdminSignature = input.skipAdminSignature ?? false;

    const harvestMint =
      input.harvestMint ??
      (await (async () => {
        return createMint(provider.connection, payer, admin.publicKey, null, 6);
      })());

    const harvestVault =
      input.harvestVault ??
      (await (async () => {
        const [pda, _] = await PublicKey.findProgramAddress(
          [
            Buffer.from("harvest_vault"),
            this.id.toBytes(),
            harvestMint.toBytes(),
          ],
          amm.programId
        );
        return pda;
      })());

    const signers = [];
    if (!skipAdminSignature) {
      signers.push(admin);
    }

    await amm.methods
      .addHarvest(bumpSeed, tokensPerSlot)
      .accounts({
        admin: admin.publicKey,
        farm: this.id,
        farmSignerPda: pda,
        harvestMint,
        harvestVault,
      })
      .signers(signers)
      .rpc();

    return {
      mint: harvestMint,
      vault: harvestVault,
    };
  }

  public async removeHarvest(
    mint: PublicKey,
    input: Partial<RemoveHarvestArgs> = {}
  ): Promise<PublicKey> {
    const [correctPda, correctBumpSeed] = await PublicKey.findProgramAddress(
      [Buffer.from("signer"), this.id.toBytes()],
      amm.programId
    );
    const pda = input.pda ?? correctPda;
    const bumpSeed = input.bumpSeed ?? correctBumpSeed;
    const admin = input.admin ?? this.admin;
    const skipAdminSignature = input.skipAdminSignature ?? false;

    const [correctVaultPda, _] = await PublicKey.findProgramAddress(
      [Buffer.from("harvest_vault"), this.id.toBytes(), mint.toBytes()],
      amm.programId
    );
    const harvestVault = input.harvestVault ?? correctVaultPda;

    const adminHarvestWallet =
      input.adminHarvestWallet ??
      (await (() =>
        createAccount(provider.connection, payer, mint, admin.publicKey))());

    const signers = [];
    if (!skipAdminSignature) {
      signers.push(admin);
    }

    await amm.methods
      .removeHarvest(bumpSeed, mint)
      .accounts({
        admin: admin.publicKey,
        adminHarvestWallet,
        farm: this.id,
        farmSignerPda: pda,
        harvestVault,
      })
      .signers(signers)
      .rpc();

    return adminHarvestWallet;
  }
}
