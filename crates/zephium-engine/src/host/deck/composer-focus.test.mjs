import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
const source = readFileSync(new URL("./composer-focus.js", import.meta.url), "utf8");
function focus(text, mention, focused = true) {
  const calls = [];
  class HTMLElement {}
  const listeners = new Set();
  const editor = new HTMLElement();
  editor.dataset = {};
  editor.isConnected = true;
  editor.addEventListener = () => {};
  editor.removeEventListener = () => {};
  editor.innerText = text;
  editor.closest = () => ({});
  editor.focus = () => { calls.push("focus"); document.activeElement = editor; };
  const selection = { removeAllRanges() {}, addRange() {} };
  const document = {
    hasFocus: () => focused,
    querySelectorAll: () => [editor],
    createRange: () => ({ selectNodeContents(value) { assert.equal(value, editor); }, collapse(value) { assert.equal(value, true); } }),
    execCommand(command, ui, value) { assert.equal(command, "insertText"); assert.equal(ui, false); calls.push(value); editor.innerText = value + editor.innerText; return true; },
  };
  runInNewContext(source, { mention, HTMLElement, document, window: { getSelection: () => selection, addEventListener: (_, listener) => listeners.add(listener), removeEventListener: (_, listener) => listeners.delete(listener) }, location: { hostname: "x.com", pathname: "/compose/post" } });
  return { calls, get text() { return editor.innerText; }, activate() { focused = true; for (const listener of listeners) listener(); } };
}
test("inserts the recipient at the start while preserving the draft", () => {
  const result = focus("existing draft", "Viewed_User");
  assert.equal(result.text, "@Viewed_User existing draft");
  assert.deepEqual(result.calls, ["focus", "@Viewed_User "]);
});
test("does not duplicate an existing recipient regardless of letter case", () => {
  const result = focus("@viewed_user existing draft", "Viewed_User");
  assert.equal(result.text, "@viewed_user existing draft");
  assert.deepEqual(result.calls, ["focus"]);
});
test("ordinary focus leaves text untouched", () => {
  const result = focus("existing draft", null);
  assert.deepEqual(result.calls, ["focus"]);
  assert.equal(result.text, "existing draft");
});

test("waits for actual window focus before inserting and inserts only once", () => {
  const result = focus("existing draft", "Viewed_User", false);
  assert.equal(result.text, "existing draft");
  result.activate();
  result.activate();
  assert.equal(result.text, "@Viewed_User existing draft");
  assert.deepEqual(result.calls, ["focus", "@Viewed_User "]);
});

test("an existing recipient without trailing whitespace is not duplicated", () => {
  const result = focus("@viewed_user", "Viewed_User");
  assert.equal(result.text, "@viewed_user");
  assert.deepEqual(result.calls, ["focus"]);
});
