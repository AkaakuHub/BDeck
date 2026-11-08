(() => {
  "use strict";
  const g = globalThis;
  const config = typeof __zephiumConfig === "object" ? __zephiumConfig : {};
  if (!config.sharedStorage) return;
  const api = g.chrome || g.browser;
  const runtime = api?.runtime, storage = api?.storage;
  if (!runtime?.id || !storage || g[Symbol.for("zephium.storage")]) return;
  const retained = [api, runtime, storage];
  g[Symbol.for("zephium.storage")] = retained;
  const native = config.sharedStorage === "native";
  const connectionAttempts = config.sharedStorage === "extensionPage" ? 600 : 40;
  const worker = typeof ServiceWorkerGlobalScope !== "undefined" && g instanceof ServiceWorkerGlobalScope;
  const origin = runtime.getURL("/");
  const content = !worker && !(typeof location !== "undefined" && location.href.startsWith(origin));
  const background = worker || g[Symbol.for("zephium.storage.background")] === true;
  const channel = "__zephium_shared_storage";
  const listeners = new Set(), areaListeners = new Map(), proxies = new Map();
  const pending = new Map();
  let connection, port, next = 1, reconnects = 0;
  let tail = Promise.resolve(), queued = 0, queuedBytes = 0;
  const observed = () => listeners.size || proxies.size || [...areaListeners.values()].some(set => set.size);
  const emit = message => {
    for (const listener of [...listeners]) { try { listener(message.changes, message.area); } catch (error) { console.error(error); } }
    for (const listener of [...(areaListeners.get(message.area) || [])]) { try { listener(message.changes); } catch (error) { console.error(error); } }
    for (const [proxy, untrusted] of proxies) {
      if (!untrusted || message.contentScripts === true) { try { proxy.postMessage(message); } catch {} }
    }
  };
  const event = set => ({
    addListener(listener) { if (typeof listener !== "function") throw new TypeError("A storage listener must be a function"); set.add(listener); void connect().catch(error => console.error(error)); },
    removeListener(listener) { set.delete(listener); },
    hasListener(listener) { return set.has(listener); },
    hasListeners() { return set.size !== 0; },
  });
  function connect() {
    if (connection) return connection;
    connection = new Promise((resolve, reject) => {
      let ready = false, hello, attempts = 0;
      const start = () => {
        try {
          const opened = native && !content ? runtime.connectNative("app.zephium.storage") : runtime.connect({name: channel});
          port = opened;
          const fail = error => {
            if (port !== opened) return;
            clearInterval(hello);
            port = null;
            if (!ready && config.sharedStorage === "extensionPage" && attempts < connectionAttempts) {
              setTimeout(start, 25);
              return;
            }
            connection = null;
            const reason = new Error(error || "Extension storage connection closed");
            if (!ready) reject(reason);
            for (const request of pending.values()) { clearTimeout(request.timeout); request.reject(reason); }
            pending.clear();
            if (observed() && reconnects++ < 3) setTimeout(() => { if (observed()) void connect().catch(error => console.error(error)); }, 250 * reconnects);
          };
          opened.onDisconnect.addListener(() => fail(runtime.lastError?.message));
          opened.onMessage.addListener(message => {
            if (port !== opened || !message || typeof message !== "object") return;
            if (message.type === "hello") { ready = true; reconnects = 0; clearInterval(hello); resolve(opened); return; }
            if (message.type === "changed") { emit(message); return; }
            const request = pending.get(message.request);
            if (!request) return;
            pending.delete(message.request); clearTimeout(request.timeout);
            if (typeof message.error === "string") request.reject(new Error(message.error));
            else request.resolve(message.value);
          });
          const greet = () => {
            if (++attempts > connectionAttempts) { fail("Extension storage connection timed out"); try { opened.disconnect(); } catch {} return; }
            try { opened.postMessage({type:"hello"}); } catch (error) { fail(String(error)); }
          };
          hello = setInterval(greet, 25); greet();
        } catch (error) { connection = null; reject(error); }
      };
      const compat = g[Symbol.for("zephium.compat")];
      Promise.resolve(compat?.workerReady).then(start, reject);
    });
    return connection;
  }
  function request(area, operation, untrusted = content) {
    let serialized;
    try { serialized = JSON.stringify({area, operation, contentScript:untrusted}); }
    catch (error) { return Promise.reject(error); }
    const bytes = serialized.length * 2;
    if (queued >= 512 || queuedBytes + bytes > 64 * 1024 * 1024) return Promise.reject(new Error("Extension storage queue is full"));
    queued++; queuedBytes += bytes;
    const result = tail.then(() => send(JSON.parse(serialized)));
    tail = result.catch(() => {}).finally(() => { queued--; queuedBytes -= bytes; });
    return result;
  }
  async function send(message) {
    const opened = await connect();
    if (next > Number.MAX_SAFE_INTEGER) throw new Error("Too many storage requests");
    const id = next++;
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => { pending.delete(id); reject(new Error("Extension storage request timed out")); }, 15000);
      pending.set(id, {resolve, reject, timeout});
      try { opened.postMessage({request:id, ...message}); }
      catch (error) { clearTimeout(timeout); pending.delete(id); reject(error); }
    });
  }
  const callback = (promise, done) => {
    if (typeof done !== "function") return promise;
    promise.then(done, error => {
      const previous = Object.getOwnPropertyDescriptor(runtime, "lastError");
      try {
        Object.defineProperty(runtime, "lastError", {configurable:true, value:{message:String(error?.message || error)}});
        done(undefined);
      } finally {
        if (previous) Object.defineProperty(runtime, "lastError", previous);
        else delete runtime.lastError;
      }
    });
  };
  const pin = (object, key, value) => Object.defineProperty(object, key, {configurable:true, enumerable:true, writable:true, value});
  const changesEvent = storage.onChanged || {};
  for (const [method, value] of Object.entries(event(listeners))) pin(changesEvent, method, value);
  retained.push(changesEvent);
  pin(storage, "onChanged", changesEvent);
  for (const name of ["local", "sync", "session"]) {
    const area = storage[name];
    if (!area) continue;
    const changed = new Set(); areaListeners.set(name, changed);
    const changesEvent = area.onChanged || {};
    for (const [method, value] of Object.entries(event(changed))) pin(changesEvent, method, value);
    retained.push(area, changesEvent);
    pin(area, "onChanged", changesEvent);
    for (const method of ["get", "getBytesInUse"]) pin(area, method, function(keys, done) {
      if (typeof keys === "function") { done = keys; keys = null; }
      return callback(request(name, {method, keys:keys ?? null}), done);
    });
    pin(area, "getKeys", done => callback(request(name, {method:"getKeys"}), done));
    pin(area, "set", (items, done) => callback(request(name, {method:"set", items}).then(() => undefined), done));
    pin(area, "remove", (keys, done) => callback(request(name, {method:"remove", keys}).then(() => undefined), done));
    pin(area, "clear", done => callback(request(name, {method:"clear"}).then(() => undefined), done));
    pin(area, "setAccessLevel", (options, done) => callback(request(name, {method:"setAccessLevel", accessLevel:options?.accessLevel}).then(() => undefined), done));
    pin(storage, name, area);
  }
  pin(api, "storage", storage);
  const scripting = api.scripting;
  if (scripting && typeof config.storageScript === "string") {
    const prepare = details => details.world === "MAIN" || !Array.isArray(details.js)
      ? details : {...details, js:[config.storageScript, ...details.js.filter(path => path !== config.storageScript)]};
    for (const name of ["registerContentScripts", "updateContentScripts"]) {
      const original = scripting[name];
      if (typeof original !== "function") continue;
      pin(scripting, name, function(scripts, done) {
        return callback(Reflect.apply(original, this, [scripts.map(prepare)]), done);
      });
    }
    const execute = scripting.executeScript;
    if (typeof execute === "function") pin(scripting, "executeScript", function(details, done) {
      if (details.world === "MAIN") return Reflect.apply(execute, this, [details, done]);
      if (Array.isArray(details.files)) return callback(Reflect.apply(execute, this,
        [{...details, files:[config.storageScript, ...details.files.filter(path => path !== config.storageScript)]}]), done);
      const bootstrap = {target:details.target, files:[config.storageScript]};
      if (details.world !== undefined) bootstrap.world = details.world;
      if (details.injectImmediately !== undefined) bootstrap.injectImmediately = details.injectImmediately;
      return callback(Promise.resolve(Reflect.apply(execute, this, [bootstrap]))
        .then(() => Reflect.apply(execute, this, [details])), done);
    });
  }
  if (native && background) runtime.onConnect.addListener(proxy => {
    if (proxy.name !== channel || proxy.sender?.id !== runtime.id) return;
    const untrusted = typeof proxy.sender.url !== "string" || !proxy.sender.url.startsWith(origin);
    proxies.set(proxy, untrusted);
    proxy.onDisconnect.addListener(() => proxies.delete(proxy));
    proxy.onMessage.addListener(message => {
      if (message?.type === "hello") { proxy.postMessage({type:"hello"}); void connect().catch(error => console.error(error)); return; }
      if (!Number.isSafeInteger(message?.request) || message.request < 1) return;
      request(message.area, message.operation, untrusted).then(value => {
        try { proxy.postMessage({request:message.request, value}); } catch {}
      }, error => { try { proxy.postMessage({request:message.request, error:String(error?.message || error)}); } catch {} });
    });
  });
})();
