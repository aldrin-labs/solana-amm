/**
 * @flow
 */

import assert from 'assert';
import BN from 'bn.js';
import {Buffer} from 'buffer';
import * as BufferLayout from 'buffer-layout';
import type {Connection, TransactionSignature} from '@solana/web3.js';
import {
  Account,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from '@solana/web3.js';

import * as Layout from './layout';
import {sendAndConfirmTransaction} from './util/send-and-confirm-transaction';
import {loadAccount} from './util/account';
import {Numberu64, TOKEN_SWAP_PROGRAM_ID} from './token-swap'


/**
 * @private
 */
export const TokenFarmingLayout: typeof BufferLayout.Structure = BufferLayout.struct(
  [
    BufferLayout.uint64('discriminator'),
    BufferLayout.u8('isInitialized'),
    BufferLayout.uint64('tokensUnlocked'),
    Layout.uint64('tokensTotal'),
    Layout.uint64('tokensPerPeriod'),
    Layout.uint64('periodLength'),
    Layout.uint64('startTime'),
    Layout.uint64('currentTime'),
    Layout.publicKey('attachedSwapAccount'),
    Layout.publicKey('farmingTokenAccount'),
    BufferLayout.blob(3758, 'farmingSnapshots'),
  ],
);

export const FarmingTicketLayout: typeof BufferLayout.Structure = BufferLayout.struct(
  [
    BufferLayout.uint64('discriminator'),
    BufferLayout.u8('isInitialized'),
    BufferLayout.uint64('tokensFrozen'),
    Layout.uint64('startTime'),
    Layout.uint64('endTime'),
    Layout.publicKey('tokenAuthority'),
    Layout.publicKey('farmingState'),
  ],
);

/**
 * A program to exchange tokens against a pool of liquidity
 */
export class TokenFarming {
  /**
   * @private
   */
  connection: Connection;

  tokenSwapAccount: PublicKey;

  farmingStateAccount: PublicKey;    

  tokenFreezeAccount: PublicKey;

  farmingTokenAccount: PublicKey;

  feeAccountPubkey: PublicKey;

  swapAuthority: PublicKey;
  
  tokenProgramId: PublicKey;

  swapProgramId: PublicKey;

  tokenAmount: number;

  tokensPerPeriod: number;  

  periodLength: number;

  /**
   * Fee payer
   */
  payer: Account;

  /**
   * Create a Token object attached to the specific token
   *
   * @param connection The connection to use
   * @param tokenSwap The token swap account
   * @param swapProgramId The program ID of the token-swap program
   * @param tokenProgramId The program ID of the token program
   * @param poolToken The pool token
   * @param authority The authority over the swap and accounts
   * @param tokenAccountA: The token swap's Token A account
   * @param tokenAccountB: The token swap's Token B account
   * @param payer Pays for the transaction
   */
  constructor(
    connection: Connection,
    tokenSwapAccount: PublicKey,
    farmingStateAccount: PublicKey,
    tokenFreezeAccount: PublicKey,
    farmingTokenAccount: PublicKey,
    feeAccountPubkey: PublicKey,
    swapAuthority: PublicKey,
    tokenProgramId: PublicKey,
    swapProgramId: PublicKey,
    tokenAmount: Numberu64,
    tokensPerPeriod: Numberu64,
    periodLength: Numberu64,
    payer: Account,
  ) {
    Object.assign(this, {
      connection,
      tokenSwapAccount,
      farmingStateAccount,
      tokenFreezeAccount,
      farmingTokenAccount,
      feeAccountPubkey,
      swapAuthority,
      tokenProgramId, 
      swapProgramId, 
      tokenAmount,
      tokensPerPeriod,
      periodLength, 
      payer,
    });
  }

  /**
   * Get the minimum balance for the token swap account to be rent exempt
   *
   * @return Number of lamports required
   */
  static async getMinBalanceRentForExemptTokenSwap(
    connection: Connection,
  ): Promise<number> {
    return await connection.getMinimumBalanceForRentExemption(
      TokenFarmingLayout.span,
    );
  }

  static createInitFarmingInstruction(
    tokenSwapAccount: Account,
    farmingStateAccount: PublicKey,    
    farmingTokenAccount: PublicKey,
    userFarmingTokenAccount: PublicKey,    
    userTransferAuthority: PublicKey,
    feeAccount: PublicKey,
    swapAuthority: PublicKey,
    clock: PublicKey,
    tokenProgramId: PublicKey,

    swapProgramId: PublicKey,
    tokenAmount: number,    
    tokensPerPeriod: number,  
    periodLength: number,  
  ): TransactionInstruction {
    const keys = [
      {pubkey: tokenSwapAccount.publicKey, isSigner: false, isWritable: true},
      {pubkey: farmingStateAccount, isSigner: false, isWritable: true},
      {pubkey: farmingTokenAccount, isSigner: false, isWritable: true},
      {pubkey: userFarmingTokenAccount, isSigner: false, isWritable: true},
      {pubkey: userTransferAuthority, isSigner: true, isWritable: false},
      {pubkey: feeAccount, isSigner: true, isWritable: false},
      {pubkey: swapAuthority, isSigner: false, isWritable: false},
      {pubkey: clock, isSigner: false, isWritable: false},
      {pubkey: tokenProgramId, isSigner: false, isWritable: false},
    ];
    const commandDataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),
      BufferLayout.uint64('tokenAmount'),
      BufferLayout.uint64('tokensPerPeriod'),
      BufferLayout.uint64('periodLength'),      
    ]);
    let data = Buffer.alloc(25);
    {
      const encodeLength = commandDataLayout.encode(
        {
          instruction: 9, // InitializeSwap instruction
          tokenAmount,
          tokensPerPeriod,
          periodLength
        },
        data,
      );
      data = data.slice(0, encodeLength);
    }
    return new TransactionInstruction({
      keys,
      programId: swapProgramId,
      data,
    });
  }

  static async loadTokenFarmingState(
    connection: Connection,
    address: PublicKey,
    programId: PublicKey,
    tokenProgramId: PublicKey,
    payer: Account,
  ): Promise<TokenFarming> {
    const data = await loadAccount(connection, address, programId);
    const tokenFarmingData = TokenFarmingLayout.decode(data);
    if (!tokenFarmingData.isInitialized) {
      throw new Error(`Invalid token farming state`);
    }

    const [authority] = await PublicKey.findProgramAddress(
      [address.toBuffer()],
      programId,
    );
    
    const swapAccount = new PublicKey(tokenFarmingData.attachedSwapAccount);
    const tokenFreezeAccount = new PublicKey(tokenFarmingData.feeAccount);
    const farmingTokenAccount = new PublicKey(tokenFarmingData.tokenAccountA);
    const feeAccountPubkey = new PublicKey(tokenFarmingData.tokenAccountB);
    const swapAuthority = new PublicKey(tokenFarmingData.mintA);
   

    const tokenAmount = Numberu64.fromBuffer(
      tokenFarmingData.tokensTotal,
    );
    const tokensPerPeriod = Numberu64.fromBuffer(
      tokenFarmingData.tokensPerPeriod,
    );
    const periodLength = Numberu64.fromBuffer(
      tokenFarmingData.periodLength,
    );
  
    return new TokenFarming(      
      connection,
      swapAccount,
      address,
      tokenFreezeAccount,
      farmingTokenAccount,
      feeAccountPubkey,
      swapAuthority,
      tokenProgramId,
      programId,
      tokenAmount,
      tokensPerPeriod,
      periodLength,
      payer,
    );
  }


  /**
   * Create a new Token Swap
   *
   * @param connection The connection to use
 
   * @return Token object for the newly minted token, Public key of the account holding the total supply of new tokens
   */
  static async initializeTokenFarming(   
    connection: Connection,
    tokenSwapAccount: PublicKey,
    farmingStateAccount: PublicKey,
    tokenFreezeAccount: PublicKey,
    farmingTokenAccount: PublicKey,
    feeAccountPubkey: PublicKey,
    swapAuthority: PublicKey,
    tokenProgramId: PublicKey,
    swapProgramId: PublicKey,
    tokenAmount: number,
    tokensPerPeriod: number,
    periodLength: number,
    payer: Account,
  ): Promise<TokenFarming> {
    let transaction;
    const tokenFarming = new TokenFarming(
      connection,
      tokenSwapAccount,
      farmingStateAccount,
      tokenFreezeAccount,
      farmingTokenAccount,
      feeAccountPubkey,
      swapAuthority,
      tokenProgramId,
      swapProgramId,
      new Numberu64(tokenAmount),
      new Numberu64(tokensPerPeriod),
      new Numberu64(periodLength),
      payer,
    );
    
    transaction = new Transaction();
    
    const instruction = TokenFarming.createInitFarmingInstruction(
      tokenSwapAccount,
      
    );

    transaction.add(instruction);
    await sendAndConfirmTransaction(
      'InitializeTokenFarming',
      connection,
      transaction,
      payer,
      tokenSwapAccount,
    );

    return tokenSwap;
  }

  /**
   * 
   *
   * @param userSource User's source token account
   * @param poolSource Pool's source token account
   * @param poolDestination Pool's destination token account
   * @param userDestination User's destination token account
   * @param hostFeeAccount Host account to gather fees
   * @param userTransferAuthority Account delegated to transfer user's tokens
   * @param amountIn Amount to transfer from source account
   * @param minimumAmountOut Minimum amount of tokens the user will receive
   */
  async takeFarmingSnapshot(   
    clock: PublicKey,
  ): Promise<TransactionSignature> {
    return await sendAndConfirmTransaction(
      'takeFarmingSnapshot',
      this.connection,
      new Transaction().add(
        TokenFarming.takeFarmingSnapshotInstruction(
          this.tokenSwap,
          this.farmingState,
          this.tokenFreezee,
          this.feeAccount,
          clock,
        ),
      ),
      this.payer,
      userTransferAuthority,
    );
  }

  static takeFarmingSnapshotInstruction(
    tokenSwap: PublicKey,
    farmingState: PublicKey,
    tokenFreezee: PublicKey,
    feeAccount: PublicKey,
    clock: PublicKey,
  ): TransactionInstruction {

    const keys = [
      {pubkey: tokenSwap, isSigner: false, isWritable: false},
      {pubkey: farmingState, isSigner: false, isWritable: false},
      {pubkey: tokenFreezee, isSigner: true, isWritable: false},
      {pubkey: feeAccount, isSigner: false, isWritable: true},
      {pubkey: clock, isSigner: false, isWritable: true},    
    ];
    if (hostFeeAccount != null) {
      keys.push({pubkey: hostFeeAccount, isSigner: false, isWritable: true});
    }
    return new TransactionInstruction({
      keys,
      programId: swapProgramId,
      null,
    });
  }

  /**
   * Deposit tokens into the pool
   * @param userAccountA User account for token A
   * @param userAccountB User account for token B
   * @param poolAccount User account for pool token
   * @param userTransferAuthority Account delegated to transfer user's tokens
   * @param poolTokenAmount Amount of pool tokens to mint
   * @param maximumTokenA The maximum amount of token A to deposit
   * @param maximumTokenB The maximum amount of token B to deposit
   */
  async startFarming(
    poolAccount: PublicKey,
    userTransferAuthority: Account,
    maximumTokenB: number | Numberu64,
  ): Promise<TransactionSignature> {
    return await sendAndConfirmTransaction(
      'depositAllTokenTypes',
      this.connection,
      new Transaction().add(
        TokenSwap.depositAllTokenTypesInstruction(
          this.tokenSwap,
          this.authority,
          
        ),
      ),
      this.payer,
      userTransferAuthority,
    );
  }

  static startFarmingInstruction(
    tokenSwap: PublicKey,
    authority: PublicKey,
    userTransferAuthority: PublicKey,
    sourceA: PublicKey,
    sourceB: PublicKey,
    intoA: PublicKey,
    intoB: PublicKey,
    poolToken: PublicKey,
    poolAccount: PublicKey,
    swapProgramId: PublicKey,
    tokenProgramId: PublicKey,
    poolTokenAmount: number | Numberu64,
    maximumTokenA: number | Numberu64,
    maximumTokenB: number | Numberu64,
  ): TransactionInstruction {
    const dataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),
      Layout.uint64('poolTokenAmount'),
      Layout.uint64('maximumTokenA'),
      Layout.uint64('maximumTokenB'),
    ]);

    const data = Buffer.alloc(dataLayout.span);
    dataLayout.encode(
      {
        instruction: 2, // Deposit instruction
        poolTokenAmount: new Numberu64(poolTokenAmount).toBuffer(),
        maximumTokenA: new Numberu64(maximumTokenA).toBuffer(),
        maximumTokenB: new Numberu64(maximumTokenB).toBuffer(),
      },
      data,
    );

    const keys = [
      {pubkey: tokenSwap, isSigner: false, isWritable: false},
      {pubkey: authority, isSigner: false, isWritable: false},
      {pubkey: userTransferAuthority, isSigner: true, isWritable: false},
      {pubkey: sourceA, isSigner: false, isWritable: true},
      {pubkey: sourceB, isSigner: false, isWritable: true},
      {pubkey: intoA, isSigner: false, isWritable: true},
      {pubkey: intoB, isSigner: false, isWritable: true},
      {pubkey: poolToken, isSigner: false, isWritable: true},
      {pubkey: poolAccount, isSigner: false, isWritable: true},
      {pubkey: tokenProgramId, isSigner: false, isWritable: false},
    ];
    return new TransactionInstruction({
      keys,
      programId: swapProgramId,
      data,
    });
  }

  /**
   * Withdraw tokens from the pool
   *
   * @param userAccountA User account for token A
   * @param userAccountB User account for token B
   * @param poolAccount User account for pool token
   * @param userTransferAuthority Account delegated to transfer user's tokens
   * @param poolTokenAmount Amount of pool tokens to burn
   * @param minimumTokenA The minimum amount of token A to withdraw
   * @param minimumTokenB The minimum amount of token B to withdraw
   */
  async endFarming(
    poolAccount: PublicKey,
    userTransferAuthority: Account,
    minimumTokenB: number | Numberu64,
  ): Promise<TransactionSignature> {
    return await sendAndConfirmTransaction(
      'withdraw',
      this.connection,
      new Transaction().add(
        TokenFarming.endFarmingInstruction(
          this.tokenSwap,         
          minimumTokenB,
        ),
      ),
      this.payer,
      userTransferAuthority,
    );
  }

  static endFarmingInstruction(
    tokenSwap: PublicKey,
    authority: PublicKey,
    userTransferAuthority: PublicKey,
    poolMint: PublicKey,
    feeAccount: PublicKey,
    sourcePoolAccount: PublicKey,
    fromA: PublicKey,
    fromB: PublicKey,
    userAccountA: PublicKey,
    userAccountB: PublicKey,
    swapProgramId: PublicKey,
    tokenProgramId: PublicKey,
    poolTokenAmount: number | Numberu64,
    minimumTokenA: number | Numberu64,
    minimumTokenB: number | Numberu64,
  ): TransactionInstruction {
    const dataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),
      Layout.uint64('poolTokenAmount'),
      Layout.uint64('minimumTokenA'),
      Layout.uint64('minimumTokenB'),
    ]);

    const data = Buffer.alloc(dataLayout.span);
    dataLayout.encode(
      {
        instruction: 3, // Withdraw instruction
        poolTokenAmount: new Numberu64(poolTokenAmount).toBuffer(),
        minimumTokenA: new Numberu64(minimumTokenA).toBuffer(),
        minimumTokenB: new Numberu64(minimumTokenB).toBuffer(),
      },
      data,
    );

    const keys = [
      {pubkey: tokenSwap, isSigner: false, isWritable: false},
      {pubkey: authority, isSigner: false, isWritable: false},
      {pubkey: userTransferAuthority, isSigner: true, isWritable: false},
      {pubkey: poolMint, isSigner: false, isWritable: true},
      {pubkey: sourcePoolAccount, isSigner: false, isWritable: true},
      {pubkey: fromA, isSigner: false, isWritable: true},
      {pubkey: fromB, isSigner: false, isWritable: true},
      {pubkey: userAccountA, isSigner: false, isWritable: true},
      {pubkey: userAccountB, isSigner: false, isWritable: true},
      {pubkey: feeAccount, isSigner: false, isWritable: true},
      {pubkey: tokenProgramId, isSigner: false, isWritable: false},
    ];
    return new TransactionInstruction({
      keys,
      programId: swapProgramId,
      data,
    });
  }

  /**
   * Deposit one side of tokens into the pool
   * @param userAccount User account to deposit token A or B
   * @param poolAccount User account to receive pool tokens
   * @param userTransferAuthority Account delegated to transfer user's tokens
   * @param sourceTokenAmount The amount of token A or B to deposit
   * @param minimumPoolTokenAmount Minimum amount of pool tokens to mint
   */
  async withdrawFarmed(
    userAccount: PublicKey,
    poolAccount: PublicKey,
    userTransferAuthority: Account,
    sourceTokenAmount: number | Numberu64,
    minimumPoolTokenAmount: number | Numberu64,
  ): Promise<TransactionSignature> {
    return await sendAndConfirmTransaction(
      'depositSingleTokenTypeExactAmountIn',
      this.connection,
      new Transaction().add(
        TokenSwap.depositSingleTokenTypeExactAmountInInstruction(
          this.tokenSwap,
          this.authority,
          userTransferAuthority.publicKey,
          userAccount,
          this.tokenAccountA,
          this.tokenAccountB,
          this.poolToken,
          poolAccount,
          this.swapProgramId,
          this.tokenProgramId,
          sourceTokenAmount,
          minimumPoolTokenAmount,
        ),
      ),
      this.payer,
      userTransferAuthority,
    );
  }

  static withdrawFarmedInstruction(
    tokenSwap: PublicKey,
    authority: PublicKey,
    userTransferAuthority: PublicKey,
    source: PublicKey,
    intoA: PublicKey,
    intoB: PublicKey,
    poolToken: PublicKey,
    poolAccount: PublicKey,
    swapProgramId: PublicKey,
    tokenProgramId: PublicKey,
    sourceTokenAmount: number | Numberu64,
    minimumPoolTokenAmount: number | Numberu64,
  ): TransactionInstruction {
    const dataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),
      Layout.uint64('sourceTokenAmount'),
      Layout.uint64('minimumPoolTokenAmount'),
    ]);

    const data = Buffer.alloc(dataLayout.span);
    dataLayout.encode(
      {
        instruction: 4, // depositSingleTokenTypeExactAmountIn instruction
        sourceTokenAmount: new Numberu64(sourceTokenAmount).toBuffer(),
        minimumPoolTokenAmount: new Numberu64(
          minimumPoolTokenAmount,
        ).toBuffer(),
      },
      data,
    );

    const keys = [
      {pubkey: tokenSwap, isSigner: false, isWritable: false},
      {pubkey: authority, isSigner: false, isWritable: false},
      {pubkey: userTransferAuthority, isSigner: true, isWritable: false},
      {pubkey: source, isSigner: false, isWritable: true},
      {pubkey: intoA, isSigner: false, isWritable: true},
      {pubkey: intoB, isSigner: false, isWritable: true},
      {pubkey: poolToken, isSigner: false, isWritable: true},
      {pubkey: poolAccount, isSigner: false, isWritable: true},
      {pubkey: tokenProgramId, isSigner: false, isWritable: false},
    ];
    return new TransactionInstruction({
      keys,
      programId: swapProgramId,
      data,
    });
  }

  /**
   * Withdraw tokens from the pool
   *
   * @param userAccount User account to receive token A or B
   * @param poolAccount User account to burn pool token
   * @param userTransferAuthority Account delegated to transfer user's tokens
   * @param destinationTokenAmount The amount of token A or B to withdraw
   * @param maximumPoolTokenAmount Maximum amount of pool tokens to burn
   */
  
}
