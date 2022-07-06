import { expect } from "chai";
import { Pool } from "../pool";
import { AccountMeta, Keypair, PublicKey } from "@solana/web3.js";
import { createAccount, getAccount } from "@solana/spl-token";
import { payer, provider, sleep, errLogs } from "../../helpers";
import { BN } from "@project-serum/anchor";

export function test() {
  describe("redeem_liquidity", () => {
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

      Pool.airdropLiquidityTokens(mint1, userTokenWallet1, pool.id, 1_000_000);
      Pool.airdropLiquidityTokens(mint2, userTokenWallet2, pool.id, 1_000_000);

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
          { mint: mint1, tokens: { amount: new BN(100) } },
          { mint: mint2, tokens: { amount: new BN(10) } },
        ],
        vaultsAndWallets,
        lpTokenWallet,
        user,
      });

      sleep(1000);
    });

    it("redeems liquidity on constant product curve", async () => {
      const minAmountTokens = [
        { mint: mint1, tokens: { amount: new BN(0) } },
        { mint: mint2, tokens: { amount: new BN(0) } },
      ];

      let lpTokensToBurn = 1;

      // call to the redeemLiquidity endpoint
      await pool.redeemLiquidity({
        minAmountTokens,
        vaultsAndWallets,
        lpTokenWallet,
        user,
        lpTokensToBurn,
      });

      // Asserting results
      let poolInfo = await pool.fetch();

      let poolTokenVaultInfo1 = await getAccount(
        provider.connection,
        info.reserves[0].vault
      );
      let userTokenWalletInfo1 = await getAccount(
        provider.connection,
        userTokenWallet1
      );

      let poolTokenVaultInfo2 = await getAccount(
        provider.connection,
        info.reserves[1].vault
      );
      let userTokenWalletInfo2 = await getAccount(
        provider.connection,
        userTokenWallet2
      );

      let lpTokenWalletAccountInfo = await getAccount(
        provider.connection,
        lpTokenWallet
      );

      expect(poolTokenVaultInfo1.amount).to.be.eq(BigInt(100 - 10));
      expect(poolTokenVaultInfo2.amount).to.be.eq(BigInt(10 - 1));

      expect(userTokenWalletInfo1.amount).to.be.eq(
        BigInt(1_000_000 - 100 + 10)
      );
      expect(userTokenWalletInfo2.amount).to.be.eq(BigInt(1_000_000 - 10 + 1));

      // we expect that the total amount of lp tokens minted corresponds the
      // lp tokens minted in the first deposit (10 in this case, which
      // corresponds to to the minimum value of deposited tokens),
      // minus the lp tokens requested by the user to be burned upon redemption
      expect(lpTokenWalletAccountInfo.amount).to.be.eq(BigInt(10 - 1));

      // test that nothing else changed in the pool
      expect(
        info.reserves[0].tokens.amount.toNumber() + 100 - 10
      ).to.be.deep.eq(poolInfo.reserves[0].tokens.amount.toNumber());
      expect(info.reserves[1].tokens.amount.toNumber() + 10 - 1).to.be.deep.eq(
        poolInfo.reserves[1].tokens.amount.toNumber()
      );

      // nothing else changes in the pool
      delete info.reserves;
      delete poolInfo.reserves;

      expect(info).to.be.deep.eq(poolInfo);

      // Redeem the remaining liquidity in the pool
      lpTokensToBurn = 9;

      // call to the redeemLiquidity endpoint
      await pool.redeemLiquidity({
        minAmountTokens,
        vaultsAndWallets,
        lpTokenWallet,
        user,
        lpTokensToBurn,
      });

      // Asserting results
      poolInfo = await pool.fetch();

      poolTokenVaultInfo1 = await getAccount(
        provider.connection,
        poolInfo.reserves[0].vault
      );
      userTokenWalletInfo1 = await getAccount(
        provider.connection,
        userTokenWallet1
      );

      poolTokenVaultInfo2 = await getAccount(
        provider.connection,
        poolInfo.reserves[1].vault
      );
      userTokenWalletInfo2 = await getAccount(
        provider.connection,
        userTokenWallet2
      );

      lpTokenWalletAccountInfo = await getAccount(
        provider.connection,
        lpTokenWallet
      );

      expect(poolTokenVaultInfo1.amount).to.be.eq(BigInt(0));
      expect(poolTokenVaultInfo2.amount).to.be.eq(BigInt(0));

      expect(userTokenWalletInfo1.amount).to.be.eq(BigInt(1_000_000));
      expect(userTokenWalletInfo2.amount).to.be.eq(BigInt(1_000_000));

      // we expect that the total amount of lp tokens minted to be zero since
      // all the liquidity is being redeemed form the pool
      expect(lpTokenWalletAccountInfo.amount).to.be.eq(BigInt(0));

      // test that nothing else changed in the pool
      expect(poolInfo.reserves[0].tokens.amount.toNumber()).to.be.deep.eq(0);
      expect(poolInfo.reserves[1].tokens.amount.toNumber()).to.be.deep.eq(0);

      // nothing else changes in the pool
      delete info.reserves;
      delete poolInfo.reserves;

      expect(info).to.be.deep.eq(poolInfo);
    });

    it("fails to redeem liquidity if lp burn amount > lp supply", async () => {
      const minAmountTokens = [
        { mint: mint1, tokens: { amount: new BN(0) } },
        { mint: mint2, tokens: { amount: new BN(0) } },
      ];

      const lpTokensToBurn = 11;

      // call to the redeemLiquidity endpoint
      const logs = await errLogs(
        pool.redeemLiquidity({
          minAmountTokens,
          vaultsAndWallets,
          lpTokenWallet,
          user,
          lpTokensToBurn,
        })
      );

      expect(logs).to.contain(
        "The amount of lp tokens to burn cannot surpass current supply"
      );
    });

    it("fails if user has not enough lp tokens to burn", async () => {
      // We first create a new user and deposit liquidity in order to increase
      // the mint supply, otherwise we would get a different error
      const user2 = Keypair.generate();

      const lpTokenWallet2 = await createAccount(
        provider.connection,
        payer,
        lpMint,
        user2.publicKey
      );

      const user2TokenWallet1 = await createAccount(
        provider.connection,
        payer,
        mint1,
        user2.publicKey
      );

      const user2TokenWallet2 = await createAccount(
        provider.connection,
        payer,
        mint2,
        user2.publicKey
      );

      await Pool.airdropLiquidityTokens(
        mint1,
        user2TokenWallet1,
        pool.id,
        1_000_000
      );
      await Pool.airdropLiquidityTokens(
        mint2,
        user2TokenWallet2,
        pool.id,
        1_000_000
      );

      const vaultsAndWallets2: AccountMeta[] = [
        getAccountMetaFromPublicKey(info.reserves[0].vault),
        getAccountMetaFromPublicKey(user2TokenWallet1),
        getAccountMetaFromPublicKey(info.reserves[1].vault),
        getAccountMetaFromPublicKey(user2TokenWallet2),
      ];

      await pool.depositLiquidity({
        maxAmountTokens: [
          { mint: mint1, tokens: { amount: new BN(100) } },
          { mint: mint2, tokens: { amount: new BN(10) } },
        ],
        vaultsAndWallets: vaultsAndWallets2,
        lpTokenWallet: lpTokenWallet2,
        user: user2,
      });

      const minAmountTokens = [
        { mint: mint1, tokens: { amount: new BN(0) } },
        { mint: mint2, tokens: { amount: new BN(0) } },
      ];

      const lpTokensToBurn = 11;

      // call to the redeemLiquidity endpoint
      const logs = await errLogs(
        pool.redeemLiquidity({
          minAmountTokens,
          vaultsAndWallets,
          lpTokenWallet,
          user,
          lpTokensToBurn,
        })
      );

      expect(logs).to.contain("InvalidLpTokenAmount");
    });

    it("fails to redeem liquidity if invalid mints in min_tokens param", async () => {
      const fakemint = Keypair.generate().publicKey;
      const minAmountTokens = [
        { mint: mint1, tokens: { amount: new BN(0) } },
        { mint: fakemint, tokens: { amount: new BN(0) } },
      ];

      const lpTokensToBurn = 1;

      // call to the redeemLiquidity endpoint
      const logs = await errLogs(
        pool.redeemLiquidity({
          minAmountTokens,
          vaultsAndWallets,
          lpTokenWallet,
          user,
          lpTokensToBurn,
        })
      );

      expect(logs).to.contain("InvalidTokenMints");
    });

    it("fails to redeem liquidity if min_tokens.len != pool dimension", async () => {
      const minAmountTokens = [{ mint: mint1, tokens: { amount: new BN(0) } }];

      const lpTokensToBurn = 1;

      // call to the redeemLiquidity endpoint
      const logs = await errLogs(
        pool.redeemLiquidity({
          minAmountTokens,
          vaultsAndWallets,
          lpTokenWallet,
          user,
          lpTokensToBurn,
        })
      );

      expect(logs).to.contain(
        "Length of min tokens map does not match pool dimension"
      );
    });
  });
}
