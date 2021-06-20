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
import { Numberu64 } from '.';


/**
 * @private
 */
export const TokenFarmingLayout: typeof BufferLayout.Structure = BufferLayout.struct(
  [
    Layout.uint64('discriminator'),
    BufferLayout.u8('isInitialized'),
    Layout.uint64('tokensUnlocked'),
    Layout.uint64('tokensPerPeriod'),
    Layout.uint64('tokensTotal'),
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
    Layout.uint64('discriminator'),
    BufferLayout.u8('isInitialized'),
    Layout.uint64('tokensFrozen'),
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
    private connection: Connection,
    public tokenSwapAccount: PublicKey,
    public farmingStateAccount: PublicKey,
    public tokenFreezeAccount: PublicKey,
    public farmingTokenAccount: PublicKey,
    public feeAccountPubkey: PublicKey,
    public swapAuthority: PublicKey,
    public tokenProgramId: PublicKey,
    public swapProgramId: PublicKey,
    public tokenAmount: Numberu64,
    public tokensPerPeriod: Numberu64,
    public periodLength: Numberu64,
    public payer: Account,
  ) {    
      this.connection = connection;
      this.tokenSwapAccount = tokenSwapAccount;
      this.farmingStateAccount = farmingStateAccount;
      this.tokenFreezeAccount = tokenFreezeAccount;
      this.farmingTokenAccount = farmingTokenAccount;
      this.feeAccountPubkey = feeAccountPubkey;
      this.swapAuthority = swapAuthority;
      this.tokenProgramId = tokenProgramId;
      this.swapProgramId = swapProgramId;
      this.tokenAmount = tokenAmount;
      this.tokensPerPeriod = tokensPerPeriod;
      this.periodLength = periodLength;
      this.payer = payer;
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

  static async getMinBalanceRentForExemptFarmingTicket(
    connection: Connection,
  ): Promise<number> {
    return await connection.getMinimumBalanceForRentExemption(
      FarmingTicketLayout.span,
    );
  }

  static createInitFarmingInstruction(
    tokenSwapAccount: PublicKey,
    farmingStateAccount: PublicKey,    
    farmingTokenAccount: PublicKey,
    userFarmingTokenAccount: PublicKey,    
    userTransferAuthority: PublicKey,
    feeAccount: PublicKey,
    feeAuthority: PublicKey,
    swapAuthority: PublicKey,
    clock: PublicKey,
    tokenProgramId: PublicKey,
    swapProgramId: PublicKey,
    tokenAmount: number,    
    tokensPerPeriod: number,  
    periodLength: number,
  ): TransactionInstruction {
    const keys = [
      {pubkey: tokenSwapAccount, isSigner: false, isWritable: true},
      {pubkey: farmingStateAccount, isSigner: false, isWritable: true},
      {pubkey: farmingTokenAccount, isSigner: false, isWritable: true},
      {pubkey: userFarmingTokenAccount, isSigner: false, isWritable: true},
      {pubkey: userTransferAuthority, isSigner: true, isWritable: false},
      {pubkey: feeAccount, isSigner: false, isWritable: false},
      {pubkey: feeAuthority, isSigner: true, isWritable: false},
      {pubkey: swapAuthority, isSigner: false, isWritable: false},
      {pubkey: clock, isSigner: false, isWritable: false},
      {pubkey: tokenProgramId, isSigner: false, isWritable: false},
    ];
    const commandDataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),
      BufferLayout.nu64('tokenAmount'),
      BufferLayout.nu64('tokensPerPeriod'),
      BufferLayout.nu64('periodLength'),      
    ]);
    let data = Buffer.alloc(25);
    {
      const encodeLength = commandDataLayout.encode(
        {
          instruction: 9, // InitializeFarming instruction
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
    tokenFreezeAccountPubkey: PublicKey,
    swapAuthority: PublicKey,
    feeAccountPubkey: PublicKey,
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
    
    const swapAccount = new PublicKey(tokenFarmingData.attachedSwapAccount);    
    const farmingTokenAccount = new PublicKey(tokenFarmingData.farmingTokenAccount);  
   
    console.log("unlocked " + Numberu64.fromBuffer(tokenFarmingData.tokensUnlocked).toString());
    console.log("total " + Numberu64.fromBuffer(tokenFarmingData.tokensTotal).toString());
    console.log("tokens per period " + Numberu64.fromBuffer(tokenFarmingData.tokensPerPeriod).toString());

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
      tokenFreezeAccountPubkey,
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
   * Create a new Token Farming
   * @return New object holding FarmingState fields to be used in calls to the farming feature
   */
  static async initializeTokenFarming(   
    connection: Connection,
    tokenSwapAccount: PublicKey,
    farmingStateAccount: PublicKey,
    tokenFreezeAccount: PublicKey,
    farmingTokenAccount: PublicKey,
    feeAccountPubkey: PublicKey,
    swapAuthority: PublicKey,
    userFarmingTokenAccount: PublicKey,    
    userTransferAuthority: PublicKey,
    clock: PublicKey,
    tokenProgramId: PublicKey,
    swapProgramId: PublicKey,
    tokenAmount: number,
    tokensPerPeriod: number,
    periodLength: number,
    payer: Account,
    authority: Account,
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
      farmingStateAccount,
      farmingTokenAccount,
      userFarmingTokenAccount,
      userTransferAuthority,
      feeAccountPubkey,
      authority.publicKey,
      swapAuthority,
      clock,
      tokenProgramId,
      swapProgramId,
      tokenAmount,
      tokensPerPeriod,
      periodLength,
    );

    transaction.add(instruction);
    await sendAndConfirmTransaction(
      'InitializeTokenFarming',
      connection,
      transaction,
      payer,     
      authority 
    );

    return tokenFarming;
  }

  /**
   * Take a farming snapshot
   */
  async takeFarmingSnapshot(
    clock: PublicKey,
    authority: Account,
  ): Promise<TransactionSignature> {
    return await sendAndConfirmTransaction(
      'takeFarmingSnapshot',
      this.connection,
      new Transaction().add(
        TokenFarming.takeFarmingSnapshotInstruction(
          this.tokenSwapAccount,
          this.farmingStateAccount,
          this.tokenFreezeAccount,
          this.feeAccountPubkey,
          clock,
          this.swapProgramId,
          authority.publicKey,
        ),
      ),
      this.payer,
      authority,
    );
  }

  static takeFarmingSnapshotInstruction(
    tokenSwap: PublicKey,
    farmingState: PublicKey,
    tokenFreezee: PublicKey,
    feeAccount: PublicKey,
    clock: PublicKey,
    swapProgramId: PublicKey,
    authority: PublicKey,
  ): TransactionInstruction {

    const dataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),            
    ]);

    const data = Buffer.alloc(dataLayout.span);
    dataLayout.encode(
      {
        instruction: 10, // takeFarmingSnapshot instruction                
      },
      data,
    );

    const keys = [
      {pubkey: tokenSwap, isSigner: false, isWritable: false},
      {pubkey: farmingState, isSigner: false, isWritable: true},
      {pubkey: tokenFreezee, isSigner: false, isWritable: false},
      {pubkey: feeAccount, isSigner: false, isWritable: false},
      {pubkey: authority, isSigner: true, isWritable: false},
      {pubkey: clock, isSigner: false, isWritable: false},    
    ];
   
    return new TransactionInstruction({
      keys,
      programId: swapProgramId,
      data,
    });
  }

  /**
   * Start token farming
   */
  async startFarming(
    farmingTicketAccount: PublicKey,
    userPoolTokenAccount: PublicKey,
    userTransferAuthority: Account,
    userWalletAuthority: Account,
    clock: PublicKey,
    poolTokenAmount: number | Numberu64,
  ): Promise<TransactionSignature> {
    return await sendAndConfirmTransaction(
      'startFarming',
      this.connection,
      new Transaction().add(
        TokenFarming.startFarmingInstruction(
          this.tokenSwapAccount,
          this.farmingStateAccount,
          farmingTicketAccount,
          this.tokenFreezeAccount,
          userPoolTokenAccount,
          userTransferAuthority.publicKey,
          userWalletAuthority.publicKey,
          this.tokenProgramId,
          clock,
          this.swapProgramId,
          poolTokenAmount
        ),
      ),
      this.payer,
      userTransferAuthority,
      userWalletAuthority
    );
  }

  static startFarmingInstruction(
    tokenSwap: PublicKey,
    farmingState: PublicKey,
    farmingTicketAccount: PublicKey,
    swapTokenFreezeAccount: PublicKey,
    userPoolTokenAccount: PublicKey,
    userAuthority: PublicKey,
    userPubkey: PublicKey,
    tokenProgramId: PublicKey,
    clock: PublicKey,
    swapProgramId: PublicKey,
    poolTokenAmount: number | Numberu64,
  ): TransactionInstruction {
    const dataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),
      Layout.uint64('poolTokenAmount'),      
    ]);

    const data = Buffer.alloc(dataLayout.span);
    dataLayout.encode(
      {
        instruction: 6, // startFarming instruction
        poolTokenAmount: new Numberu64(poolTokenAmount).toBuffer(),        
      },
      data,
    );

    const keys = [
      {pubkey: tokenSwap, isSigner: false, isWritable: false},
      {pubkey: farmingState, isSigner: false, isWritable: false},
      {pubkey: farmingTicketAccount, isSigner: false, isWritable: true},
      {pubkey: swapTokenFreezeAccount, isSigner: false, isWritable: true},
      {pubkey: userPoolTokenAccount, isSigner: false, isWritable: true},
      {pubkey: userAuthority, isSigner: true, isWritable: false},
      {pubkey: userPubkey, isSigner: true, isWritable: false},
      {pubkey: tokenProgramId, isSigner: false, isWritable: false},
      {pubkey: clock, isSigner: false, isWritable: false},
    ];
    return new TransactionInstruction({
      keys,
      programId: swapProgramId,
      data,
    });
  }

  /**
   * End token farming
   */
  async endFarming(
    farmingTicketAccount: PublicKey,
    userPoolTokenAccount: PublicKey,    
    userWallet: Account,
    clock: PublicKey,
  ): Promise<TransactionSignature> {
    return await sendAndConfirmTransaction(
      'endFarming',
      this.connection,
      new Transaction().add(
        TokenFarming.endFarmingInstruction(
          this.tokenSwapAccount,         
          this.farmingStateAccount,
          farmingTicketAccount,
          this.tokenFreezeAccount,          
          this.swapAuthority,
          userPoolTokenAccount,
          userWallet.publicKey,
          clock,
          this.tokenProgramId,
          this.swapProgramId,
        ),
      ),
      this.payer,      
      userWallet
    );
  }

  static endFarmingInstruction(
    tokenSwap: PublicKey,
    farmingState: PublicKey,
    farmingTicket: PublicKey,
    tokenFreezeAccount: PublicKey,    
    swapAuthority: PublicKey,
    userPoolTokenAccount: PublicKey,
    userPubkey: PublicKey,
    clock: PublicKey,
    tokenProgramId: PublicKey,
    swapProgramId: PublicKey,
  ): TransactionInstruction {
    const dataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),   
    ]);

    const data = Buffer.alloc(dataLayout.span);
    dataLayout.encode(
      {
        instruction: 8, // endFarming instruction       
      },
      data,
    );

    const keys = [
      {pubkey: tokenSwap, isSigner: false, isWritable: false},
      {pubkey: farmingState, isSigner: false, isWritable: false},
      {pubkey: farmingTicket, isSigner: false, isWritable: true},
      {pubkey: tokenFreezeAccount, isSigner: false, isWritable: true},
      {pubkey: swapAuthority, isSigner: false, isWritable: false},
      {pubkey: userPoolTokenAccount, isSigner: false, isWritable: true},      
      {pubkey: userPubkey, isSigner: true, isWritable: false},
      {pubkey: clock, isSigner: false, isWritable: false},
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
    farmingTicket: PublicKey,
    userFarmingTokenAccount: PublicKey,
    userWallet: Account,
    clock: PublicKey,
  ): Promise<TransactionSignature> {
    return await sendAndConfirmTransaction(
      'withdrawFarmed',
      this.connection,
      new Transaction().add(
        TokenFarming.withdrawFarmedInstruction(
          this.tokenSwapAccount,
          this.farmingStateAccount,
          farmingTicket,
          this.farmingTokenAccount,
          this.swapAuthority,
          userFarmingTokenAccount,
          userWallet.publicKey,
          clock,
          this.tokenProgramId,
          this.swapProgramId
        ),
      ),
      this.payer,  
      userWallet    
    );
  }

  static withdrawFarmedInstruction(
    tokenSwap: PublicKey,
    farmingState: PublicKey,
    farmingTicket: PublicKey,
    swapFarmingTokenAccount: PublicKey,
    swapAuthority: PublicKey,
    userFarmingTokenAccount: PublicKey,
    userPubkey: PublicKey,
    clock: PublicKey,
    tokenProgramId: PublicKey,
    swapProgramId  : PublicKey,
  ): TransactionInstruction {
    const dataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),    
    ]);

    const data = Buffer.alloc(dataLayout.span);
    dataLayout.encode(
      {
        instruction: 7, // withdrawFarmed instruction       
      },
      data,
    );

    const keys = [
      {pubkey: tokenSwap, isSigner: false, isWritable: false},
      {pubkey: farmingState, isSigner: false, isWritable: false},
      {pubkey: farmingTicket, isSigner: false, isWritable: true},
      {pubkey: swapFarmingTokenAccount, isSigner: false, isWritable: true},
      {pubkey: swapAuthority, isSigner: false, isWritable: false},
      {pubkey: userFarmingTokenAccount, isSigner: false, isWritable: true},
      {pubkey: userPubkey, isSigner: true, isWritable: false},
      {pubkey: clock, isSigner: false, isWritable: false},
      {pubkey: tokenProgramId, isSigner: false, isWritable: false},
    ];
    return new TransactionInstruction({
      keys,
      programId: swapProgramId,
      data,
    });
  }  
}
