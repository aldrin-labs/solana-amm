import { PublicKey } from "@solana/web3.js";
import { expect } from "chai";

import {
  payer,
  provider,
  errLogs,
  sleep,
  assertApproxCurrentSlot,
} from "../helpers";
import { Farm } from "../farm";

export function test() {
  describe("take_snapshot", () => {
    const admin = payer;
    let farm: Farm;
    let depositorStakeWallet: PublicKey;

    beforeEach("create farm", async () => {
      farm = await Farm.init();
    });

    beforeEach("Mint and Deposit stake tokens to stakeVault", async () => {
      depositorStakeWallet = await farm.createStakeWallet(10_000);
    });

    it("fails if time elapsed between slots < minSnapshotWindow", async () => {
      const minSnapshotWindow = 1;
      await farm.setMinSnapshotWindow(minSnapshotWindow);

      sleep(2_000);
      // First snapshot works
      await farm.takeSnapshot();

      // Second snapshot fails because not enough time has passed
      const logs = await errLogs(farm.takeSnapshot());

      expect(logs).to.contain("InsufficientSlotTime");
    });

    it("fails if wrong stake vault is provided", async () => {
      const minSnapshotWindow = 1;
      await farm.setMinSnapshotWindow(minSnapshotWindow, {
        admin,
      });

      const fakeVault = await farm.createStakeWallet(0);

      sleep(2_000);
      const logs = await errLogs(
        farm.takeSnapshot({
          stakeVault: fakeVault,
        })
      );

      expect(logs).to.contain(
        "stake vault does not correspond to the Farm stake vault"
      );
    });

    it("takes multiple snapshots", async () => {
      const minSnapshotWindow = 1;
      await farm.setMinSnapshotWindow(minSnapshotWindow);

      let stakedAMount = 0;
      let cumulatedStaking = 0;

      let tip = 0;
      for (let i = 0; i < 3; i++) {
        // Transfer tokens 100 tokens to the StakeVault
        stakedAMount = tip * 10;
        await farm.transferToStakeVault(depositorStakeWallet, stakedAMount);
        cumulatedStaking += stakedAMount;

        await farm.takeSnapshot();

        tip++;

        const farmInfoAfter = await farm.fetch();

        const snapshots = farmInfoAfter.snapshots;
        const ringBuffer = snapshots.ringBuffer as any[];

        expect(snapshots.ringBufferTip.toNumber()).to.eq(tip);
        await assertApproxCurrentSlot(ringBuffer[tip].startedAt);
        expect(ringBuffer[tip].staked.amount.toNumber()).to.eq(
          cumulatedStaking
        );

        sleep(2_000);
      }
    });

    it("is initialised to defaulted values", async () => {
      const { snapshots } = await farm.fetch();

      expect(snapshots.ringBufferTip.toNumber()).to.eq(0);
      const buffer = snapshots.ringBuffer as any[];
      expect(buffer.length).to.eq(1000);

      buffer.slice().forEach((entry) => {
        expect(entry.staked.amount.toNumber()).to.eq(0);
        expect(entry.startedAt.slot.toNumber()).to.eq(0);
      });
    });
  });
}
