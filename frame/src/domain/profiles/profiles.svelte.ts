import type { ProfileEntryView, ProfilesView } from "$shared/ipc/bindings";
import { commands } from "$shared/ipc/bindings";
import { events } from "$shared/ipc/native-events";

let view = $state.raw<ProfilesView | null>(null);
let users = 0;
let stop: (() => void) | null = null;

export const all = (): ProfileEntryView[] => view?.profiles ?? [];

export async function retain() {
  users += 1;
  if (users > 1) return;
  stop = await events.profilesChanged.listen(({ payload }) => {
    view = payload;
  });
  try {
    const reply = await commands.profilesGet();
    if (reply) view = reply;
  } catch {
    view = null;
  }
}

export function release() {
  users = Math.max(0, users - 1);
  if (users > 0) return;
  stop?.();
  stop = null;
}

export const create = () => commands.profilesCreate();
