import {
  AnchorProvider,
  Program,
  setProvider,
  workspace,
} from "@project-serum/anchor";
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
