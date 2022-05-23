import {
  AnchorProvider,
  Program,
  setProvider,
  workspace,
} from "@project-serum/anchor";
import { PublicKey } from "@solana/web3.js";
import NodeWallet from "@project-serum/anchor/dist/cjs/nodewallet";
import { Amm } from "../../target/types/amm";

export const provider = AnchorProvider.local();
setProvider(provider);
export const payer = (provider.wallet as NodeWallet).payer;

export const amm = workspace.Amm as Program<Amm>;

export async function errLogs(job: Promise<unknown>): Promise<string> {
  try {
    await job;
  } catch (error) {
    if (!Array.isArray(error.logs)) {
      console.log("No logs on the error:", error);
      throw new Error(`No logs on the error objection`);
    }

    return String(error.logs);
  }

  throw new Error("Expected promise to fail");
}

export async function airdrop(to: PublicKey, amount: number = 100_000_000_000) {
  await provider.connection.confirmTransaction(
    await provider.connection.requestAirdrop(to, amount),
    "confirmed"
  );
}
