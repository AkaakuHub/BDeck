import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";

const source = readFileSync(new URL("./session.js", import.meta.url), "utf8");
const identity = { id_str: "123", name: "Actual name", screen_name: "actual_name" };
function session({ authenticated = false, loggedOut = false } = {}) {
  const messages = [];
  const callbacks = [];
  const tasks = [];
  const state = { authenticated, loggedOut };
  const document = {
    documentElement: {},
    get scripts() {
      const data = state.authenticated ? { session: { user_id: "123" }, entities: { users: { entities: { 123: identity } } } } : {};
      return [{ textContent: `window.__INITIAL_STATE__=${JSON.stringify(data)};window.__META_DATA__={}` }];
    },
    querySelector(selector) {
      return selector.includes("loginButton") && state.loggedOut ? {} : null;
    },
  };
  const window = { ipc: { postMessage(value) { messages.push(value); } } };
  window.top = window;
  runInNewContext(source, { window, document, location: { hostname: "x.com", pathname: "/home" }, MutationObserver: class {
    constructor(callback) { this.callback = callback; }
    observe() { callbacks.push(this.callback); }
    disconnect() {}
  }, setTimeout(callback) { tasks.push(callback); return tasks.length; } });
  return { state, messages, mutate() { for (const callback of callbacks) callback(); while (tasks.length) tasks.shift()(); } };
}

test("authenticated page data identifies a session without a desktop account-switcher control", () => {
  const page = session({ authenticated: true });
  assert.ok(page.messages.includes("account.logged-in"));
  assert.deepEqual(JSON.parse(page.messages.find(value => value.startsWith("account.identity=")).slice(17)), { user_id: "123", display_name: "Actual name", screen_name: "actual_name" });
});
test("identity reading retries after data arrives and emits login once", () => {
  const page = session();
  assert.equal(page.messages.length, 0);
  page.state.authenticated = true;
  page.mutate();
  page.mutate();
  assert.equal(page.messages.filter(value => value === "account.logged-in").length, 1);
});
test("explicit logout is not reversed by cached initial page data", () => {
  const page = session({ authenticated: true });
  page.state.loggedOut = true;
  page.mutate();
  page.mutate();
  assert.equal(page.messages.at(-1), "account.logged-out");
  assert.equal(page.messages.filter(value => value === "account.logged-in").length, 1);
});
test("anonymous pages do not manufacture a logged-in state", () => {
  const page = session({ loggedOut: true });
  assert.deepEqual(page.messages, ["account.logged-out"]);
});
