import { amm, errLogs, payer, provider } from "../helpers";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { createMint, getAccount } from "@solana/spl-token";
import { expect } from "chai";

export function test() {
  describe("create_farm", () => {
    it("fails if farm account isn't initialized with system program", async () => {
      const logs = await errLogs(Farm.init({ skipCreateFarm: true }));
      expect(logs).to.contain("range end index 8");
    });

    it("fails if farm account already exists", async () => {
      const farm = await Farm.init();

      const logs = await errLogs(Farm.init({ keypair: farm.keypair }));
      expect(logs).to.contain("already in use");
    });

    it("fails if provided with incorrect PDA signer bump seed", async () => {
      const farmKeypair = Keypair.generate();
      const [_, correctBumpSeed] = await Farm.signerFrom(farmKeypair.publicKey);

      const logs = await errLogs(
        Farm.init({
          keypair: farmKeypair,
          bumpSeed: correctBumpSeed === 0 ? 1 : 0,
        })
      );
      expect(logs).to.contain("seeds constraint was violated");
    });

    it("fails if provided with incorrect PDA signer address", async () => {
      const logs = await errLogs(
        Farm.init({
          pda: Keypair.generate().publicKey,
        })
      );
      expect(logs).to.contain("seeds constraint was violated");
    });

    it("fails if admin isn't signer", async () => {
      await expect(Farm.init({ skipAdminSignature: true })).to.be.rejected;
    });

    it("fails if stake vault PDA is invalid", async () => {
      const logs = await errLogs(
        Farm.init({ stakeVault: Keypair.generate().publicKey })
      );
      expect(logs).to.contain("unauthorized signer");
    });

    it("fails if vesting vault PDA is invalid", async () => {
      const logs = await errLogs(
        Farm.init({ vestingVault: Keypair.generate().publicKey })
      );
      expect(logs).to.contain("unauthorized signer");
    });

    it("works", async () => {
      const farm = await Farm.init();
      const farmInfo = await farm.fetch();

      expect(farmInfo.admin).to.deep.eq(farm.admin.publicKey);
      expect(farmInfo.stakeMint).to.deep.eq(farm.stakeMint);
      expect(farmInfo.stakeVault).to.deep.eq(await farm.stakeVault());
      expect(farmInfo.vestingVault).to.deep.eq(await farm.vestingVault());

      const stakeVault = await getAccount(
        provider.connection,
        farmInfo.stakeVault
      );
      const vestingVault = await getAccount(
        provider.connection,
        farmInfo.stakeVault
      );
      expect(stakeVault.mint).to.deep.eq(vestingVault.mint);
      expect(stakeVault.mint).to.deep.eq(farm.stakeMint);
      expect(stakeVault.owner).to.deep.eq(vestingVault.owner);
      expect(stakeVault.owner).to.deep.eq(await farm.signer());
      expect(stakeVault.closeAuthority).to.be.null;
      expect(vestingVault.closeAuthority).to.be.null;
      expect(stakeVault.isInitialized).to.be.true;
      expect(vestingVault.isInitialized).to.be.true;

      expect(farmInfo.harvests).to.be.lengthOf(10);
      (farmInfo.harvests as any[]).forEach((h) => {
        expect(h.harvestMint).to.deep.eq(PublicKey.default);
        expect(h.harvestVault).to.deep.eq(PublicKey.default);

        expect(h.tokensPerSlot).to.be.lengthOf(10);
        h.tokensPerSlot.forEach(({ value, at }) => {
          expect(value.amount.toNumber()).to.eq(0);
          expect(at.slot.toNumber()).to.eq(0);
        });
      });

      expect(farmInfo.snapshots.ringBufferTip.toNumber()).to.eq(0);
      expect(farmInfo.snapshots.ringBuffer).to.be.lengthOf(1_000);
      (farmInfo.snapshots.ringBuffer as any[]).forEach(
        ({ staked, startedAt }) => {
          expect(staked.amount.toNumber()).to.eq(0);
          expect(startedAt.slot.toNumber()).to.eq(0);
        }
      );
    });
  });
}

