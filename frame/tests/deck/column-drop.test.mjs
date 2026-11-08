import { test } from "node:test";
import assert from "node:assert/strict";
import { columnDrop } from "../../src/domain/deck/column-drop.ts";

const bounds = { left: 56, top: 48, right: 1016, bottom: 800 };
const frames = ["a", "b", "c"].map((id, i) => ({ id, x: 56 + i * 320, y: 48, width: 320, height: 40 }));
const columns = frames.map(({ id }) => ({ id, row: 0 }));

test("dropping over the lower part creates a lower row with an indicator inside its bounds", () => {
  const target = columnDrop(frames, columns, "b", { x: 400, y: 700 }, bounds);
  assert.equal(target.row, 1);
  assert.equal(target.before, null);
  assert.deepEqual(target.preview, [56, 424, 960, 376]);
});

test("a carried column is excluded when finding the insertion position", () => {
  const target = columnDrop(frames, columns, "b", { x: 380, y: 60 }, bounds);
  assert.equal(target.row, 0);
  assert.equal(target.before, "c");
  assert.equal(target.preview[0], 696);
  assert.equal(columnDrop(frames, columns, "b", { x: 20, y: 60 }, bounds), null);
});

test("a lower column can be placed before an upper column", () => {
  const splitFrames = [frames[0], { ...frames[1], x: 56, y: 424 }];
  const target = columnDrop(splitFrames, [{ id: "a", row: 0 }, { id: "b", row: 1 }], "b", { x: 60, y: 60 }, bounds);
  assert.equal(target.row, 0);
  assert.equal(target.before, "a");
  assert.deepEqual(target.preview, [56, 48, 2, 376]);
});
