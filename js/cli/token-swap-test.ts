import {
  Account,
  Connection,
  PublicKey,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  Transaction,
} from '@solana/web3.js';
import {AccountLayout, Token, TOKEN_PROGRAM_ID} from '@solana/spl-token';

import {TokenSwap, CurveType, TOKEN_SWAP_PROGRAM_ID, Numberu64} from '../src';
import {sendAndConfirmTransaction} from '../src/util/send-and-confirm-transaction';
import {newAccountWithLamports} from '../src/util/new-account-with-lamports';
import {url} from '../src/util/url';
import {sleep} from '../src/util/sleep';
import { FarmingTicketLayout, TokenFarming, TokenFarmingLayout } from '../src/token-farming';

// The following globals are created by `createTokenSwap` and used by subsequent tests
// Token swap
let tokenSwap: TokenSwap;
let tokenFarming: TokenFarming;
// authority of the token and accounts
let authority: PublicKey;
// nonce used to generate the authority public key
let nonce: number;
// owner of the user accounts
let owner: Account;
// Token pool
let tokenPool: Token;
let tokenAccountPool: PublicKey;
let feeAccount: PublicKey;
let farmingAccount: PublicKey;
let farmingTicket: PublicKey;
// Tokens swapped
let mintA: Token;
let mintB: Token;
let farmingTokenMint: Token;
let tokenAccountA: PublicKey;
let tokenAccountB: PublicKey;

let newPoolTokenAccount: PublicKey;

// Hard-coded fee address, for testing production mode
const SWAP_PROGRAM_OWNER_FEE_ADDRESS = '9VHVV44zDSmmdDMUHk4fwotXioimN78yzNDgzaVUP5Fb';

// Pool fees
const TRADING_FEE_NUMERATOR = 25;
const TRADING_FEE_DENOMINATOR = 10000;
const OWNER_TRADING_FEE_NUMERATOR = 5;
const OWNER_TRADING_FEE_DENOMINATOR = 10000;
const OWNER_WITHDRAW_FEE_NUMERATOR = 0;
const OWNER_WITHDRAW_FEE_DENOMINATOR = 0;
const HOST_FEE_NUMERATOR = 0;
const HOST_FEE_DENOMINATOR = 0;

// curve type used to calculate swaps and deposits
const CURVE_TYPE = CurveType.ConstantProduct;

// Initial amount in each swap token
let currentSwapTokenA = 1000000;
let currentSwapTokenB = 1000000;
let currentFeeAmount = 0;

// Swap instruction constants
// Because there is no withdraw fee in the production version, these numbers
// need to get slightly tweaked in the two cases.
const SWAP_AMOUNT_IN = 100000;
const SWAP_AMOUNT_OUT = SWAP_PROGRAM_OWNER_FEE_ADDRESS ? 90661 : 90674;
const SWAP_FEE = SWAP_PROGRAM_OWNER_FEE_ADDRESS ? 22273 : 22277;
const HOST_SWAP_FEE = 0;
const OWNER_SWAP_FEE = SWAP_FEE - HOST_SWAP_FEE;

// Pool token amount minted on init
const DEFAULT_POOL_TOKEN_AMOUNT = 1000000000;
// Pool token amount to withdraw / deposit
const POOL_TOKEN_AMOUNT = 10000000;

function assert(condition: boolean, message?: string) {
  if (!condition) {
    console.log(Error().stack + ':token-test.js');
    throw message || 'Assertion failed';
  }
}

let connection: Connection;
async function getConnection(): Promise<Connection> {
  if (connection) return connection;

  connection = new Connection(url, 'recent');
  const version = await connection.getVersion();

  console.log('Connection to cluster established:', url, version);
  return connection;
}

