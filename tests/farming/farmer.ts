import { airdrop, farming, provider } from "../helpers";
import { Keypair, PublicKey, AccountMeta } from "@solana/web3.js";
import { Farm } from "./farm";
import { Account, getOrCreateAssociatedTokenAccount } from "@solana/spl-token";
import { BN } from "@project-serum/anchor";

export interface InitFarmerArgs {
  payer: Keypair;
  authority: Keypair;
  pda: PublicKey;
}

export interface StartFarmingArgs {
  authority: Keypair;
  farm: PublicKey;
  skipAuthoritySignature: boolean;
  stakeVault: PublicKey;
  stakeWallet: PublicKey;
}

export interface CloseFarmerArgs {
  authority: Keypair;
  skipAuthoritySignature: boolean;
  farmer: PublicKey;
}

export interface StopFarmingArgs {
  authority: Keypair;
  farm: PublicKey;
  skipAuthoritySignature: boolean;
  stakeVault: PublicKey;
  stakeWallet: PublicKey;
  farmSignerPda: PublicKey;
}

export interface UpdateEligibleHarvestArgs {
  farm: PublicKey;
}

export interface ClaimEligibleHarvestArgs {
  authority: Keypair;
  skipAuthoritySignature: boolean;
  farmSignerPda: PublicKey;
}

export class Farmer {
  public async id(): Promise<PublicKey> {
    const [pda, _] = await Farmer.signerFrom(
      this.farm.id,
      this.authority.publicKey
    );

    return pda;
  }

  private constructor(public farm: Farm, public authority: Keypair) {
    //
  }

  public static async init(
    farm: Farm,
    input: Partial<InitFarmerArgs> = {}
  ): Promise<Farmer> {
    const authority = input.authority ?? Keypair.generate();
    const payer = input.payer ?? authority;
    const [correctPda, _bumpSeed] = await Farmer.signerFrom(
      farm.id,
      authority.publicKey
    );
    const pda = input.pda ?? correctPda;

    await airdrop(authority.publicKey);

    await farming.methods
      .createFarmer()
      .accounts({
        payer: payer.publicKey,
        authority: authority.publicKey,
        farm: farm.id,
        farmer: pda,
      })
      .signers([payer])
      .rpc();

    return new Farmer(farm, authority);
  }

  public async fetch() {
    return farming.account.farmer.fetch(await this.id());
  }

  public static async signerFrom(
    farm: PublicKey,
    authority: PublicKey
  ): Promise<[PublicKey, number]> {
    return PublicKey.findProgramAddress(
      [Buffer.from("farmer"), farm.toBytes(), authority.toBytes()],
      farming.programId
    );
  }

  public async stakeWallet(): Promise<Account> {
    return getOrCreateAssociatedTokenAccount(
      provider.connection,
      this.authority,
      this.farm.stakeMint,
      this.authority.publicKey
    );
  }

  public async airdropStakeTokens(amount?: number) {
    const { address } = await this.stakeWallet();
    return this.farm.airdropStakeTokens(address, amount);
  }

  public async harvestWallet(mint: PublicKey): Promise<Account> {
    return getOrCreateAssociatedTokenAccount(
      provider.connection,
      this.authority,
      mint,
      this.authority.publicKey
    );
  }

  public async harvestWalletPubkey(mint: PublicKey): Promise<PublicKey> {
    return (await this.harvestWallet(mint)).address;
  }

  public async startFarming(
    amount: number,
    input: Partial<StartFarmingArgs> = {}
  ) {
    const farm = input.farm ?? this.farm.id;
    const skipAuthoritySignature = input.skipAuthoritySignature ?? false;
    const stakeWallet = input.stakeWallet ?? (await this.stakeWallet()).address;
    const authority = input.authority ?? this.authority;
    const stakeVault = input.stakeVault ?? (await this.farm.stakeVault());

    const signers = [];
    if (!skipAuthoritySignature) {
      signers.push(authority);
    }

    await farming.methods
      .startFarming({ amount: new BN(amount) })
      .accounts({
        farm,
        farmer: await this.id(),
        stakeVault,
        stakeWallet,
        walletAuthority: authority.publicKey,
      })
      .signers(signers)
      .rpc();
  }
  public async close(input: Partial<CloseFarmerArgs> = {}) {
    const farmer = input.farmer ?? (await this.id());
    const authority = input.authority ?? this.authority;
    const skipAuthoritySignature = input.skipAuthoritySignature ?? false;

    const signers = [];
    if (!skipAuthoritySignature) {
      signers.push(authority);
    }

    await farming.methods
      .closeFarmer()
      .accounts({
        authority: authority.publicKey,
        farmer,
      })
      .signers(signers)
      .rpc();
  }

  public async stopFarming(
    amount: number,
    input: Partial<StopFarmingArgs> = {}
  ) {
    const farm = input.farm ?? this.farm.id;
    const skipAuthoritySignature = input.skipAuthoritySignature ?? false;
    const stakeWallet = input.stakeWallet ?? (await this.stakeWallet()).address;
    const authority = input.authority ?? this.authority;
    const stakeVault = input.stakeVault ?? (await this.farm.stakeVault());

    const [correctPda, _correctBumpSeed] = PublicKey.findProgramAddressSync(
      [Buffer.from("signer"), this.farm.id.toBytes()],
      farming.programId
    );
    const farmSignerPda = input.farmSignerPda ?? correctPda;

    const signers = [];
    if (!skipAuthoritySignature) {
      signers.push(authority);
    }

    await farming.methods
      .stopFarming({ amount: new BN(amount) })
      .accounts({
        authority: authority.publicKey,
        farmer: await this.id(),
        stakeWallet,
        farm,
        farmSignerPda,
        stakeVault,
      })
      .signers(signers)
      .rpc();
  }

  public async updateEligibleHarvest(
    input: Partial<UpdateEligibleHarvestArgs> = {}
  ) {
    const farm = input.farm ?? this.farm.id;

    await farming.methods
      .updateEligibleHarvest()
      .accounts({
        farmer: await this.id(),
        farm,
      })
      .rpc();
  }

  public async claimEligibleHarvest(
    vaultWalletPairs: [PublicKey, PublicKey][],
    input: Partial<ClaimEligibleHarvestArgs> = {}
  ) {
    const authority = input.authority ?? this.authority;
    const skipAuthoritySignature = input.skipAuthoritySignature ?? false;

    const [correctPda, _correctBumpSeed] = PublicKey.findProgramAddressSync(
      [Buffer.from("signer"), this.farm.id.toBytes()],
      farming.programId
    );
    const farmSignerPda = input.farmSignerPda ?? correctPda;

    const remainingAccounts: AccountMeta[] = vaultWalletPairs
      .map((tuple) => [
        {
          pubkey: tuple[0],
          isSigner: false,
          isWritable: true,
        },
        {
          pubkey: tuple[1],
          isSigner: false,
          isWritable: true,
        },
      ])
      .flat();

    const signers = [];
    if (!skipAuthoritySignature) {
      signers.push(authority);
    }

    await farming.methods
      .claimEligibleHarvest()
      .accounts({
        authority: authority.publicKey,
        farmer: await this.id(),
        farmSignerPda,
      })
      .remainingAccounts(remainingAccounts)
      .signers(signers)
      .rpc();
  }
}
