import type { AccountLoginState } from "$shared/ipc/bindings";
import { commands } from "$shared/ipc/bindings";
import { events } from "$shared/ipc/native-events";

let sessions = $state<Record<string, AccountLoginState>>({});
let stop: (() => void) | null = null;
let deleting = $state(false);
let generation = 0;
let pending: Promise<void> | null = null;

function init(): Promise<void> {
  if (stop) return Promise.resolve();
  if (pending) return pending;
  const current = ++generation;
  const loading = (async () => {
  const unlisten = await events.uiCommand.listen(({ payload }) => {
    if (current !== generation) return;
    if (payload === "account.deleting=true" || payload === "account.deleting=false") { deleting = payload === "account.deleting=true"; return; }
    if (payload.startsWith("account.removed=")) {
      const id = payload.slice("account.removed=".length);
      delete sessions[id];
      return;
    }
    if (!payload.startsWith("account.status=")) return;
    const [id, state] = payload.slice("account.status=".length).split(":");
    if (!id || !/^[0-9A-HJKMNP-TV-Z]{26}$/u.test(id)) return;
    if (state === "logged_in" || state === "logged_out" || state === "unknown" || state === "account_changed") sessions[id] = state;
  });
  if (current !== generation) { unlisten(); return; }
  stop = unlisten;
  await commands.runCommand("account.status");
  })();
  pending = loading;
  void loading.finally(() => { if (pending === loading) pending = null; }).catch(() => console.error("account session initialization failed"));
  return loading;
}

function dispose() { generation += 1; pending = null; stop?.(); stop = null; sessions = {}; deleting = false; }

export const accountSessions = {
  init, dispose, deleting: () => deleting, status: (id: string): AccountLoginState => sessions[id] ?? "unknown",
};