export async function createTokenSwap(): Promise<void> {
  const connection = await getConnection();
  const payer = await newAccountWithLamports(connection, 10000000000);
  owner = await newAccountWithLamports(connection, 1000000000);
  const tokenSwapAccount = new Account();

  [authority, nonce] = await PublicKey.findProgramAddress(
    [tokenSwapAccount.publicKey.toBuffer()],
    TOKEN_SWAP_PROGRAM_ID,
  );

  console.log('creating pool mint');
  tokenPool = await Token.createMint(
    connection,
    payer,
    authority,
    null,
    2,
    TOKEN_PROGRAM_ID,
  );

  console.log('creating pool account');
  tokenAccountPool = await tokenPool.createAccount(owner.publicKey);
  const ownerKey = SWAP_PROGRAM_OWNER_FEE_ADDRESS || owner.publicKey.toString();
  feeAccount = await tokenPool.createAccount(new PublicKey(ownerKey));

  console.log('creating token A');
  mintA = await Token.createMint(
    connection,
    payer,
    owner.publicKey,
    null,
    2,
    TOKEN_PROGRAM_ID,
  );

  console.log('creating token A account');
  tokenAccountA = await mintA.createAccount(authority);
  console.log('minting token A to swap');
  await mintA.mintTo(tokenAccountA, owner, [], currentSwapTokenA);

  console.log('creating token B');
  mintB = await Token.createMint(
    connection,
    payer,
    owner.publicKey,
    null,
    2,
    TOKEN_PROGRAM_ID,
  );

  console.log('creating token B account');
  tokenAccountB = await mintB.createAccount(authority);
  console.log('minting token B to swap');
  await mintB.mintTo(tokenAccountB, owner, [], currentSwapTokenB);

  console.log('createing token freeze account');
  const tokenFreezeAccount = await tokenPool.createAccount(authority);
  const balanceNeeded = await TokenFarming.getMinBalanceRentForExemptTokenSwap(
    connection,
  );

  console.log('creating farming account');
  const farmingState = new Account();
  await sendAndConfirmTransaction('create farmingState account', connection, 
  new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: payer.publicKey,
      newAccountPubkey: farmingState.publicKey,
      lamports: balanceNeeded,
      space: TokenFarmingLayout.span,
      programId: TOKEN_SWAP_PROGRAM_ID,
    }),
  ), payer, farmingState);


  console.log(TOKEN_PROGRAM_ID.toBase58());
  console.log('creating token swap');
  const swapPayer = await newAccountWithLamports(connection, 10000000000);
  tokenSwap = await TokenSwap.createTokenSwap(
    connection,
    swapPayer,
    tokenSwapAccount,
    authority,
    tokenAccountA,
    tokenAccountB,
    tokenPool.publicKey,
    mintA.publicKey,
    mintB.publicKey,
    feeAccount,
    tokenAccountPool,
    tokenFreezeAccount,
    TOKEN_SWAP_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
    nonce,
    TRADING_FEE_NUMERATOR,
    TRADING_FEE_DENOMINATOR,
    OWNER_TRADING_FEE_NUMERATOR,
    OWNER_TRADING_FEE_DENOMINATOR,
    OWNER_WITHDRAW_FEE_NUMERATOR,
    OWNER_WITHDRAW_FEE_DENOMINATOR,
    HOST_FEE_NUMERATOR,
    HOST_FEE_DENOMINATOR,
    CURVE_TYPE,
    farmingState.publicKey,
  );

  console.log('loading token swap');
  const fetchedTokenSwap = await TokenSwap.loadTokenSwap(
    connection,
    tokenSwapAccount.publicKey,
    TOKEN_SWAP_PROGRAM_ID,
    swapPayer,
  );

  assert(fetchedTokenSwap.tokenProgramId.equals(TOKEN_PROGRAM_ID));
  assert(fetchedTokenSwap.tokenAccountA.equals(tokenAccountA));
  assert(fetchedTokenSwap.tokenAccountB.equals(tokenAccountB));
  assert(fetchedTokenSwap.mintA.equals(mintA.publicKey));
  assert(fetchedTokenSwap.mintB.equals(mintB.publicKey));
  assert(fetchedTokenSwap.poolToken.equals(tokenPool.publicKey));
  assert(fetchedTokenSwap.feeAccount.equals(feeAccount));
  assert(
    TRADING_FEE_NUMERATOR == fetchedTokenSwap.tradeFeeNumerator.toNumber(),
  );
  assert(
    TRADING_FEE_DENOMINATOR == fetchedTokenSwap.tradeFeeDenominator.toNumber(),
  );
  assert(
    OWNER_TRADING_FEE_NUMERATOR ==
      fetchedTokenSwap.ownerTradeFeeNumerator.toNumber(),
  );
  assert(
    OWNER_TRADING_FEE_DENOMINATOR ==
      fetchedTokenSwap.ownerTradeFeeDenominator.toNumber(),
  );
  assert(
    OWNER_WITHDRAW_FEE_NUMERATOR ==
      fetchedTokenSwap.ownerWithdrawFeeNumerator.toNumber(),
  );
  assert(
    OWNER_WITHDRAW_FEE_DENOMINATOR ==
      fetchedTokenSwap.ownerWithdrawFeeDenominator.toNumber(),
  );
  assert(HOST_FEE_NUMERATOR == fetchedTokenSwap.hostFeeNumerator.toNumber());
  assert(
    HOST_FEE_DENOMINATOR == fetchedTokenSwap.hostFeeDenominator.toNumber(),
  );
  assert(CURVE_TYPE == fetchedTokenSwap.curveType);
}

