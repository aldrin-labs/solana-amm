import { amm, payer, provider } from "./helpers";
import { Keypair, PublicKey } from "@solana/web3.js";
import {
  createAccount,
  createMint,
  transfer,
  mintTo,
  Account,
  getAccount,
} from "@solana/spl-token";
import { BN } from "@project-serum/anchor";
import { Farmer } from "./farmer";

export interface InitFarmArgs {
  adminKeypair: Keypair;
  keypair: Keypair;
  pda: PublicKey;
  skipAdminSignature: boolean;
  skipCreateFarm: boolean;
  skipKeypairSignature: boolean;
  stakeVault: PublicKey;
  stakeMint: PublicKey;
}

export interface AddHarvestArgs {
  admin: Keypair;
  harvestMint: PublicKey;
  harvestVault: PublicKey;
  pda: PublicKey;
  skipAdminSignature: boolean;
  tokensPerSlot: number;
}

export interface RemoveHarvestArgs {
  admin: Keypair;
  harvestVault: PublicKey;
  pda: PublicKey;
  skipAdminSignature: boolean;
  adminHarvestWallet: PublicKey;
}

export interface TakeSnapshotArgs {
  caller: Keypair;
  farm: PublicKey;
  stakeMint: PublicKey;
  stakeVault: PublicKey;
  clock: PublicKey;
}

export interface SetMinSnapshotWindowArgs {
  admin: Keypair;
  farm: PublicKey;
  skipAdminSignature: boolean;
}

export interface SetFarmOwner {
  admin: Keypair;
  farm: PublicKey;
  newFarmAdmin: Keypair;
  skipAdminSignature: boolean;
  skipNewAdminSignature: boolean;
}

export interface SetTokensPerSlot {
  admin: Keypair;
  farm: PublicKey;
  skipAdminSignature: boolean;
}

export interface FarmWhitelist {
  admin: Keypair;
  sourceFarm: PublicKey;
  targetFarm: PublicKey;
  whitelistCompounding: PublicKey;
  skipAdminSignature: boolean;
}

export interface CompoundSameFarm {
  farm: PublicKey;
  stakeVault: PublicKey;
  harvestVault: PublicKey;
  farmer: PublicKey;
  farmSignerPda: PublicKey;
  whitelistCompounding: PublicKey;
}

