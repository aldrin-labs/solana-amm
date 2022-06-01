import { airdrop, amm } from "./helpers";
import { Keypair, PublicKey } from "@solana/web3.js";
import { Farm } from "./farm";

export interface InitFarmerArgs {
  authority: Keypair;
  skipAuthoritySignature: boolean;
  pda: PublicKey;
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
}
