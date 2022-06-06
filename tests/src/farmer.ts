import { airdrop, amm, provider } from "./helpers";
import { Keypair, PublicKey } from "@solana/web3.js";
import { Farm } from "./farm";
import { Account, getOrCreateAssociatedTokenAccount } from "@solana/spl-token";
import { BN } from "@project-serum/anchor";

export interface InitFarmerArgs {
  authority: Keypair;
  pda: PublicKey;
  skipAuthoritySignature: boolean;
}

export interface StartFarmingArgs {
  authority: Keypair;
  farm: PublicKey;
  skipAuthoritySignature: boolean;
  stakeVault: PublicKey;
  stakeWallet: PublicKey;
}

export interface StopFarmingArgs {
  authority: Keypair;
  farm: PublicKey;
  skipAuthoritySignature: boolean;
  stakeVault: PublicKey;
  stakeWallet: PublicKey;
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
    const skipAuthoritySignature = input.skipAuthoritySignature ?? false;
    const [correctPda, _bumpSeed] = await Farmer.signerFrom(
      farm.id,
      authority.publicKey
    );
    const pda = input.pda ?? correctPda;

    await airdrop(authority.publicKey);

    const signers = [];
    if (!skipAuthoritySignature) {
      signers.push(authority);
    }

    await amm.methods
      .createFarmer()
      .accounts({
        authority: authority.publicKey,
        farm: farm.id,
        farmer: pda,
      })
      .signers(signers)
      .rpc();

    return new Farmer(farm, authority);
  }

  public async fetch() {
    return amm.account.farmer.fetch(await this.id());
  }

  public static async signerFrom(
    farm: PublicKey,
    authority: PublicKey
  ): Promise<[PublicKey, number]> {
    return PublicKey.findProgramAddress(
      [Buffer.from("farmer"), farm.toBytes(), authority.toBytes()],
      amm.programId
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

    await amm.methods
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

  public async stopFarming(
    amount: number,
    input: Partial<StopFarmingArgs> = {}
  ) {
    const farm = input.farm ?? this.farm.id;
    const skipAuthoritySignature = input.skipAuthoritySignature ?? false;
    const stakeWallet = input.stakeWallet ?? (await this.stakeWallet()).address;
    const authority = input.authority ?? this.authority;
    const stakeVault = input.stakeVault ?? (await this.farm.stakeVault());

    const [correctPda, _correctBumpSeed] = await PublicKey.findProgramAddress(
      [Buffer.from("signer"), this.farm.id.toBytes()],
      amm.programId
    );
    const farmSignerPda = input.farmSignerPda ?? correctPda;

    const signers = [];
    if (!skipAuthoritySignature) {
      signers.push(authority);
    }

    await amm.methods
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
}