export interface CompoundAcrossFarms {
  sourceFarm: PublicKey;
  targetFarm: PublicKey;
  targetStakeVault: PublicKey;
  sourceHarvestVault: PublicKey;
  sourceFarmer: PublicKey;
  targetFarmer: PublicKey;
  sourceFarmSignerPda: PublicKey;
  whitelistCompounding: PublicKey;
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
    const [correctPda, _correctBumpSeed] = await PublicKey.findProgramAddress(
      [Buffer.from("signer"), farmKeypair.publicKey.toBytes()],
      amm.programId
    );
    const farmSignerPda = input.pda ?? correctPda;

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
        const [pda, _bumpSeed] = await PublicKey.findProgramAddress(
          [Buffer.from("stake_vault"), farmKeypair.publicKey.toBytes()],
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
      .createFarm()
      .accounts({
        admin: adminKeypair.publicKey,
        farm: farmKeypair.publicKey,
        farmSignerPda,
        stakeMint,
        stakeVault,
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
    const [pda, _bumpSeed] = await PublicKey.findProgramAddress(
      [Buffer.from("stake_vault"), this.id.toBytes()],
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

  public async findWhitelistPda(targetFarm: PublicKey): Promise<PublicKey> {
    const [pda, _signerBumpSeed] = await PublicKey.findProgramAddress(
      [
        Buffer.from("whitelist_compounding"),
        this.id.toBytes(),
        targetFarm.toBytes(),
      ],
      amm.programId
    );
    return pda;
  }

  public async addHarvest(input: Partial<AddHarvestArgs> = {}): Promise<{
    mint: PublicKey;
    vault: PublicKey;
  }> {
    const tokensPerSlot = { amount: new BN(input.tokensPerSlot ?? 0) };
    const [correctPda, _correctBumpSeed] = await PublicKey.findProgramAddress(
      [Buffer.from("signer"), this.id.toBytes()],
      amm.programId
    );
    const farmSignerPda = input.pda ?? correctPda;
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
        const [pda, _bumpSeed] = await PublicKey.findProgramAddress(
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
      .addHarvest(tokensPerSlot)
      .accounts({
        admin: admin.publicKey,
        farm: this.id,
        farmSignerPda,
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
    const [correctPda, _signerBumpSeed] = await PublicKey.findProgramAddress(
      [Buffer.from("signer"), this.id.toBytes()],
      amm.programId
    );
    const pda = input.pda ?? correctPda;
    const admin = input.admin ?? this.admin;
    const skipAdminSignature = input.skipAdminSignature ?? false;

    const [correctVaultPda, _vaultBumpSeed] =
      await PublicKey.findProgramAddress(
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
      .removeHarvest(mint)
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

  public async takeSnapshot(input: Partial<TakeSnapshotArgs> = {}) {
    const farm = input.farm ?? this.id;

    const stakeVault = input.stakeVault ?? (await this.stakeVault());

    await amm.methods
      .takeSnapshot()
      .accounts({
        farm,
        stakeVault,
      })
      .rpc();
  }

  // To test take_snapshot endpoint to see if the snapshots store the correct amount staked tokens
  // we first need to transfer tokens to the stakeVault
  public async transferToStakeVault(
    depositorWallet: PublicKey,
    amount: number,
    authority: PublicKey = this.admin.publicKey
  ) {
    const stakeVault = await this.stakeVault();

    await transfer(
      provider.connection,
      payer,
      depositorWallet, // source
      stakeVault, // destination
      authority, // owner
      amount // amount
    );
  }

  public async setMinSnapshotWindow(
    setMinSnapshotWindow: number,
    input: Partial<SetMinSnapshotWindowArgs> = {}
  ) {
    const farm = input.farm ?? this.id;
    const admin = input.admin ?? this.admin;
    const skipAdminSignature = input.skipAdminSignature ?? false;

    const signers = [];
    if (!skipAdminSignature) {
      signers.push(admin);
    }

    await amm.methods
      .setMinSnapshotWindow(new BN(setMinSnapshotWindow))
      .accounts({
        admin: admin.publicKey,
        farm,
      })
      .signers(signers)
      .rpc();
  }

  public async createStakeWallet(
    withAmount: number = 0,
    owner: PublicKey = this.admin.publicKey
  ) {
    const stakeWallet = await createAccount(
      provider.connection,
      payer,
      this.stakeMint,
      owner,
      // optional keypair make sure different account is created
      // each time
      Keypair.generate()
    );

    if (withAmount > 0) {
      await this.airdropStakeTokens(stakeWallet, withAmount);
    }

    return stakeWallet;
  }

  public async stakeVaultInfo(): Promise<Account> {
    return getAccount(provider.connection, await this.stakeVault());
  }

  public async airdropStakeTokens(
    wallet: PublicKey,
    amount: number = 1_000_000
  ) {
    return mintTo(
      provider.connection,
      payer,
      this.stakeMint,
      wallet,
      this.admin,
      amount
    );
  }

  public async airdropHarvestTokens(
    harvestMint: PublicKey,
    destination: PublicKey,
    amount: number = 1_000_000
  ) {
    await mintTo(
      provider.connection,
      payer,
      harvestMint,
      destination,
      this.admin,
      amount
    );
  }

  public async setFarmOwner(input: Partial<SetFarmOwner> = {}) {
    const admin = input.admin ?? this.admin;
    const farm = input.farm ?? this.id;
    const newFarmAdmin = input.newFarmAdmin ?? Keypair.generate();
    const skipAdminSignature = input.skipAdminSignature ?? false;
    const skipNewAdminSignature = input.skipNewAdminSignature ?? false;

    const signers = [];
    if (!skipAdminSignature) {
      signers.push(admin);
    }
    if (!skipNewAdminSignature) {
      signers.push(newFarmAdmin);
    }

    await amm.methods
      .setFarmOwner()
      .accounts({
        admin: admin.publicKey,
        farm,
        newFarmAdmin: newFarmAdmin.publicKey,
      })
      .signers(signers)
      .rpc();
  }

  public async setTokensPerSlot(
    harvestMint: PublicKey,
    validFromSlot: number = 0,
    tokensPerSlot: number = 0,
    input: Partial<SetTokensPerSlot> = {}
  ): Promise<void> {
    const admin = input.admin ?? this.admin;
    const farm = input.farm ?? this.id;
    const skipAdminSignature = input.skipAdminSignature ?? false;

    const signers = [];

    if (!skipAdminSignature) {
      signers.push(admin);
    }

    await amm.methods
      .setTokensPerSlot(
        harvestMint,
        { slot: new BN(validFromSlot) },
        { amount: new BN(tokensPerSlot) }
      )
      .accounts({
        admin: admin.publicKey,
        farm,
      })
      .signers(signers)
      .rpc();
  }

  public async WhitelistFarmForCompounding(
    input: Partial<FarmWhitelist> = {}
  ): Promise<void> {
    const admin = input.admin ?? this.admin;
    const sourceFarm = input.sourceFarm ?? this.id;
    const targetFarm = input.targetFarm ?? Keypair.generate().publicKey;
    const skipAdminSignature = input.skipAdminSignature ?? false;

    const [correctPda, _signerBumpSeed] = await PublicKey.findProgramAddress(
      [
        Buffer.from("whitelist_compounding"),
        this.id.toBytes(),
        targetFarm.toBytes(),
      ],
      amm.programId
    );

    const whitelistCompounding = input.whitelistCompounding ?? correctPda;

    const signers = [];

    if (!skipAdminSignature) {
      signers.push(admin);
    }

    await amm.methods
      .whitelistFarmForCompounding()
      .accounts({
        admin: admin.publicKey,
        sourceFarm,
        targetFarm,
        whitelistCompounding,
      })
      .signers(signers)
      .rpc();
  }

  public async DewhitelistFarmForCompounding(
    input: Partial<FarmWhitelist> = {}
  ): Promise<void> {
    const admin = input.admin ?? this.admin;
    const sourceFarm = input.sourceFarm ?? this.id;
    const targetFarm = input.targetFarm ?? Keypair.generate().publicKey;
    const skipAdminSignature = input.skipAdminSignature ?? false;

    const [correctPda, _signerBumpSeed] = await PublicKey.findProgramAddress(
      [
        Buffer.from("whitelist_compounding"),
        this.id.toBytes(),
        targetFarm.toBytes(),
      ],
      amm.programId
    );

    const whitelistCompounding = input.whitelistCompounding ?? correctPda;

    const signers = [];

    if (!skipAdminSignature) {
      signers.push(admin);
    }

    await amm.methods
      .dewhitelistFarmForCompounding()
      .accounts({
        admin: admin.publicKey,
        sourceFarm,
        targetFarm,
        whitelistCompounding,
      })
      .signers(signers)
      .rpc();
  }

  public async compoundSameFarm(
    mint: PublicKey,
    input: Partial<CompoundSameFarm> = {}
  ): Promise<void> {
    const farm = input.farm ?? this.id;
    const stakeVault = input.stakeVault ?? (await this.stakeVault());
    const farmer = input.farmer ?? Keypair.generate().publicKey;

    // Harvest Vault
    const [correctVaultPda, _vaultBumpSeed] =
      await PublicKey.findProgramAddress(
        [Buffer.from("harvest_vault"), this.id.toBytes(), mint.toBytes()],
        amm.programId
      );
    const harvestVault = input.harvestVault ?? correctVaultPda;

    // Whitelist PDA
    const [whitelistCorrectPda, _signerBumpSeed] =
      await PublicKey.findProgramAddress(
        [Buffer.from("whitelist_compounding"), farm.toBytes(), farm.toBytes()],
        amm.programId
      );
    const whitelistCompounding =
      input.whitelistCompounding ?? whitelistCorrectPda;

    // Farm Signer PDA
    const [correctPda, _correctBumpSeed] = await PublicKey.findProgramAddress(
      [Buffer.from("signer"), farm.toBytes()],
      amm.programId
    );
    const farmSignerPda = input.farmSignerPda ?? correctPda;

    await amm.methods
      .compoundSameFarm()
      .accounts({
        farm,
        farmSignerPda,
        whitelistCompounding,
        stakeVault,
        harvestVault,
        farmer,
      })
      .rpc();
  }

  public async compoundAcrossFarms(
    mint: PublicKey,
    input: Partial<CompoundAcrossFarms> = {}
  ): Promise<void> {
    const sourceFarm = input.sourceFarm ?? this.id;

    const possibleTargetFarm = await Farm.init();
    const targetFarm = input.targetFarm ?? possibleTargetFarm.id;

    const [correctTargetVaultPda, _bumpSeed] =
      await PublicKey.findProgramAddress(
        [Buffer.from("stake_vault"), targetFarm.toBytes()],
        amm.programId
      );
    const targetStakeVault = input.targetStakeVault ?? correctTargetVaultPda;

    const sourceFarmer =
      input.sourceFarmer ?? (await (await Farmer.init(this)).id());
    const targetFarmer =
      input.targetFarmer ??
      (await (await Farmer.init(possibleTargetFarm)).id());

    // Harvest Vault
    const [correctVaultPda, _vaultBumpSeed] =
      await PublicKey.findProgramAddress(
        [Buffer.from("harvest_vault"), this.id.toBytes(), mint.toBytes()],
        amm.programId
      );
    const sourceHarvestVault = input.sourceHarvestVault ?? correctVaultPda;

    // Whitelist PDA
    const [whitelistCorrectPda, _signerBumpSeed] =
      await PublicKey.findProgramAddress(
        [
          Buffer.from("whitelist_compounding"),
          this.id.toBytes(),
          targetFarm.toBytes(),
        ],
        amm.programId
      );

    const whitelistCompounding =
      input.whitelistCompounding ?? whitelistCorrectPda;
    // Farm Signer PDA
    const [correctPda, _correctBumpSeed] = await PublicKey.findProgramAddress(
      [Buffer.from("signer"), sourceFarm.toBytes()],
      amm.programId
    );
    const sourceFarmSignerPda = input.sourceFarmSignerPda ?? correctPda;

    await amm.methods
      .compoundAcrossFarms()
      .accounts({
        sourceFarm,
        targetFarm,
        sourceFarmSignerPda,
        whitelistCompounding,
        targetStakeVault,
        sourceHarvestVault,
        sourceFarmer,
        targetFarmer,
      })
      .rpc();
  }
}