export async function depositAllTokenTypes(): Promise<void> {
  const poolMintInfo = await tokenPool.getMintInfo();
  const supply = poolMintInfo.supply.toNumber();
  const swapTokenA = await mintA.getAccountInfo(tokenAccountA);
  const tokenA = Math.floor(
    (swapTokenA.amount.toNumber() * POOL_TOKEN_AMOUNT) / supply,
  );
  const swapTokenB = await mintB.getAccountInfo(tokenAccountB);
  const tokenB = Math.floor(
    (swapTokenB.amount.toNumber() * POOL_TOKEN_AMOUNT) / supply,
  );

  const userTransferAuthority = new Account();
  console.log('Creating depositor token a account');
  const userAccountA = await mintA.createAccount(owner.publicKey);
  await mintA.mintTo(userAccountA, owner, [], tokenA);
  await mintA.approve(
    userAccountA,
    userTransferAuthority.publicKey,
    owner,
    [],
    tokenA,
  );
  console.log('Creating depositor token b account');
  const userAccountB = await mintB.createAccount(owner.publicKey);
  await mintB.mintTo(userAccountB, owner, [], tokenB);
  await mintB.approve(
    userAccountB,
    userTransferAuthority.publicKey,
    owner,
    [],
    tokenB,
  );
  console.log('Creating depositor pool token account');
  const newAccountPool = await tokenPool.createAccount(owner.publicKey);

  console.log('Depositing into swap');
  await tokenSwap.depositAllTokenTypes(
    userAccountA,
    userAccountB,
    newAccountPool,
    userTransferAuthority,
    POOL_TOKEN_AMOUNT,
    tokenA,
    tokenB,
  );

  let info;
  info = await mintA.getAccountInfo(userAccountA);
  assert(info.amount.toNumber() == 0);
  info = await mintB.getAccountInfo(userAccountB);
  assert(info.amount.toNumber() == 0);
  info = await mintA.getAccountInfo(tokenAccountA);
  assert(info.amount.toNumber() == currentSwapTokenA + tokenA);
  currentSwapTokenA += tokenA;
  info = await mintB.getAccountInfo(tokenAccountB);
  assert(info.amount.toNumber() == currentSwapTokenB + tokenB);
  currentSwapTokenB += tokenB;
  info = await tokenPool.getAccountInfo(newAccountPool);
  assert(info.amount.toNumber() == POOL_TOKEN_AMOUNT);
}

export async function withdrawAllTokenTypes(): Promise<void> {
  const poolMintInfo = await tokenPool.getMintInfo();
  const supply = poolMintInfo.supply.toNumber();
  let swapTokenA = await mintA.getAccountInfo(tokenAccountA);
  let swapTokenB = await mintB.getAccountInfo(tokenAccountB);
  let feeAmount = 0;
  if (OWNER_WITHDRAW_FEE_NUMERATOR !== 0) {
    feeAmount = Math.floor(
      (POOL_TOKEN_AMOUNT * OWNER_WITHDRAW_FEE_NUMERATOR) /
        OWNER_WITHDRAW_FEE_DENOMINATOR,
    );
  }
  const poolTokenAmount = POOL_TOKEN_AMOUNT - feeAmount;
  const tokenA = Math.floor(
    (swapTokenA.amount.toNumber() * poolTokenAmount) / supply,
  );
  const tokenB = Math.floor(
    (swapTokenB.amount.toNumber() * poolTokenAmount) / supply,
  );

  console.log('Creating withdraw token A account');
  let userAccountA = await mintA.createAccount(owner.publicKey);
  console.log('Creating withdraw token B account');
  let userAccountB = await mintB.createAccount(owner.publicKey);

  const userTransferAuthority = new Account();
  console.log('Approving withdrawal from pool account');
  await tokenPool.approve(
    tokenAccountPool,
    userTransferAuthority.publicKey,
    owner,
    [],
    POOL_TOKEN_AMOUNT,
  );

  console.log('Withdrawing pool tokens for A and B tokens');
  await tokenSwap.withdrawAllTokenTypes(
    userAccountA,
    userAccountB,
    tokenAccountPool,
    userTransferAuthority,
    POOL_TOKEN_AMOUNT,
    tokenA,
    tokenB,
  );

  //const poolMintInfo = await tokenPool.getMintInfo();
  swapTokenA = await mintA.getAccountInfo(tokenAccountA);
  swapTokenB = await mintB.getAccountInfo(tokenAccountB);

  let info = await tokenPool.getAccountInfo(tokenAccountPool);
  assert(
    info.amount.toNumber() == DEFAULT_POOL_TOKEN_AMOUNT - POOL_TOKEN_AMOUNT,
  );
  assert(swapTokenA.amount.toNumber() == currentSwapTokenA - tokenA);
  currentSwapTokenA -= tokenA;
  assert(swapTokenB.amount.toNumber() == currentSwapTokenB - tokenB);
  currentSwapTokenB -= tokenB;
  info = await mintA.getAccountInfo(userAccountA);
  assert(info.amount.toNumber() == tokenA);
  info = await mintB.getAccountInfo(userAccountB);
  assert(info.amount.toNumber() == tokenB);
  info = await tokenPool.getAccountInfo(feeAccount);
  assert(info.amount.toNumber() == feeAmount);
  currentFeeAmount = feeAmount;
}

