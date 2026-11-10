import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
const source = readFileSync(new URL("./composer-focus.js", import.meta.url), "utf8");
function composer(text, focused = true) {
  const calls = [];
  class HTMLElement {}
  const listeners = new Set();
  const editor = new HTMLElement();
  editor.dataset = {};
  editor.isConnected = true;
  editor.addEventListener = () => {};
  editor.removeEventListener = () => {};
  editor.textContent = text;
  editor.closest = () => ({});
  editor.focus = () => { calls.push("focus"); document.activeElement = editor; };
  let selected = 0;
  const selection = { removeAllRanges() {}, addRange(range) { selected = range.end; } };
  const document = {
    hasFocus: () => focused,
    querySelectorAll: () => [editor],
    createRange: () => ({ end: 0, selectNodeContents(value) { assert.equal(value, editor); }, collapse(value) { assert.equal(value, true); }, setEnd(node, offset) { this.end = offset; } }),
    createTreeWalker: () => { let next = { textContent: editor.textContent }; return { nextNode() { const current = next; next = null; return current; } }; },
    execCommand(command, ui, value) { assert.equal(command, "insertText"); assert.equal(ui, false); calls.push(value); editor.textContent = value + editor.textContent.slice(selected); return true; },
  };
  const window = { getSelection: () => selection, addEventListener: (_, listener) => listeners.add(listener), removeEventListener: (_, listener) => listeners.delete(listener) };
  return {
    calls,
    get text() { return editor.textContent; },
    edit(value) { editor.textContent = value; },
    focus(mention) { runInNewContext(source, { mention, HTMLElement, NodeFilter: { SHOW_TEXT: 4 }, document, window, location: { hostname: "x.com", pathname: "/compose/post" } }); },
    activate() { focused = true; for (const listener of listeners) listener(); },
  };
}
test("inserts the recipient at the start while preserving the draft", () => {
  const result = composer("existing draft");
  result.focus("Viewed_User");
  assert.equal(result.text, "@Viewed_User existing draft");
});
test("does not duplicate a manually entered recipient regardless of letter case", () => {
  const result = composer("@viewed_user existing draft");
  result.focus("Viewed_User");
  result.focus(null);
  assert.equal(result.text, "@viewed_user existing draft");
});
test("ordinary focus leaves text untouched", () => {
  const result = composer("existing draft");
  result.focus(undefined);
  assert.equal(result.text, "existing draft");
});
test("waits for actual window focus before inserting and inserts only once", () => {
  const result = composer("existing draft", false);
  result.focus("Viewed_User");
  assert.equal(result.text, "existing draft");
  result.activate();
  result.activate();
  assert.equal(result.text, "@Viewed_User existing draft");
  assert.deepEqual(result.calls, ["focus", "@Viewed_User "]);
});
test("an existing recipient without trailing whitespace is not duplicated", () => {
  const result = composer("@viewed_user");
  result.focus("Viewed_User");
  assert.equal(result.text, "@viewed_user");
});
test("returning home or opening from notifications removes only the automatic recipient", () => {
  const result = composer("draft @manual");
  result.focus("Viewed_User");
  result.focus(undefined);
  assert.equal(result.text, "@Viewed_User draft @manual");
  result.focus(null);
  assert.equal(result.text, "draft @manual");
});
test("switching profiles replaces the automatic recipient without accumulating mentions", () => {
  const result = composer("draft");
  result.focus("First");
  result.focus("Second");
  result.focus("Second");
  assert.equal(result.text, "@Second draft");
});
test("a later ordinary post cancels an unfocused pending recipient", () => {
  const result = composer("draft", false);
  result.focus("First");
  result.focus(null);
  result.activate();
  assert.equal(result.text, "draft");
});
test("manually replaced recipient text is preserved", () => {
  const result = composer("");
  result.focus("First");
  result.edit("@Manual draft");
  result.focus(null);
  assert.equal(result.text, "@Manual draft");
});
test("a nonbreaking space after an automatic recipient is removed with it", () => {
  const result = composer("draft");
  result.focus("First");
  result.edit("@First\u00a0draft");
  result.focus(null);
  assert.equal(result.text, "draft");
});
