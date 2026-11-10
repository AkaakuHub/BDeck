import { flushSync } from "svelte";
import { commands } from "$shared/ipc/bindings";
import { events } from "$shared/ipc/native-events";

export type SettingsSection = "appearance" | "accounts" | "extensions";
let visible = $state(false);
let section = $state<SettingsSection>("appearance");
let failed = $state(false);
let stops: (() => void)[] = [];
let returning: Promise<boolean> | null = null;
let returned: ((success: boolean) => void) | null = null;
function finishReturn(success: boolean) { const finish = returned; returned = null; returning = null; finish?.(success); }

async function init() {
  if (stops.length) return;
  stops = await Promise.all([
    events.uiCommand.listen(({ payload }) => {
      if (payload === "browser.settings" || payload === "browser.return") {
        flushSync(() => { visible = payload === "browser.settings"; });
        finishReturn(payload === "browser.return");
      }
      if (payload === "browser.return-failed") { failed = true; finishReturn(false); }
    }),
    events.browserReturn.listen(() => flushSync(() => { visible = false; })),
  ]);
}

function dispose() {
  finishReturn(false);
  for (const stop of stops) stop();
  stops = [];
  visible = false;
  failed = false;
}

async function open(next: SettingsSection) {
  finishReturn(false);
  section = next;
  if (visible) return;
  failed = false;
  try {
    failed = !(await commands.runCommand("browser.settings")).accepted;
  } catch {
    failed = true;
  }
}

async function close(): Promise<boolean> {
  if (returning) return returning;
  if (!visible) return true;
  failed = false;
  const result = new Promise<boolean>((resolve) => { returned = resolve; });
  returning = result;
  try {
    if (!(await commands.runCommand("browser.return")).accepted) { failed = true; finishReturn(false); }
  } catch { failed = true; finishReturn(false); }
  return result;
}

function toggle(next: SettingsSection) { return visible && section === next ? close() : open(next); }

export const surface = { init, dispose, open, close, toggle, visible: () => visible, section: () => section, failed: () => failed };