export async function createAccountAndSwapAtomic(): Promise<void> {
  console.log('Creating swap token a account');
  let userAccountA = await mintA.createAccount(owner.publicKey);
  await mintA.mintTo(userAccountA, owner, [], SWAP_AMOUNT_IN);

  // @ts-ignore
  const balanceNeeded = await Token.getMinBalanceRentForExemptAccount(
    connection,
  );
  const newAccount = new Account();
  const transaction = new Transaction();
  transaction.add(
    SystemProgram.createAccount({
      fromPubkey: owner.publicKey,
      newAccountPubkey: newAccount.publicKey,
      lamports: balanceNeeded,
      space: AccountLayout.span,
      programId: mintB.programId,
    }),
  );

  transaction.add(
    Token.createInitAccountInstruction(
      mintB.programId,
      mintB.publicKey,
      newAccount.publicKey,
      owner.publicKey,
    ),
  );

  const userTransferAuthority = new Account();
  transaction.add(
    Token.createApproveInstruction(
      mintA.programId,
      userAccountA,
      userTransferAuthority.publicKey,
      owner.publicKey,
      [owner],
      SWAP_AMOUNT_IN,
    ),
  );

  transaction.add(
    TokenSwap.swapInstruction(
      tokenSwap.tokenSwap,
      tokenSwap.authority,
      userTransferAuthority.publicKey,
      userAccountA,
      tokenSwap.tokenAccountA,
      tokenSwap.tokenAccountB,
      newAccount.publicKey,
      tokenSwap.poolToken,
      tokenSwap.feeAccount,
      null,
      tokenSwap.swapProgramId,
      tokenSwap.tokenProgramId,
      SWAP_AMOUNT_IN,
      0,
    ),
  );

  // Send the instructions
  console.log('sending big instruction');
  await sendAndConfirmTransaction(
    'create account, approve transfer, swap',
    connection,
    transaction,
    owner,
    newAccount,
    userTransferAuthority,
  );

  let info;
  info = await mintA.getAccountInfo(tokenAccountA);
  currentSwapTokenA = info.amount.toNumber();
  info = await mintB.getAccountInfo(tokenAccountB);
  currentSwapTokenB = info.amount.toNumber();
}

