import type { Deck } from "./deck-types";

export function normalizeColumnRows(deck: Deck): Deck {
  const upper = deck.columns.some((column) => column.row === 0);
  const lower = deck.columns.some((column) => column.row === 1);
  const split = upper && lower;
  return {
    ...deck,
    layout: split ? "two_rows" : "horizontal",
    columns: split ? deck.columns : deck.columns.map((column) => ({ ...column, row: 0 })),
  };
}

export function repositionColumn(deck: Deck, id: string, before: string | null, row?: 0 | 1): Deck | null {
  const column = deck.columns.find((candidate) => candidate.id === id);
  const destination = deck.columns.find((candidate) => candidate.id === before);
  if (!column || id === before || (before !== null && !destination)) return null;
  const columns = deck.columns.filter((candidate) => candidate.id !== id);
  const index = before === null ? columns.length : columns.findIndex((candidate) => candidate.id === before);
  columns.splice(index, 0, { ...column, row: row ?? destination?.row ?? column.row });
  return normalizeColumnRows({ ...deck, columns });
}
