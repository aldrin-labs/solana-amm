import {
  createAccount,
  createMint,
  getAccount,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import {
  AccountMeta,
  PublicKey,
  Keypair,
  Signer,
  SystemProgram,
} from "@solana/web3.js";
import { airdrop, amm, payer, provider } from "../helpers";
import { createProgramToll, programTollAddress } from "./amm";
import { BN } from "@project-serum/anchor";

export interface DepositLiquidityArgs {
  user: Keypair;
  pool: PublicKey;
  poolSignerPda: PublicKey;
  lpMint: PublicKey;
  lpTokenWallet: PublicKey;
  maxAmountTokens: { mint: PublicKey, tokens: { amount: BN } }[],
  vaultsAndWallets: AccountMeta[];
}

export class Pool {
  private constructor(public id: Keypair, public admin: Keypair) {
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
          id.publicKey,
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

    return new Pool(id, admin);
  }

  public async fetch() {
    return amm.account.pool.fetch(this.id.publicKey);
  }

  public static signerFrom(publicKey: PublicKey): PublicKey {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("signer"), publicKey.toBytes()],
      amm.programId
    )[0];
  }

  public signer(): PublicKey {
    return Pool.signerFrom(this.id.publicKey);
  }

  public signerPda(): PublicKey {
    return Pool.signerFrom(this.id.publicKey);
  }

  public static async airdropLiquidityTokens(
    mint: PublicKey,
    wallet: PublicKey,
    authority: Signer,
    amount: number = 1_000_000
  ) {
    return mintTo(provider.connection, payer, mint, wallet, authority, amount);
  }

  public async depositLiquidity(
    input: Partial<DepositLiquidityArgs>
  ): Promise<void> {
    const user = input.user ?? Keypair.generate();
    const pool = input.pool ?? this.id.publicKey;
    const poolSignerPda = input.poolSignerPda ?? this.signerPda();
    const lpMint = input.lpMint ?? (await this.fetch()).mint;
    const lpTokenWallet =
      input.lpTokenWallet ??
      (await createAccount(provider.connection, payer, lpMint, user.publicKey));

    const defineMaxAmountTokens = async () => {
      const fetchPool = await this.fetch();
      const mint1 = fetchPool.reserves[0].mint;
      const mint2 = fetchPool.reserves[1].mint;

      const amountTokens: { mint: PublicKey, tokens: { amount: BN } }[] = [];
      amountTokens.push({ mint: mint1, tokens: { amount: new BN(100) }});
      amountTokens.push({mint: mint2, tokens: { amount: new BN(10) }});

      return maxAmountTokens;
    };

    const maxAmountTokens =
      input.maxAmountTokens ?? (await defineMaxAmountTokens());

    const getVaultsAndWallets = async () => {
      const fetchPool = await this.fetch();

      const firstVault = fetchPool.reserves[0].vault;
      const secondVault = fetchPool.reserves[1].vault;

      const firstMint = fetchPool.reserves[0].mint;
      const secondMint = fetchPool.reserves[1].mint;

      const firstVaultAccount = await getAccount(
        provider.connection,
        firstVault
      );
      const secondVaultAccount = await getAccount(
        provider.connection,
        secondVault
      );

      const firstWalletAccount = await createAccount(
        provider.connection,
        payer,
        firstMint,
        user.publicKey
      );
      const secondWalletAccount = await createAccount(
        provider.connection,
        payer,
        secondMint,
        user.publicKey
      );

      return [
        {
          isSigner: false,
          isWritable: true,
          pubkey: firstVaultAccount.address,
        },
        {
          isSigner: false,
          isWritable: true,
          pubkey: firstWalletAccount,
        },
        {
          isSigner: false,
          isWritable: true,
          pubkey: secondVaultAccount.address,
        },
        {
          isSigner: false,
          isWritable: true,
          pubkey: secondWalletAccount,
        },
      ];
    };

    const vaultsAndWallets =
      input.vaultsAndWallets ?? (await getVaultsAndWallets());

    await amm.methods
      .depositLiquidity(maxAmountTokens)
      .accounts({
        user: user.publicKey,
        pool,
        poolSignerPda,
        lpMint,
        lpTokenWallet,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts(vaultsAndWallets)
      .signers([user])
      .rpc();
  }

  public async setSwapFee(permillion: number) {
    await amm.methods
      .setPoolSwapFee({
        permillion: new BN(permillion),
      })
      .accounts({ admin: this.admin.publicKey, pool: this.id.publicKey })
      .signers([this.admin])
      .rpc();
  }
}
