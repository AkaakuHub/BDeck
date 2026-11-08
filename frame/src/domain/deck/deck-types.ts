import type { AccountIdentity } from "$shared/ipc/bindings";
export type DeckLayout = "horizontal" | "two_rows";

export type ColumnTarget =
  | { kind: "home" }
  | { kind: "notifications" }
  | { kind: "search"; query: string }
  | { kind: "list"; url: string }
  | { kind: "url"; url: string };

export type DeckAccount = {
  id: string;
  profile: string;
  identity: AccountIdentity | null;
};

export type DeckColumn = {
  id: string;
  account: string;
  target: ColumnTarget;
  width: number;
  row: number;
  suspended: boolean;
};

export type Deck = {
  id: string;
  name: string;
  layout: DeckLayout;
  row_ratio: number;
  columns: DeckColumn[];
};

export type DeckState = {
  accounts: DeckAccount[];
  decks: Deck[];
  active: string;
};

export const DECK_SETTING = "deck.state";
export const DECK_APPEARANCE_SETTING = "deck.appearance";
export const DEFAULT_COLUMN_WIDTH = 320;
export const MIN_COLUMN_WIDTH = 240;
export const MAX_COLUMN_WIDTH = 960;
export const MIN_ROW_RATIO = 20;
export const MAX_ROW_RATIO = 80;
export const DEFAULT_ROW_RATIO = 50;

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function isAccountIdentity(value: unknown): value is AccountIdentity {
  return isRecord(value) && typeof value.user_id === "string" && /^[1-9][0-9]{0,19}$/u.test(value.user_id)
    && typeof value.display_name === "string" && typeof value.screen_name === "string";
}

function isTarget(value: unknown): value is ColumnTarget {
  if (!isRecord(value) || typeof value.kind !== "string") return false;
  if (value.kind === "home" || value.kind === "notifications") return true;
  if (value.kind === "search") return typeof value.query === "string";
  return (value.kind === "list" || value.kind === "url") && typeof value.url === "string";
}

export function isDeckState(value: unknown): value is DeckState {
  if (!isRecord(value) || typeof value.active !== "string") return false;
  if (!Array.isArray(value.accounts) || !Array.isArray(value.decks)) return false;
  if (
    !value.accounts.every(
      (account) =>
        isRecord(account) &&
        typeof account.id === "string" &&
        typeof account.profile === "string" &&
        (account.identity === null || isAccountIdentity(account.identity)),
    )
  )
    return false;
  return (
    value.decks.some((deck) => isRecord(deck) && deck.id === value.active) &&
    value.decks.every(
      (deck) =>
        isRecord(deck) &&
        typeof deck.id === "string" &&
        typeof deck.name === "string" &&
        (deck.layout === "horizontal" || deck.layout === "two_rows") &&
        typeof deck.row_ratio === "number" &&
        Number.isInteger(deck.row_ratio) &&
        deck.row_ratio >= MIN_ROW_RATIO &&
        deck.row_ratio <= MAX_ROW_RATIO &&
        Array.isArray(deck.columns) &&
        deck.columns.every(
          (column) =>
            isRecord(column) &&
            typeof column.id === "string" &&
            typeof column.account === "string" &&
            typeof column.width === "number" &&
            typeof column.row === "number" &&
            typeof column.suspended === "boolean" &&
            isTarget(column.target),
        ),
    )
  );
}

export function newUlid(): string {
  const random = new Uint8Array(10);
  crypto.getRandomValues(random);
  let value = BigInt(Date.now());
  for (const byte of random) value = (value << 8n) | BigInt(byte);
  const alphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
  let result = "";
  for (let index = 0; index < 26; index += 1) {
    result = alphabet[Number(value & 31n)] + result;
    value >>= 5n;
  }
  return result;
}

export function targetName(target: ColumnTarget): string {
  switch (target.kind) {
    case "home":
      return "ホーム";
    case "notifications":
      return "通知";
    case "search":
      return `検索: ${target.query}`;
    case "list":
      return "リスト";
    case "url":
      if (target.url === "https://x.com/messages") return "メッセージ";
      if (target.url === "https://x.com/explore") return "トレンド";
      return target.url;
  }
}
