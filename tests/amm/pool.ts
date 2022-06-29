import { createAccount, createMint, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { airdrop, amm, payer, provider } from "../helpers";
import { createProgramToll, programTollAddress } from "./amm";
import { BN } from "@project-serum/anchor";

export class Pool {
  private constructor(public id: PublicKey, public admin: Keypair) {
    //
  }

  public static async init(amplifier = 0): Promise<Pool> {
    const id = Keypair.generate();

    const admin = Keypair.generate();
    await airdrop(admin.publicKey);

    const toll = await programTollAddress();
    let tollAuthority = payer.publicKey;
    try {
      const info = await amm.account.programToll.fetch(toll);
      tollAuthority = info.authority;
    } catch {
      await createProgramToll(tollAuthority);
    }

    const poolSigner = Pool.signerFrom(id.publicKey);

    const lpMint = await createMint(
      provider.connection,
      payer,
      poolSigner,
      null,
      9
    );
    const programTollWallet = await createAccount(
      provider.connection,
      payer,
      lpMint,
      tollAuthority
    );

    const vaults = await Promise.all(
      new Array(2).fill(undefined).map(async () => {
        const mint = await createMint(
          provider.connection,
          payer,
          admin.publicKey,
          null,
          9
        );
        const kp = Keypair.generate();
        await createAccount(provider.connection, payer, mint, poolSigner, kp);
        return {
          isSigner: false,
          isWritable: false,
          pubkey: kp.publicKey,
        };
      })
    );

    await amm.methods
      .createPool(new BN(amplifier))
      .accounts({
        admin: admin.publicKey,
        pool: id.publicKey,
        programToll: toll,
        poolSigner,
        programTollWallet,
        lpMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(vaults)
      .signers([admin, id])
      .rpc();

    return new Pool(id.publicKey, admin);
  }

  public async fetch() {
    return amm.account.pool.fetch(this.id);
  }

  public static signerFrom(publicKey: PublicKey): PublicKey {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("signer"), publicKey.toBytes()],
      amm.programId
    )[0];
  }

  public signer(): PublicKey {
    return Pool.signerFrom(this.id);
  }

  public signerPda(): PublicKey {
    return Pool.signerFrom(this.id);
  }

  public async setSwapFee(permillion: number) {
    await amm.methods
      .setPoolSwapFee({
        permillion: new BN(permillion),
      })
      .accounts({ admin: this.admin.publicKey, pool: this.id })
      .signers([this.admin])
      .rpc();
  }
}
