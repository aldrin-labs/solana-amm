import { farming, payer, provider } from "../helpers";
import { Keypair, PublicKey } from "@solana/web3.js";
import {
  createAccount,
  createMint,
  transfer,
  mintTo,
  Account,
  getAccount,
  getOrCreateAssociatedTokenAccount,
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

export interface SetFarmOwnerArgs {
  admin: Keypair;
  farm: PublicKey;
  skipAdminSignature: boolean;
  skipNewAdminSignature: boolean;
}

export interface NewHarvestPeriodArgs {
  admin: Keypair;
  farm: PublicKey;
  skipAdminSignature: boolean;
  harvestVault: PublicKey;
  harvestWallet: PublicKey;
  signerPda: PublicKey;
  depositTokens: boolean;
}

export interface FarmWhitelistArgs {
  admin: Keypair;
  sourceFarm: PublicKey;
  targetFarm: PublicKey;
  whitelistCompounding: PublicKey;
  skipAdminSignature: boolean;
}

export interface CompoundSameFarmArgs {
  farm: PublicKey;
  stakeVault: PublicKey;
  harvestVault: PublicKey;
  farmer: PublicKey;
  farmSignerPda: PublicKey;
  whitelistCompounding: PublicKey;
}

export interface CompoundAcrossFarmsArgs {
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
    const farmSignerPda =
      input.pda ??
      (await (async () => {
        const [pda, _] = await Farm.signerFrom(farmKeypair.publicKey);
        return pda;
      })());

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
        const [pda, _bumpSeed] = PublicKey.findProgramAddressSync(
          [Buffer.from("stake_vault"), farmKeypair.publicKey.toBytes()],
          farming.programId
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
        await farming.account.farm.createInstruction(farmKeypair)
      );
    }

    await farming.methods
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
    return farming.account.farm.fetch(this.id);
  }

  public async stakeVault(): Promise<PublicKey> {
    const [pda, _bumpSeed] = PublicKey.findProgramAddressSync(
      [Buffer.from("stake_vault"), this.id.toBytes()],
      farming.programId
    );
    return pda;
  }

  public static async signerFrom(
    publicKey: PublicKey
  ): Promise<[PublicKey, number]> {
    return PublicKey.findProgramAddress(
      [Buffer.from("signer"), publicKey.toBytes()],
      farming.programId
    );
  }

  public async signer(): Promise<[PublicKey, number]> {
    return Farm.signerFrom(this.id);
  }

  public async signerPda(): Promise<PublicKey> {
    const [pda, _] = await Farm.signerFrom(this.id);
    return pda;
  }

  public async findWhitelistPda(targetFarm: PublicKey): Promise<PublicKey> {
    const [pda, _signerBumpSeed] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("whitelist_compounding"),
        this.id.toBytes(),
        targetFarm.toBytes(),
      ],
      farming.programId
    );
    return pda;
  }

  public harvestVault(mint: PublicKey): PublicKey {
    const [pda, _bumpSeed] = PublicKey.findProgramAddressSync(
      [Buffer.from("harvest_vault"), this.id.toBytes(), mint.toBytes()],
      farming.programId
    );
    return pda;
  }

  public async harvestVaultAccount(mint: PublicKey): Promise<Account> {
    const pda = this.harvestVault(mint);
    return getAccount(provider.connection, pda);
  }

  public async addHarvest(input: Partial<AddHarvestArgs> = {}): Promise<{
    mint: PublicKey;
    vault: PublicKey;
  }> {
    const farmSignerPda = input.pda ?? (await this.signerPda());
    const admin = input.admin ?? this.admin;
    const skipAdminSignature = input.skipAdminSignature ?? false;

    const harvestMint =
      input.harvestMint ??
      (await (async () => {
        return createMint(provider.connection, payer, admin.publicKey, null, 6);
      })());

    const harvestVault = input.harvestVault ?? this.harvestVault(harvestMint);

    const signers = [];
    if (!skipAdminSignature) {
      signers.push(admin);
    }

    await farming.methods
      .addHarvest()
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
  ): Promise<void> {
    const pda = input.pda ?? (await this.signerPda());
    const admin = input.admin ?? this.admin;
    const skipAdminSignature = input.skipAdminSignature ?? false;

    const harvestVault = input.harvestVault ?? this.harvestVault(mint);

    const signers = [];
    if (!skipAdminSignature) {
      signers.push(admin);
    }

    await farming.methods
      .removeHarvest(mint)
      .accounts({
        admin: admin.publicKey,
        farm: this.id,
        farmSignerPda: pda,
        harvestVault,
      })
      .signers(signers)
      .rpc();
  }

  public async takeSnapshot(input: Partial<TakeSnapshotArgs> = {}) {
    const farm = input.farm ?? this.id;

    const stakeVault = input.stakeVault ?? (await this.stakeVault());

    await farming.methods
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

    await farming.methods
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

  public async setFarmOwner(
    newFarmAdmin: Keypair,
    input: Partial<SetFarmOwnerArgs> = {}
  ) {
    const admin = input.admin ?? this.admin;
    const farm = input.farm ?? this.id;
    const skipAdminSignature = input.skipAdminSignature ?? false;
    const skipNewAdminSignature = input.skipNewAdminSignature ?? false;

    const signers = [];
    if (!skipAdminSignature) {
      signers.push(admin);
    }
    if (!skipNewAdminSignature) {
      signers.push(newFarmAdmin);
    }

    await farming.methods
      .setFarmOwner()
      .accounts({
        admin: admin.publicKey,
        farm,
        newFarmAdmin: newFarmAdmin.publicKey,
      })
      .signers(signers)
      .rpc();
  }

  public async adminHarvestWallet(mint: PublicKey): Promise<PublicKey> {
    const { address } = await this.adminHarvestWalletAccount(mint);
    return address;
  }

  public async adminHarvestWalletAccount(mint: PublicKey): Promise<Account> {
    return getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer,
      mint,
      this.admin.publicKey
    );
  }

  public async newHarvestPeriod(
    harvestMint: PublicKey,
    fromSlot: number,
    periodLength: number,
    tokensPerSlot: number,
    input: Partial<NewHarvestPeriodArgs> = {}
  ): Promise<void> {
    const admin = input.admin ?? this.admin;
    const farm = input.farm ?? this.id;
    const skipAdminSignature = input.skipAdminSignature ?? false;
    const harvestVault = input.harvestVault ?? this.harvestVault(harvestMint);
    const harvestWallet =
      input.harvestWallet ?? (await this.adminHarvestWallet(harvestMint));
    const farmSignerPda = input.signerPda ?? (await this.signerPda());

    if (input.depositTokens ?? true) {
      await this.airdropHarvestTokens(
        harvestMint,
        harvestWallet,
        periodLength * tokensPerSlot
      );
    }

    const signers = [];
    if (!skipAdminSignature) {
      signers.push(admin);
    }

    await farming.methods
      .newHarvestPeriod(
        harvestMint,
        { slot: new BN(fromSlot) },
        new BN(periodLength),
        { amount: new BN(tokensPerSlot) }
      )
      .accounts({
        admin: admin.publicKey,
        farm,
        harvestVault,
        harvestWallet,
        farmSignerPda,
      })
      .signers(signers)
      .rpc();
  }

  public async whitelistFarmForCompounding(
    input: Partial<FarmWhitelistArgs> = {}
  ): Promise<void> {
    const admin = input.admin ?? this.admin;
    const sourceFarm = input.sourceFarm ?? this.id;
    const targetFarm = input.targetFarm ?? Keypair.generate().publicKey;
    const skipAdminSignature = input.skipAdminSignature ?? false;

    const [correctPda, _signerBumpSeed] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("whitelist_compounding"),
        this.id.toBytes(),
        targetFarm.toBytes(),
      ],
      farming.programId
    );

    const whitelistCompounding = input.whitelistCompounding ?? correctPda;

    const signers = [];
    if (!skipAdminSignature) {
      signers.push(admin);
    }

    await farming.methods
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

  public async dewhitelistFarmForCompounding(
    input: Partial<FarmWhitelistArgs> = {}
  ): Promise<void> {
    const admin = input.admin ?? this.admin;
    const sourceFarm = input.sourceFarm ?? this.id;
    const targetFarm = input.targetFarm ?? Keypair.generate().publicKey;
    const skipAdminSignature = input.skipAdminSignature ?? false;

    const [correctPda, _signerBumpSeed] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("whitelist_compounding"),
        this.id.toBytes(),
        targetFarm.toBytes(),
      ],
      farming.programId
    );

    const whitelistCompounding = input.whitelistCompounding ?? correctPda;

    const signers = [];

    if (!skipAdminSignature) {
      signers.push(admin);
    }

    await farming.methods
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
    input: Partial<CompoundSameFarmArgs> = {}
  ): Promise<void> {
    const farm = input.farm ?? this.id;
    const stakeVault = input.stakeVault ?? (await this.stakeVault());
    const farmer = input.farmer ?? Keypair.generate().publicKey;

    const harvestVault = input.harvestVault ?? this.harvestVault(mint);

    // Whitelist PDA
    const [whitelistCorrectPda, _signerBumpSeed] =
      PublicKey.findProgramAddressSync(
        [Buffer.from("whitelist_compounding"), farm.toBytes(), farm.toBytes()],
        farming.programId
      );
    const whitelistCompounding =
      input.whitelistCompounding ?? whitelistCorrectPda;

    const farmSignerPda = input.farmSignerPda ?? (await this.signerPda());

    await farming.methods
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
    input: Partial<CompoundAcrossFarmsArgs> = {}
  ): Promise<void> {
    const sourceFarm = input.sourceFarm ?? this.id;

    const possibleTargetFarm = await Farm.init();
    const targetFarm = input.targetFarm ?? possibleTargetFarm.id;

    const [correctTargetVaultPda, _bumpSeed] = PublicKey.findProgramAddressSync(
      [Buffer.from("stake_vault"), targetFarm.toBytes()],
      farming.programId
    );
    const targetStakeVault = input.targetStakeVault ?? correctTargetVaultPda;

    const sourceFarmer =
      input.sourceFarmer ?? (await (await Farmer.init(this)).id());
    const targetFarmer =
      input.targetFarmer ??
      (await (await Farmer.init(possibleTargetFarm)).id());

    const sourceHarvestVault =
      input.sourceHarvestVault ?? this.harvestVault(mint);

    // Whitelist PDA
    const [whitelistCorrectPda, _signerBumpSeed] =
      PublicKey.findProgramAddressSync(
        [
          Buffer.from("whitelist_compounding"),
          this.id.toBytes(),
          targetFarm.toBytes(),
        ],
        farming.programId
      );

    const whitelistCompounding =
      input.whitelistCompounding ?? whitelistCorrectPda;
    const sourceFarmSignerPda =
      input.sourceFarmSignerPda ?? (await this.signerPda());

    await farming.methods
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
