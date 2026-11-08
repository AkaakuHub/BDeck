import { commands } from "$shared/ipc/bindings";
import { runOperation } from "$shared/ipc/operation";
import { accountSessions } from "./account-sessions.svelte";

let pending = $state(false);
let failed = $state(false);
let generation = 0;

async function remove(account: string) {
  if (pending || accountSessions.deleting()) return;
  const current = generation;
  pending = true;
  failed = false;
  try {
    const result = await runOperation(() => commands.accountDelete(account));
    if (current === generation) failed = result.kind !== "cancelled" && (result.kind !== "processed" || result.disposition.outcome !== "applied" || result.disposition.reason !== "profile_deletion_completed");
  } catch { if (current === generation) failed = true; }
  finally { if (current === generation) pending = false; }
}
function dispose() { generation += 1; pending = false; failed = false; }
export const accountDeletion = { dispose, remove, pending: () => pending || accountSessions.deleting(), failed: () => failed };
