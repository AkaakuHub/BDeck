import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import assert from "node:assert/strict";
import test from "node:test";

const create = runInNewContext(`(${readFileSync(new URL("./home-preference.js", import.meta.url), "utf8")})`);
function store(initial) {
  const values = new Map(initial === undefined ? [] : [["bdeck.home-timeline", initial]]);
  let writes = 0;
  return {
    getItem: key => values.get(key) ?? null,
    setItem(key, value) { values.set(key, value); writes++; },
    removeItem: key => values.delete(key),
    get writes() { return writes; },
    get size() { return values.size; },
  };
}
const plain = value => JSON.parse(JSON.stringify(value));
test("an unconfigured account starts with Following in chronological order", () => {
  assert.deepEqual(plain(create(store()).get()), { tab: "react:home_latest", sort: "mostRecent" });
});
test("the chosen timeline and ordering survive closing and opening another document", () => {
  const storage = store();
  let preference = create(storage);
  preference.save("react:home_latest", "popular");
  preference.save("react:list-123");
  preference = create(storage);
  assert.deepEqual(plain(preference.get()), { tab: "react:list-123", sort: "popular" });
  preference.save("react:home");
  assert.equal(create(storage).get().tab, "react:home");
  preference.save("cpft:separated");
  assert.equal(create(storage).get().tab, "cpft:separated");
});
test("repeated updates use one bounded record and do not rewrite unchanged values", () => {
  const storage = store();
  const preference = create(storage);
  for (let i = 0; i < 100; i++) preference.save("react:topic-456");
  assert.equal(storage.writes, 1);
  assert.equal(storage.size, 1);
  for (let i = 0; i < 100; i++) preference.save(`react:list-${i}`);
  assert.equal(storage.size, 1);
});
test("accounts do not share their timeline preferences", () => {
  const first = store();
  const second = store();
  create(first).save("react:home", "popular");
  assert.equal(create(second).get().tab, "react:home_latest");
});
test("malformed or oversized records are cleared without accepting an invalid selection", () => {
  for (const encoded of ["null", "not JSON", "x".repeat(1025), JSON.stringify({ tab: "", sort: "popular" }), JSON.stringify({ tab: "react:home", sort: "unsupported" })]) {
    const storage = store(encoded);
    assert.equal(create(storage).get().tab, "react:home_latest");
    assert.equal(storage.size, 0);
  }
});
test("invalid new selections never overwrite the current preference", () => {
  const storage = store();
  const preference = create(storage);
  for (const tab of ["", "x".repeat(257), "react:home\n"]) assert.throws(() => preference.save(tab));
  assert.throws(() => preference.save("react:home", "unsupported"));
  assert.equal(storage.writes, 0);
});
