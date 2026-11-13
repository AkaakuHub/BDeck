import type { DeckAppearance } from "$shared/ipc/bindings";
import { DEFAULT_APPEARANCE, isDeckAppearance } from "./deck-appearance";
import { accountAvatars } from "./avatars.svelte";
import { accountDeletion } from "./account-deletion.svelte";
import { accountSessions } from "./account-sessions.svelte";
import { accepted, runOperation } from "$shared/ipc/operation";
import { events } from "$shared/ipc/native-events";
import { surface } from "$domain/surface";
import { profiles } from "$domain/profiles";
import { commands } from "$shared/ipc/bindings";
import { normalizeColumnRows, repositionColumn } from "./columns";
import {
  DECK_SETTING, DEFAULT_ROW_RATIO, DECK_APPEARANCE_SETTING,
  type ColumnTarget, type Deck, type DeckColumn, type DeckState,
  isDeckState, newUlid,
} from "./deck-types";

let deckState = $state<DeckState | null>(null);
let stateRevision = 0;
let appearance = $state<DeckAppearance>(DEFAULT_APPEARANCE);
let saveError = $state(false);
let hideAdvertisements = $state(false);
const HIDE_ADVERTISEMENTS_SETTING = "deck.hide-advertisements";

let activeDeck = $derived(deckState?.decks.find((deck) => deck.id === deckState?.active) ?? null);
let readyAccounts = $derived((deckState?.accounts ?? []).filter((account) => account.identity !== null && accountSessions.status(account.id) === "logged_in" && !accountSessions.deleting()));
function parseStored(value: string | null): DeckState | null {
  if (!value) return null;
  try {
    const parsed: unknown = JSON.parse(value);
    return isDeckState(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function parseAppearance(value: string | null): DeckAppearance {
  if (!value) return DEFAULT_APPEARANCE;
  try {
    const parsed: unknown = JSON.parse(value);
    if (!isDeckAppearance(parsed)) return DEFAULT_APPEARANCE;
    return {
      font_size: parsed.font_size,
      density: parsed.density,
      avatars: parsed.avatars,
      media: parsed.media,
      scrollbars: parsed.scrollbars,
      hide_navigation: parsed.hide_navigation,
    };
  } catch { return DEFAULT_APPEARANCE; }
}

function defaultState(): DeckState | null {
  const deckId = newUlid();
  return {
    accounts: [],
    decks: [
      {
        id: deckId,
        name: "メイン",
        layout: "horizontal",
        row_ratio: DEFAULT_ROW_RATIO,
        columns: [],
      },
    ],
    active: deckId,
  };
}

async function save(next: DeckState): Promise<boolean> {
  next = { ...next, decks: next.decks.map(normalizeColumnRows) };
  try {
    const result = await runOperation(() => commands.settingSet(DECK_SETTING, JSON.stringify(next)));
    if (!accepted(result)) {
      saveError = true;
      return false;
    }
    saveError = false;
    return true;
  } catch {
    saveError = true;
    return false;
  }
}

async function load() {
  const revision = stateRevision;
  try {
    const [storedDeck, storedAppearance, storedAdvertisements] = await Promise.all([
      commands.settingGet(DECK_SETTING),
      commands.settingGet(DECK_APPEARANCE_SETTING),
      commands.settingGet(HIDE_ADVERTISEMENTS_SETTING),
    ]);
    const stored = parseStored(storedDeck);
    if (storedDeck !== null && !stored) { saveError = true; return; }
    appearance = parseAppearance(storedAppearance);
    hideAdvertisements = storedAdvertisements === "true";
    if (stateRevision !== revision) return;
    const next = stored ?? defaultState();
    if (next) {
      deckState = next;
      if (!stored) await save(next);
    }
  } catch {
    if (stateRevision === revision) deckState = null;
  }
}

async function setAppearance(update: Partial<DeckAppearance>) {
  const next = { ...appearance, ...update };
  try {
    const result = await runOperation(() => commands.settingSet(DECK_APPEARANCE_SETTING, JSON.stringify(next)));
    saveError = !accepted(result);
    if (accepted(result)) appearance = next;
  } catch { saveError = true; }
}

async function setHideAdvertisements(hidden: boolean) {
  try {
    const result = await runOperation(() => commands.settingSet(HIDE_ADVERTISEMENTS_SETTING, String(hidden)));
    saveError = !accepted(result);
    if (accepted(result)) hideAdvertisements = hidden;
  } catch { saveError = true; }
}

function activeState(change: (deck: Deck) => Deck): DeckState | null {
  if (!deckState || !activeDeck) return null;
  return {
    ...deckState,
    decks: deckState.decks.map((deck) => (deck.id === activeDeck.id ? change(deck) : deck)),
  };
}

async function addColumn(target: ColumnTarget, account: string, width: number, row: number) {
  const id = newUlid();
  const next = activeState((deck) => ({
    ...deck,
    columns: [
      ...deck.columns,
      {
        id,
        account,
        target,
        width,
        row,
        suspended: false,
        secret: false,
      },
    ],
  }));
  return next && await save(next) ? id : null;
}

async function updateColumn(id: string, update: (column: DeckColumn) => DeckColumn) {
  const next = activeState((deck) => ({
    ...deck,
    columns: deck.columns.map((column) => {
      if (column.id !== id) return column;
      const next = update(column);
      return next.account === column.account ? next : { ...next, id: newUlid() };
    }),
  }));
  return next ? save(next) : false;
}

async function removeColumn(id: string) {
  const next = activeState((deck) => ({
    ...deck,
    columns: deck.columns.filter((column) => column.id !== id),
  }));
  return next ? save(next) : false;
}

async function duplicateColumn(column: DeckColumn) {
  const next = activeState((deck) => {
    const index = deck.columns.findIndex((candidate) => candidate.id === column.id);
    const copy = { ...column, id: newUlid() };
    const columns = [...deck.columns];
    columns.splice(index + 1, 0, copy);
    return { ...deck, columns };
  });
  return next ? save(next) : false;
}

async function reorderColumn(id: string, before: string | null, row?: 0 | 1) {
  if (!activeDeck) return false;
  const moved = repositionColumn(activeDeck, id, before, row);
  if (!moved) return false;
  const next = activeState(() => moved);
  return next ? save(next) : false;
}

async function moveColumn(id: string, offset: -1 | 1) {
  const columns = activeDeck?.columns;
  const index = columns?.findIndex((column) => column.id === id) ?? -1;
  const target = columns?.[index + offset];
  if (!columns || index < 0 || !target) return;
  if (offset < 0) return reorderColumn(id, target.id);
  return reorderColumn(target.id, id);
}

async function toggleSuspended(id: string) {
  return updateColumn(id, (column) => ({ ...column, suspended: !column.suspended }));
}

async function toggleSecret(id: string) {
  return updateColumn(id, (column) => ({ ...column, secret: !column.secret }));
}

function columnAction(id: string, action: "reload" | "back" | "forward") {
  if (action === "reload") void commands.tabsReload(id);
  if (action === "back") void commands.tabsBack(id);
  if (action === "forward") void commands.tabsForward(id);
}

function setRowRatio(rowRatio: number) {
  const next = activeState((deck) => ({ ...deck, row_ratio: rowRatio }));
  if (next) void save(next);
}

async function setColumnRow(id: string, row: 0 | 1) {
  return updateColumn(id, (column) => ({ ...column, row }));
}

async function openComposer(): Promise<boolean> {
  if (readyAccounts.length === 0) return false;
  try {
    if (!await surface.close()) return false;
    return accepted(await runOperation(() => commands.runCommand("composer.open")));
  } catch { return false; }
}

async function createAccount() {
  try {
    const result = await runOperation(() => profiles.create());
    saveError = !accepted(result);
    return accepted(result);
  } catch {
    saveError = true;
    return false;
  }
}

let initialized = false;
let stopState: (() => void) | null = null;

async function init() {
  if (initialized) return;
  initialized = true;
  stopState = await events.uiCommand.listen(({ payload }) => {
    if (!payload.startsWith("deck.state=")) return;
    const next = parseStored(payload.slice("deck.state=".length));
    if (next) { stateRevision += 1; deckState = next; saveError = false; }
  });
  await profiles.retain();
  await load();
  const metadata = await Promise.allSettled([accountSessions.init(), accountAvatars.init()]);
  for (const result of metadata) if (result.status === "rejected") console.error("account metadata initialization failed", result.reason);
}

function dispose() {
  if (!initialized) return;
  initialized = false;
  stopState?.();
  stopState = null;
  profiles.release();
  accountDeletion.dispose();
  accountSessions.dispose();
  accountAvatars.dispose();
}

export const deck = {
  init, dispose,
  state: () => deckState,
  active: () => activeDeck,
  readyAccounts: () => readyAccounts,
  loginRequired: () => deckState !== null && (deckState.accounts.length === 0 || deckState.accounts.every((account) => account.identity === null || accountSessions.status(account.id) === "logged_out" || accountSessions.status(account.id) === "account_changed")),
  appearance: () => appearance,
  hideAdvertisements: () => hideAdvertisements,
  setHideAdvertisements,
  failed: () => saveError,
  displayName: (id: string): string => {
    const account = deckState?.accounts.find((account) => account.id === id);
    return account?.identity?.display_name ?? "";
  },
  save, setAppearance, addColumn, updateColumn, removeColumn, duplicateColumn, moveColumn, reorderColumn,
  toggleSuspended, toggleSecret, columnAction, setRowRatio, setColumnRow, createAccount, openComposer,
};
