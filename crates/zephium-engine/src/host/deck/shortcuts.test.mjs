import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
const source = readFileSync(new URL("./shortcuts.js", import.meta.url), "utf8");
function shortcuts(pathname = "/Viewed_User", overlayOpen = false) {
  const listeners = new Map();
  const frames = [];
  const messages = [];
  class Element { closest() { return this.editable ? this : null; } }
  const window = { ipc: { postMessage: value => messages.push(value) } };
  window.top = window;
  const location = { hostname: "x.com", pathname, href: `https://x.com${pathname}` };
  runInNewContext(source, { composerOverlay: () => overlayOpen, window, document: { addEventListener: (name, callback) => listeners.set(name, callback), body: null }, location, Element, requestAnimationFrame: callback => { frames.push(callback); return frames.length; } });
  return { messages, navigate(path) { location.href = `https://x.com${path}`; location.pathname = path; }, dispatch(name, values = {}) {
    const target = new Element();
    target.editable = values.editable ?? false;
    let prevented = false;
    const event = { isTrusted: true, defaultPrevented: false, isComposing: false, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, deltaX: 0, deltaY: 0, deltaMode: 0, composedPath: () => [target], preventDefault() { prevented = true; }, stopImmediatePropagation() {}, ...values };
    listeners.get(name)(event);
    while (frames.length) frames.shift()();
    return prevented;
  } };
}
test("ordinary horizontal trackpad input is left to browser navigation", () => {
  const page = shortcuts();
  assert.equal(page.dispatch("wheel", { deltaX: 150 }), false);
  assert.deepEqual(page.messages, []);
});
test("shift wheel explicitly scrolls the deck instead", () => {
  const page = shortcuts();
  assert.equal(page.dispatch("wheel", { shiftKey: true, deltaY: 120 }), true);
  assert.deepEqual(page.messages, ["deck.scrollBy=120"]);
});
test("N continues to work after repeated opens and closes", () => {
  const profile = shortcuts();
  const composer = shortcuts("/compose/post");
  for (let count = 0; count < 3; count++) {
    assert.equal(profile.dispatch("keydown", { key: "n" }), true);
    assert.equal(composer.dispatch("keydown", { key: "Escape", editable: true }), true);
  }
  assert.deepEqual(profile.messages, Array(3).fill('composer.open="https://x.com/Viewed_User"'));
  assert.deepEqual(composer.messages, Array(3).fill("composer.close"));
});
test("typing N in a draft or input does not open a new composer", () => {
  const page = shortcuts();
  assert.equal(page.dispatch("keydown", { key: "n", editable: true }), false);
  assert.deepEqual(page.messages, []);
});

test("N reads the current URL on every keypress after same-document navigation", () => {
  const page = shortcuts();
  for (const path of ["/Viewed_User", "/home", "/notifications"]) {
    page.navigate(path);
    page.dispatch("keydown", { key: "n" });
  }
  assert.deepEqual(page.messages, [
    'composer.open="https://x.com/Viewed_User"',
    'composer.open="https://x.com/home"',
    'composer.open="https://x.com/notifications"',
  ]);
});

test("Escape is left to X while a composer menu is open", () => {
  const page = shortcuts("/compose/post", true);
  assert.equal(page.dispatch("keydown", { key: "Escape", editable: true }), false);
  assert.deepEqual(page.messages, []);
});
