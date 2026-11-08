import { test } from "node:test";
import assert from "node:assert/strict";
import { normalizeColumnRows, repositionColumn } from "../../src/domain/deck/columns.ts";

function deck() {
  return {
    id: "deck", name: "main", layout: "horizontal", row_ratio: 50,
    columns: ["a", "b", "c"].map((id) => ({ id, account: "account", target: { kind: "home" }, width: 320, row: 0, suspended: false })),
  };
}

test("moving a column to the lower row splits the deck without changing its account or identity", () => {
  const moved = repositionColumn(deck(), "b", null, 1);
  assert.equal(moved.layout, "two_rows");
  assert.deepEqual(moved.columns.map(({ id, row }) => [id, row]), [["a", 0], ["c", 0], ["b", 1]]);
  assert.equal(moved.columns[2].account, "account");
});

test("moving the last lower column back removes the second row", () => {
  const split = repositionColumn(deck(), "b", null, 1);
  const moved = repositionColumn(split, "b", "a", 0);
  assert.equal(moved.layout, "horizontal");
  assert.ok(moved.columns.every(({ row }) => row === 0));
});

test("a single remaining row fills the viewport even when the upper row was removed", () => {
  const state = deck();
  state.columns = state.columns.map((column) => ({ ...column, row: 1 }));
  const normalized = normalizeColumnRows(state);
  assert.equal(normalized.layout, "horizontal");
  assert.ok(normalized.columns.every(({ row }) => row === 0));
  assert.equal(repositionColumn(state, "missing", null, 0), null);
  assert.equal(repositionColumn(state, "a", "missing", 0), null);
});
