import assert from "node:assert/strict";
import { after, test } from "node:test";
import { setImmediate } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";

const server = await createServer({ configFile: false, root: fileURLToPath(new URL("../../", import.meta.url)), appType: "custom", optimizeDeps: { noDiscovery: true, include: [] }, server: { middlewareMode: true, hmr: false, watch: null } });
const { accepted, runOperation } = await server.ssrLoadModule("/src/shared/ipc/operation.ts");
const original = globalThis.window;
globalThis.window = new EventTarget();
after(async () => { globalThis.window = original; await server.close(); });

const admission = { operation_id: "0000000000000001", accepted: true };
const processed = { operation_id: admission.operation_id, outcome: "deferred", reason: "native_pending" };
function emit(disposition) { window.dispatchEvent(new CustomEvent("zephium:operation-processed", { detail: disposition })); }

test("a processed result arriving before the IPC reply is retained", async () => {
  const result = await runOperation(async () => { emit(processed); return admission; });
  assert.deepEqual(result, { kind: "processed", disposition: processed });
  assert.equal(accepted(result), true);
});

test("queue admission alone never completes an operation, and another operation cannot complete it", async () => {
  let settled = false;
  const pending = runOperation(async () => admission).then((result) => { settled = true; return result; });
  await setImmediate();
  assert.equal(settled, false);
  emit({ ...processed, operation_id: "0000000000000002" });
  await setImmediate();
  assert.equal(settled, false);
  emit(processed);
  assert.deepEqual(await pending, { kind: "processed", disposition: processed });
});

test("cancellation and rejected admission remain distinct", async () => {
  assert.deepEqual(await runOperation(async () => null), { kind: "cancelled" });
  assert.deepEqual(await runOperation(async () => ({ accepted: false, operation_id: null })), { kind: "rejected" });
  await assert.rejects(runOperation(async () => { throw new Error("unavailable"); }), /unavailable/);
});

test("only logical application and the known pending native or store work are accepted", () => {
  for (const [outcome, reason] of [["applied", "mutation_applied"], ["no_op", "state_unchanged"], ["deferred", "native_pending"], ["deferred", "store_pending"]]) {
    assert.equal(accepted({ kind: "processed", disposition: { ...processed, outcome, reason } }), true);
  }
  for (const [outcome, reason] of [["rejected", "invalid_scope"], ["native_admission_failed", "native_dispatch_rejected"], ["deferred", "discard_completion_pending"], ["deferred", "store_outcome_unknown"]]) {
    assert.equal(accepted({ kind: "processed", disposition: { ...processed, outcome, reason } }), false);
  }
});