export async function swap(): Promise<void> {
  console.log('Creating swap token a account');
  let userAccountA = await mintA.createAccount(owner.publicKey);
  await mintA.mintTo(userAccountA, owner, [], SWAP_AMOUNT_IN);
  const userTransferAuthority = new Account();
  await mintA.approve(
    userAccountA,
    userTransferAuthority.publicKey,
    owner,
    [],
    SWAP_AMOUNT_IN,
  );
  console.log('Creating swap token b account');
  let userAccountB = await mintB.createAccount(owner.publicKey);
  let poolAccount = SWAP_PROGRAM_OWNER_FEE_ADDRESS
    ? await tokenPool.createAccount(owner.publicKey)
    : null;

  console.log('Swapping');
  await tokenSwap.swap(
    userAccountA,
    tokenAccountA,
    tokenAccountB,
    userAccountB,
    poolAccount,
    userTransferAuthority,
    SWAP_AMOUNT_IN,
    SWAP_AMOUNT_OUT,
  );

  await sleep(500);

  let info;
  info = await mintA.getAccountInfo(userAccountA);
  assert(info.amount.toNumber() == 0);

  info = await mintB.getAccountInfo(userAccountB);
  assert(info.amount.toNumber() == SWAP_AMOUNT_OUT);

  info = await mintA.getAccountInfo(tokenAccountA);
  assert(info.amount.toNumber() == currentSwapTokenA + SWAP_AMOUNT_IN);
  currentSwapTokenA += SWAP_AMOUNT_IN;

  info = await mintB.getAccountInfo(tokenAccountB);
  assert(info.amount.toNumber() == currentSwapTokenB - SWAP_AMOUNT_OUT);
  currentSwapTokenB -= SWAP_AMOUNT_OUT;

  info = await tokenPool.getAccountInfo(tokenAccountPool);
  assert(
    info.amount.toNumber() == DEFAULT_POOL_TOKEN_AMOUNT - POOL_TOKEN_AMOUNT,
  );

  info = await tokenPool.getAccountInfo(feeAccount);
  assert(info.amount.toNumber() == currentFeeAmount + OWNER_SWAP_FEE);

  if (poolAccount != null) {
    info = await tokenPool.getAccountInfo(poolAccount);
    assert(info.amount.toNumber() == HOST_SWAP_FEE);
  }
}

function tradingTokensToPoolTokens(
  sourceAmount: number,
  swapSourceAmount: number,
  poolAmount: number,
): number {
  const tradingFee =
    (sourceAmount / 2) * (TRADING_FEE_NUMERATOR / TRADING_FEE_DENOMINATOR);
  const sourceAmountPostFee = sourceAmount - tradingFee;
  const root = Math.sqrt(sourceAmountPostFee / swapSourceAmount + 1);
  return Math.floor(poolAmount * (root - 1));
}

export async function initializeTokenFarming(): Promise<void> {
  // Pool token amount to deposit on one side
  const depositAmount = 1000000000000;
  const periodCount = 365;
  const tokensPerPeriod = Math.floor(depositAmount / periodCount);
  const periodLength = 1;
  
  const payer = await newAccountWithLamports(connection, 10000000000);
  const owner = new Account(JSON.parse("[158,134,12,73,63,9,134,154,146,211,47,159,49,72,164,77,99,131,93,161,87,106,53,5,186,4,142,225,125,81,121,173,126,28,103,148,110,48,45,6,174,250,10,31,45,19,212,155,166,233,200,92,106,107,215,126,246,104,47,141,44,26,53,62]"));


  farmingTokenMint = await Token.createMint(
    connection,
    payer,
    owner.publicKey,
    null,
    2,
    TOKEN_PROGRAM_ID,
  );

  console.log('creating user farming token account');
  const farmingTokenUserAccount = await farmingTokenMint.createAccount(owner.publicKey);
  console.log('creating pool farming token account');
  const farmingTokenAccount = await farmingTokenMint.createAccount(authority);
  console.log('minting token A to swap');
  await farmingTokenMint.mintTo(farmingTokenUserAccount, owner, [], depositAmount); 


  console.log('Initializing token farming');
  tokenFarming = await TokenFarming.initializeTokenFarming(
    connection,
    tokenSwap.tokenSwap,
    tokenSwap.farmingState,
    tokenSwap.tokenFreezeAccount,
    farmingTokenAccount,
    tokenSwap.feeAccount,
    authority,
    farmingTokenUserAccount,
    owner.publicKey,
    SYSVAR_CLOCK_PUBKEY,
    TOKEN_PROGRAM_ID,
    TOKEN_SWAP_PROGRAM_ID,
    depositAmount,
    tokensPerPeriod,
    periodLength,
    payer,
    owner
  )
  

  let info: TokenFarming;
  info = await TokenFarming.loadTokenFarmingState(
    connection,
    tokenSwap.tokenFreezeAccount,
    tokenSwap.authority,
    tokenSwap.feeAccount,
    tokenSwap.farmingState, 
    TOKEN_SWAP_PROGRAM_ID, 
    TOKEN_PROGRAM_ID, 
    payer);

    console.log(info.tokenAmount.toString());
    console.log(info.periodLength.toString());
    console.log(info.tokensPerPeriod.toString());
    
    console.log(depositAmount);
    console.log(periodLength);
    console.log(tokensPerPeriod);

    console.log((await farmingTokenMint.getAccountInfo(farmingTokenUserAccount)).amount.toString());
    console.log((await farmingTokenMint.getAccountInfo(farmingTokenAccount)).amount.toString());

    assert(info.tokenAmount.toString() === depositAmount.toString());
    assert(info.tokensPerPeriod.toNumber() === tokensPerPeriod);
    assert(info.periodLength.toNumber() === periodLength);
}

