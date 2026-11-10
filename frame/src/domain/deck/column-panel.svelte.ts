import { commands, type PanelAction, type PanelView, type ColumnDraft } from "$shared/ipc/bindings";
import { accepted, runOperation } from "$shared/ipc/operation";
import { surface } from "$domain/surface";
import { events } from "$shared/ipc/native-events";
import { deck } from "./deck.svelte";
import { accountAvatars } from "./avatars.svelte";
import { DEFAULT_COLUMN_WIDTH, targetName, type ColumnTarget } from "./deck-types";

const ACCOUNT_SETTING = "deck.column-account";
let lastAccount = $state<string | null>(null);
let selection = $state<string | null>(null);
let queue: Promise<void> = Promise.resolve();
let presentationRequest: symbol | null = null;

function snapshot(): PanelView | null {
  const active = deck.active();
  if (!active || !selection) return null;
  const accounts = deck.readyAccounts().map((account) => ({ id: account.id, title: deck.displayName(account.id), screen_name: account.identity?.screen_name ?? null, avatar: accountAvatars.url(account.profile) }));
  const column = active.columns.find((column) => column.id === selection);
  if ((!column && selection !== "add") || accounts.length === 0) return null;
  return {
    composer: false, content_height: null, id: column?.id ?? null, title: column ? targetName(column.target) : "カラムを追加", index: column ? active.columns.indexOf(column) : 0, count: active.columns.length,
    account: column?.account ?? accounts.find((account) => account.id === lastAccount)?.id ?? accounts[0]?.id ?? "", accounts,
    width: column?.width ?? DEFAULT_COLUMN_WIDTH, row: column?.row ?? 0, two_rows: active.layout === "two_rows", suspended: column?.suspended ?? false, failed: deck.failed(),
  };
}

function target(draft: ColumnDraft): ColumnTarget {
  switch (draft.kind) {
    case "home": return { kind: "home" };
    case "notifications": return { kind: "notifications" };
    case "profile": return { kind: "profile" };
    case "messages": return { kind: "url", url: "https://x.com/messages" };
    case "explore": return { kind: "url", url: "https://x.com/explore" };
    case "search": return { kind: "search", query: draft.text.trim() };
    case "list": return { kind: "list", url: draft.text.trim() };
  }
}

async function apply(id: string | null, action: PanelAction) {
  if (action.kind === "account" && id === null && selection === "add") {
    try {
      const result = await runOperation(() => commands.settingSet(ACCOUNT_SETTING, action.value));
      if (!accepted(result)) throw new Error("column account selection was not saved");
      lastAccount = action.value;
    } finally { await commands.columnPanelSync(snapshot()); }
    return;
  }
  if (action.kind === "create") {
    if (id !== null || selection !== "add") return;
    const draft = action.value;
    const created = await deck.addColumn(target(draft), draft.account, draft.width, draft.row);
    if (selection !== "add") return;
    if (created) {
      selection = null;
      await commands.columnPanelSync(null);
      await commands.tabsActivate(created);
    } else { await commands.columnPanelSync(snapshot()); }
    return;
  }
  const column = deck.active()?.columns.find((column) => column.id === id);
  if (!id || !column || id !== selection) return;
  try {
    switch (action.kind) {
      case "width": await deck.updateColumn(id, (current) => ({ ...current, width: action.value })); break;
      case "account": await deck.updateColumn(id, (current) => ({ ...current, account: action.value })); break;
      case "row": if (action.value === 0 || action.value === 1) await deck.setColumnRow(id, action.value); break;
      case "previous": await deck.moveColumn(id, -1); break;
      case "next": await deck.moveColumn(id, 1); break;
      case "duplicate": await deck.duplicateColumn(column); break;
      case "suspend":
        if (await deck.toggleSuspended(id) && selection === id) {
          selection = null;
          await commands.columnPanelSync(null);
        }
        break;
      case "remove": await deck.removeColumn(id); break;
    }
  } finally { if (selection === id) await commands.columnPanelSync(snapshot()); }
}

async function show(next: string, event: MouseEvent) {
  if (next === "add" && deck.readyAccounts().length === 0) return;
  if (!(event.currentTarget instanceof HTMLElement)) return;
  const button = event.currentTarget;
  const request = Symbol();
  presentationRequest = request;
  if (!await surface.close() || !button.isConnected || request !== presentationRequest) return;
  const closed = await runOperation(() => commands.runCommand("composer.close"));
  if (!accepted(closed) || !button.isConnected || request !== presentationRequest) return;
  const anchor = button.getBoundingClientRect();
  selection = next;
  const view = snapshot();
  if (view) await commands.columnPanelOpen(view, anchor.right, anchor.bottom);
}

async function init(): Promise<() => void> {
  lastAccount = await commands.settingGet(ACCOUNT_SETTING);
  if (selection === null) await commands.columnPanelSync(null);
  const stop = await events.columnPanelAction.listen(({ payload: [id, action] }) => {
    queue = queue.then(() => apply(id, action)).catch(() => console.error("column panel update failed"));
  });
  const stopSync = $effect.root(() => {
    $effect(() => {
      const view = snapshot();
      if (selection) void commands.columnPanelSync(view).catch(() => console.error("column panel synchronization failed"));
    });
  });
  return () => { stop(); stopSync(); selection = null; };
}

export const columnPanel = { open: (id: string, event: MouseEvent) => show(id, event), add: (event: MouseEvent) => show("add", event), init };
