import { sleep } from '../src/util/sleep';
import {
  createAccountAndSwapAtomic,
  createTokenSwap,
  swap,
  depositAllTokenTypes,
  withdrawAllTokenTypes,
  initializeTokenFarming,  
  startFarming,
  takeFarmingSnapshot,
  endFarming,
  withdrawFarmed,
} from './token-swap-test';

async function main() {
  // These test cases are designed to run sequentially and in the following order
  console.log('Run test: createTokenSwap');
  await createTokenSwap();
  console.log('Run test: deposit all token types');
  await depositAllTokenTypes();
  console.log('Run test: withdraw all token types');
  await withdrawAllTokenTypes();
  console.log('Run test: swap');
  await swap();
  console.log('Run test: create account, approve, swap all at once');
  await createAccountAndSwapAtomic();
  console.log('Run test: initialize farming');
  await initializeTokenFarming();
  console.log('Run test: start farming');
  await startFarming();
  await sleep(10000);
  console.log('Run test: take farming snapshot');
  await takeFarmingSnapshot();
  console.log('Run test: end farming');
  await endFarming();
  console.log('Run test: withdraw farmed');
  await withdrawFarmed();
  console.log('Success\n');
}

main()
  .catch(err => {
    console.error(err);
    process.exit(-1);
  })
  .then(() => process.exit());