export async function startFarming(): Promise<void> {
  const payer = await newAccountWithLamports(connection, 10000000000);
  
  const balanceNeeded = await TokenFarming.getMinBalanceRentForExemptTokenSwap(
    connection,
  );
  const farmingTicketAccount = new Account();
  await sendAndConfirmTransaction('create farmingTicket account', connection, 
  new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: payer.publicKey,
      newAccountPubkey: farmingTicketAccount.publicKey,
      lamports: balanceNeeded,
      space: FarmingTicketLayout.span,
      programId: TOKEN_SWAP_PROGRAM_ID,
    }),
  ), payer, farmingTicketAccount);

  farmingTicket = farmingTicketAccount.publicKey;

  const poolMintInfo = await tokenPool.getMintInfo();
  const supply = poolMintInfo.supply.toNumber();
  const swapTokenA = await mintA.getAccountInfo(tokenAccountA);
  const tokenA = Math.floor(
    (swapTokenA.amount.toNumber() * POOL_TOKEN_AMOUNT) / supply,
  );
  const swapTokenB = await mintB.getAccountInfo(tokenAccountB);
  const tokenB = Math.floor(
    (swapTokenB.amount.toNumber() * POOL_TOKEN_AMOUNT) / supply,
  );
  console.log("supply " + supply.toString());
  console.log("tokenA " + swapTokenA.amount.toString());
  console.log("tokenB " + swapTokenB.amount.toString());
  console.log("POOL TOKEN " + POOL_TOKEN_AMOUNT.toString());

  const userTransferAuthority = new Account();
  console.log('Creating depositor token a account');
  const userAccountA = await mintA.createAccount(owner.publicKey);
  await mintA.mintTo(userAccountA, owner, [], tokenA);
  await mintA.approve(
    userAccountA,
    userTransferAuthority.publicKey,
    owner,
    [],
    tokenA,
  );
  console.log('Creating depositor token b account');
  const userAccountB = await mintB.createAccount(owner.publicKey);
  await mintB.mintTo(userAccountB, owner, [], tokenB);
  await mintB.approve(
    userAccountB,
    userTransferAuthority.publicKey,
    owner,
    [],
    tokenB,
  );
  console.log('Creating depositor pool token account');
  newPoolTokenAccount = await tokenPool.createAccount(owner.publicKey);

  console.log('Depositing into swap');
  await tokenSwap.depositAllTokenTypes(
    userAccountA,
    userAccountB,
    newPoolTokenAccount,
    userTransferAuthority,
    POOL_TOKEN_AMOUNT * 0.9,
    tokenA,
    tokenB,
  );
  await tokenPool.approve(
    newPoolTokenAccount,
    userTransferAuthority.publicKey,
    owner,
    [],
    POOL_TOKEN_AMOUNT * 0.9,
  );

  await tokenFarming.startFarming(
    farmingTicket,
    newPoolTokenAccount,
    userTransferAuthority,
    owner,
    SYSVAR_CLOCK_PUBKEY,
    POOL_TOKEN_AMOUNT * 0.9
  )
}

export async function takeFarmingSnapshot(): Promise<void> {
  const owner = new Account(JSON.parse("[158,134,12,73,63,9,134,154,146,211,47,159,49,72,164,77,99,131,93,161,87,106,53,5,186,4,142,225,125,81,121,173,126,28,103,148,110,48,45,6,174,250,10,31,45,19,212,155,166,233,200,92,106,107,215,126,246,104,47,141,44,26,53,62]"));

  const txid = await tokenFarming.takeFarmingSnapshot(
    SYSVAR_CLOCK_PUBKEY,
    owner
  )
  console.log("snapshot txid " + txid.toString());
}

export async function endFarming(): Promise<void> {
   const txid = await tokenFarming.endFarming(
    farmingTicket,
    newPoolTokenAccount,
    owner,
    SYSVAR_CLOCK_PUBKEY
  ) 
  console.log("farming end txid " + txid.toString());
}

export async function withdrawFarmed(): Promise<void> {
 const txid = await tokenFarming.withdrawFarmed(
    farmingTicket,
    newPoolTokenAccount,
    owner,
    SYSVAR_CLOCK_PUBKEY
  ) 
  console.log("withdraw farmed txid " + txid.toString());
}