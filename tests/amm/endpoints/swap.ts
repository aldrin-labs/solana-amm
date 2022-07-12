import { expect } from "chai";
import { Pool } from "../pool";
import { AccountMeta, Keypair, PublicKey } from "@solana/web3.js";
import { createAccount, getAccount } from "@solana/spl-token";
import { payer, provider, sleep } from "../../helpers";
import { BN } from "@project-serum/anchor";

export function test() {
  describe("swap", () => {
    const user = Keypair.generate();
    let pool: Pool;
    let info;
    let mint1;
    let mint2;
    let vaultsAndWallets: AccountMeta[];
    let lpTokenWallet: PublicKey;
    let lpMint;

    let userTokenWallet1: PublicKey;
    let userTokenWallet2: PublicKey;

    const getAccountMetaFromPublicKey = (pk) => {
      return { isSigner: false, isWritable: true, pubkey: pk };
    };

    beforeEach("init pool", async () => {
      pool = await Pool.init();
      info = await pool.fetch();
      await pool.setSwapFee(5_000); // 0.5%
    });

    beforeEach("set up accounts", async () => {
      mint1 = info.reserves[0].mint;
      mint2 = info.reserves[1].mint;

      userTokenWallet1 = await createAccount(
        provider.connection,
        payer,
        mint1,
        user.publicKey
      );

      userTokenWallet2 = await createAccount(
        provider.connection,
        payer,
        mint2,
        user.publicKey
      );

      Pool.airdropLiquidityTokens(
        mint1,
        userTokenWallet1,
        pool.id,
        2_000_000_000
      );
      Pool.airdropLiquidityTokens(mint2, userTokenWallet2, pool.id, 20_000_000);

      await sleep(1000);

      vaultsAndWallets = [
        getAccountMetaFromPublicKey(info.reserves[0].vault),
        getAccountMetaFromPublicKey(userTokenWallet1),
        getAccountMetaFromPublicKey(info.reserves[1].vault),
        getAccountMetaFromPublicKey(userTokenWallet2),
      ];
    });

    beforeEach("deposit liquidity", async () => {
      // get mint public key
      lpMint = info.mint;

      // create user lpTokenWallet
      lpTokenWallet = await createAccount(
        provider.connection,
        payer,
        lpMint,
        user.publicKey
      );

      // call to the depositLiquidity endpoint
      await pool.depositLiquidity({
        maxAmountTokens: [
          { mint: mint1, tokens: { amount: new BN(1_000_000_000) } },
          { mint: mint2, tokens: { amount: new BN(10_000_000) } },
        ],
        vaultsAndWallets,
        lpTokenWallet,
        user,
      });

      sleep(1000);
    });

    it("works", async () => {
      await pool.swap(
        user,
        userTokenWallet1,
        userTokenWallet2,
        1_000_000,
        9_900
      );

      // Asserting results
      const poolInfo = await pool.fetch();

      const poolTokenVaultInfo1 = await getAccount(
        provider.connection,
        info.reserves[0].vault
      );
      const userTokenWalletInfo1 = await getAccount(
        provider.connection,
        userTokenWallet1
      );
      expect(Number(poolTokenVaultInfo1.amount)).to.eq(1_001_000_000);
      expect(Number(userTokenWalletInfo1.amount)).to.eq(999_000_000);

      const poolTokenVaultInfo2 = await getAccount(
        provider.connection,
        info.reserves[1].vault
      );
      const userTokenWalletInfo2 = await getAccount(
        provider.connection,
        userTokenWallet2
      );
      // 9_990_060 =  K / x1, where x1 is the reserve being sold by the trader
      // K = 10_000_000_000_000_000 and x1 = 1_000_995_000, because
      // x1 = x0 + Δx, where Δx = 1_000_000 * (1 - 0.005)
      expect(Number(poolTokenVaultInfo2.amount)).to.eq(9_990_060);
      expect(Number(userTokenWalletInfo2.amount)).to.eq(10_009_940);

      const tollWallet = await getAccount(
        provider.connection,
        poolInfo.programTollWallet
      );
      expect(Number(tollWallet.amount)).to.eq(8);
    });
  });
}
