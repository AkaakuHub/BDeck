import type { AccountLoginState } from "$shared/ipc/bindings";
import { commands } from "$shared/ipc/bindings";
import { events } from "$shared/ipc/native-events";

let sessions = $state<Record<string, AccountLoginState>>({});
let stop: (() => void) | null = null;
let deleting = $state(false);
let connected = $state<string | null>(null);

async function init() {
  if (stop) return;
  stop = await events.uiCommand.listen(({ payload }) => {
    if (payload === "account.deleting=true" || payload === "account.deleting=false") { deleting = payload === "account.deleting=true"; return; }
    if (payload.startsWith("account.removed=")) {
      const id = payload.slice("account.removed=".length);
      delete sessions[id];
      if (connected === id) connected = null;
      return;
    }
    if (payload.startsWith("account.connected=")) {
      const id = payload.slice("account.connected=".length);
      if (/^[0-9A-HJKMNP-TV-Z]{26}$/u.test(id)) connected = id;
      return;
    }
    if (!payload.startsWith("account.status=")) return;
    const [id, state] = payload.slice("account.status=".length).split(":");
    if (!id || !/^[0-9A-HJKMNP-TV-Z]{26}$/u.test(id)) return;
    if (state === "logged_in" || state === "logged_out" || state === "unknown" || state === "account_changed") sessions[id] = state;
  });
  await commands.runCommand("account.status");
}

function dispose() { stop?.(); stop = null; sessions = {}; connected = null; deleting = false; }

export const accountSessions = {
  init, dispose, deleting: () => deleting, status: (id: string): AccountLoginState => sessions[id] ?? "unknown",
  connected: () => connected, acknowledge: () => { connected = null; },
};
