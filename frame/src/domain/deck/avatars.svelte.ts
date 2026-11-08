import { commands } from "$shared/ipc/bindings";
import { events } from "$shared/ipc/native-events";
let values = $state<Record<string, string>>({});
let stop: (() => void) | null = null;
let generation = 0;

async function init() {
  const current = ++generation;
  const observed = new Set<string>();
  values = {};
  stop = await events.uiCommand.listen(({ payload }) => {
    if (!payload.startsWith("account.avatar=")) return;
    const body = payload.slice("account.avatar=".length);
    const separator = body.indexOf(":");
    if (separator < 0) return;
    const profile = body.slice(0, separator);
    try {
      const url: unknown = JSON.parse(body.slice(separator + 1));
      if (typeof url === "string" || url === null) {
        observed.add(profile);
        const next = { ...values };
        if (url === null) delete next[profile]; else next[profile] = url;
        values = next;
      }
    } catch {}
  });
  const encoded = await commands.settingGet("deck.avatars");
  const decoded: unknown = encoded ? JSON.parse(encoded) : {};
  const stored: Record<string, string> = {};
  if (typeof decoded === "object" && decoded !== null && !Array.isArray(decoded)) {
    for (const [profile, value] of Object.entries(decoded)) if (typeof value === "string") stored[profile] = value;
  }
  if (generation !== current) return;
  const next = { ...stored, ...values };
  for (const profile of observed) if (!(profile in values)) delete next[profile];
  values = next;
}
function dispose() { generation += 1; stop?.(); stop = null; values = {}; }
export const accountAvatars = { init, dispose, url: (profile: string): string | null => values[profile] ?? null };