export class Farm {
  public get id(): PublicKey {
    return this.keypair.publicKey;
  }

  private constructor(
    public keypair: Keypair,
    public admin: Keypair,
    public stakeMint: PublicKey
  ) {
    //
  }

  public static async init(input: Partial<InitFarmArgs> = {}): Promise<Farm> {
    const adminKeypair = input.adminKeypair ?? payer;
    const farmKeypair = input.keypair ?? Keypair.generate();
    const skipAdminSignature = input.skipAdminSignature ?? false;
    const skipCreateFarm = input.skipCreateFarm ?? false;
    const skipKeypairSignature = input.skipAdminSignature ?? skipCreateFarm;
    const [correctPda, correctBumpSeed] = await PublicKey.findProgramAddress(
      [Buffer.from("signer"), farmKeypair.publicKey.toBytes()],
      amm.programId
    );
    const pda = input.pda ?? correctPda;
    const bumpSeed = input.bumpSeed ?? correctBumpSeed;

    const stakeMint =
      input.stakeMint ??
      (await (async () => {
        return createMint(
          provider.connection,
          payer,
          adminKeypair.publicKey,
          null,
          6
        );
      })());

    const stakeVault =
      input.stakeVault ??
      (await (async () => {
        const [pda, _] = await PublicKey.findProgramAddress(
          [Buffer.from("stake_vault"), farmKeypair.publicKey.toBytes()],
          amm.programId
        );
        return pda;
      })());

    const vestingVault =
      input.vestingVault ??
      (await (async () => {
        const [pda, _] = await PublicKey.findProgramAddress(
          [Buffer.from("vesting_vault"), farmKeypair.publicKey.toBytes()],
          amm.programId
        );
        return pda;
      })());

    const signers = [];
    if (!skipAdminSignature) {
      signers.push(adminKeypair);
    }
    if (!skipKeypairSignature) {
      signers.push(farmKeypair);
    }

    const preInstructions = [];
    if (!skipCreateFarm) {
      preInstructions.push(
        await amm.account.farm.createInstruction(farmKeypair)
      );
    }

    await amm.methods
      .createFarm(bumpSeed)
      .accounts({
        admin: adminKeypair.publicKey,
        farm: farmKeypair.publicKey,
        farmSignerPda: pda,
        stakeMint,
        stakeVault,
        vestingVault,
      })
      .signers(signers)
      .preInstructions(preInstructions)
      .rpc();

    return new Farm(farmKeypair, adminKeypair, stakeMint);
  }

  public async fetch() {
    return amm.account.farm.fetch(this.id);
  }

  public async stakeVault(): Promise<PublicKey> {
    const [pda, _] = await PublicKey.findProgramAddress(
      [Buffer.from("stake_vault"), this.id.toBytes()],
      amm.programId
    );
    return pda;
  }

  public async vestingVault(): Promise<PublicKey> {
    const [pda, _] = await PublicKey.findProgramAddress(
      [Buffer.from("vesting_vault"), this.id.toBytes()],
      amm.programId
    );
    return pda;
  }

  public static async signerFrom(
    publicKey: PublicKey
  ): Promise<[PublicKey, number]> {
    return PublicKey.findProgramAddress(
      [Buffer.from("signer"), publicKey.toBytes()],
      amm.programId
    );
  }

  public async signer(): Promise<PublicKey> {
    const [pda] = await Farm.signerFrom(this.id);
    return pda;
  }
}

export interface InitFarmArgs {
  adminKeypair: Keypair;
  bumpSeed: number;
  keypair: Keypair;
  pda: PublicKey;
  skipAdminSignature: boolean;
  skipCreateFarm: boolean;
  skipKeypairSignature: boolean;
  stakeVault: PublicKey;
  vestingVault: PublicKey;
  stakeMint: PublicKey;
}
