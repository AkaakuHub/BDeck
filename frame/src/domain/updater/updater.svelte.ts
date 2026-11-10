import { commands, type UpdateStatus } from "$shared/ipc/bindings";
import { events } from "$shared/ipc/native-events";

let status = $state<UpdateStatus | null>(null);
let lifetime: symbol | null = null;
let stop: (() => void) | null = null;

function receive(next: UpdateStatus) {
  if (status === null || next.revision >= status.revision) status = next;
}

async function init() {
  if (lifetime !== null) return;
  const current = Symbol();
  lifetime = current;
  try {
    const listening = await events.updater.listen(({ payload }) => {
      if (lifetime === current) receive(payload);
    });
    if (lifetime !== current) { listening(); return; }
    stop = listening;
    const snapshot = await commands.updaterStatus();
    if (lifetime === current) receive(snapshot);
  } catch {
    if (lifetime === current) console.error("update status initialization failed");
  }
}

function dispose() {
  lifetime = null;
  stop?.();
  stop = null;
  status = null;
}

export const updater = {
  init, dispose,
  status: () => status,
  available: () => status !== null && status.version !== null
    && ["checking", "available", "downloading", "verifying", "installing"].includes(status.phase),
};
