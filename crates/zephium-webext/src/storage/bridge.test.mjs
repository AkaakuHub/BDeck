import {readFile} from 'node:fs/promises';
import vm from 'node:vm';
import test from 'node:test';
import assert from 'node:assert/strict';

const script = await readFile(new URL('./bridge.js', import.meta.url), 'utf8');
const extension = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const origin = `chrome-extension://${extension}/`;
const tick = () => new Promise(resolve => setImmediate(resolve));
function event() {
  const listeners = new Set();
  return {addListener: listener => listeners.add(listener), removeListener: listener => listeners.delete(listener), emit: value => { for (const listener of [...listeners]) listener(value); }};
}
function pair() {
  let closed = false;
  const left = {onMessage:event(), onDisconnect:event()}, right = {onMessage:event(), onDisconnect:event()};
  left.postMessage = message => { if (closed) throw new Error('closed'); const copy = structuredClone(message); queueMicrotask(() => { if (!closed) right.onMessage.emit(copy); }); };
  right.postMessage = message => { if (closed) throw new Error('closed'); const copy = structuredClone(message); queueMicrotask(() => { if (!closed) left.onMessage.emit(copy); }); };
  left.disconnect = right.disconnect = () => { if (closed) return; closed = true; left.onDisconnect.emit(); right.onDisconnect.emit(); };
  return [left, right];
}
function backend(handler = () => ({theme:'dark'})) {
  const ports = new Set(), requests = [];
  return {requests, connect() {
    const [client, host] = pair(); ports.add(host);
    host.onDisconnect.addListener(() => ports.delete(host));
    host.onMessage.addListener(message => {
      if (message.type === 'hello') { host.postMessage({type:'hello'}); return; }
      requests.push(message);
      Promise.resolve().then(() => handler(message)).then(value => host.postMessage({request:message.request, value}), error => host.postMessage({request:message.request, error:error.message}));
    });
    return client;
  }, changed(changes, contentScripts = true) { for (const port of ports) port.postMessage({type:'changed',area:'local',changes,contentScripts}); }};
}
function context(server, {worker=false, url=origin+'page.html', background=null} = {}) {
  let disposed = false;
  const timers = new Set(), opened = new Set();
  const runtime = {id:extension, getURL:path => origin+path.replace(/^\//,''), onConnect:event()};
  Object.defineProperty(runtime, 'lastError', {configurable:true, get:() => undefined});
  runtime.connectNative = name => { assert.equal(name, 'app.zephium.storage'); const port = server.connect(); opened.add(port); return port; };
  runtime.connect = ({name}) => {
    const [client, host] = pair(); host.name = name; host.sender = {id:extension, url}; opened.add(client);
    queueMicrotask(() => background.runtime.onConnect.emit(host));
    return client;
  };
  const area = () => ({get() { throw new Error('Profile-local storage was called'); }});
  const chrome = {runtime, storage:{local:area(),sync:area(),session:area()}};
  const scope = {chrome, __zephiumConfig:{sharedStorage:'native'}, location:{href:url}, console,
    setTimeout(fn, delay) { if (disposed) return 0; const handle = setTimeout(() => { timers.delete(handle); if (!disposed) fn(); },delay); timers.add(handle); return handle; },
    clearTimeout(handle) { clearTimeout(handle); timers.delete(handle); },
    setInterval(fn, delay) { const handle = setInterval(fn,delay); timers.add(handle); return handle; },
    clearInterval(handle) { clearInterval(handle); timers.delete(handle); },
  };
  if (worker) scope.ServiceWorkerGlobalScope = class {static [Symbol.hasInstance]() {return true;}};
  vm.runInNewContext(script, scope);
  return {chrome, runtime, dispose() { disposed = true; for (const port of opened) port.disconnect(); for (const handle of timers) {clearTimeout(handle);clearInterval(handle);} }};
}

test('promise and callback reads use the browser-wide transport without native profile-local reads', async t => {
  const server = backend(); const page = context(server); t.after(() => page.dispose());
  assert.deepEqual(await page.chrome.storage.local.get({theme:'light'}), {theme:'dark'});
  const result = await new Promise(resolve => page.chrome.storage.sync.get(null, resolve));
  assert.deepEqual(result, {theme:'dark'});
  assert.deepEqual(server.requests.map(request => [request.area,request.operation,request.contentScript]), [
    ['local',{method:'get',keys:{theme:'light'}},false], ['sync',{method:'get',keys:null},false],
  ]);
});

test('callback errors expose lastError only during the callback and promise errors reject', async t => {
  const server = backend(() => {throw new Error('quota rejected');}); const page = context(server); t.after(() => page.dispose());
  await new Promise(resolve => page.chrome.storage.local.set({value:1}, value => {
    assert.equal(value, undefined); assert.equal(page.runtime.lastError.message, 'quota rejected'); resolve();
  }));
  assert.equal(page.runtime.lastError, undefined);
  await assert.rejects(page.chrome.storage.local.set({value:1}), /quota rejected/);
});

test('change notifications reach independent extension pages once and can be unsubscribed', async t => {
  const server = backend(); const first = context(server), second = context(server); t.after(() => {first.dispose();second.dispose();});
  const one = [], two = [];
  const listener = (changes,area) => one.push({changes,area});
  first.chrome.storage.onChanged.addListener(listener);
  second.chrome.storage.local.onChanged.addListener(changes => two.push(changes));
  await Promise.all([first.chrome.storage.local.get(null),second.chrome.storage.local.get(null)]);
  const changes = {theme:{oldValue:'dark',newValue:'light'}};
  server.changed(changes); await tick();
  assert.deepEqual(one,[{changes,area:'local'}]); assert.deepEqual(two,[changes]);
  first.chrome.storage.onChanged.removeListener(listener);
  server.changed(changes); await tick();
  assert.equal(one.length,1); assert.equal(two.length,2);
});

test('content-script requests are classified from the native sender and cannot claim trusted access', async t => {
  const server = backend(); const worker = context(server,{worker:true});
  const content = context(server,{url:'https://test.invalid/',background:worker}); t.after(() => {content.dispose();worker.dispose();});
  assert.deepEqual(await content.chrome.storage.session.get(null),{theme:'dark'});
  assert.equal(server.requests.at(-1).contentScript,true);
  const [client,host] = pair(); host.name = '__zephium_shared_storage'; host.sender = {id:extension,url:'https://test.invalid/'};
  worker.runtime.onConnect.emit(host); client.postMessage({request:77,area:'local',operation:{method:'get',keys:null},contentScript:false});
  await tick(); assert.equal(server.requests.at(-1).contentScript,true); client.disconnect();
});

test('other extensions cannot reach the background proxy and restricted changes stay out of content scripts', async t => {
  const server = backend(); const worker = context(server,{worker:true});
  const content = context(server,{url:'https://test.invalid/',background:worker}); t.after(() => {content.dispose();worker.dispose();});
  const received = []; content.chrome.storage.local.onChanged.addListener(changes => received.push(changes));
  await content.chrome.storage.local.get(null);
  server.changed({secret:{newValue:1}},false); await tick(); assert.deepEqual(received,[]);
  server.changed({public:{newValue:2}},true); await tick(); assert.deepEqual(received,[{public:{newValue:2}}]);
  const [client,host] = pair(); host.name = '__zephium_shared_storage'; host.sender = {id:'b'.repeat(32),url:'https://test.invalid/'};
  const count = server.requests.length; worker.runtime.onConnect.emit(host);
  client.postMessage({request:1,area:'local',operation:{method:'get',keys:null}}); await tick();
  assert.equal(server.requests.length,count); client.disconnect();
});

test('startup bursts wait in order instead of failing the native pending request limit', async t => {
  let active = 0, peak = 0;
  const server = backend(async message => {
    peak = Math.max(peak, ++active);
    await tick();
    active--;
    return message.operation.keys;
  });
  const page = context(server); t.after(() => page.dispose());
  const expected = Array.from({length:96}, (_, index) => `key-${index}`);
  assert.deepEqual(await Promise.all(expected.map(key => page.chrome.storage.local.get(key))), expected);
  assert.equal(peak, 1);
  assert.deepEqual(server.requests.map(request => request.operation.keys), expected);
});

test('queued writes capture the call value and an error does not block later requests', async t => {
  const server = backend(message => {
    if (message.operation.method === 'remove') throw new Error('rejected');
    return message.operation.items;
  });
  const page = context(server); t.after(() => page.dispose());
  const items = {theme:'dark'};
  const write = page.chrome.storage.local.set(items);
  items.theme = 'light';
  const rejected = page.chrome.storage.local.remove('blocked');
  const after = page.chrome.storage.local.set({theme:'after'});
  await Promise.all([write, assert.rejects(rejected, /rejected/), after]);
  assert.deepEqual(server.requests.map(request => request.operation.method), ['set','remove','set']);
  assert.deepEqual(server.requests[0].operation.items, {theme:'dark'});
  assert.deepEqual(server.requests[2].operation.items, {theme:'after'});
});

test('the bounded queue rejects overflow while admitted requests still settle', async t => {
  const server = backend(message => message.operation.keys);
  const page = context(server); t.after(() => page.dispose());
  const calls = Array.from({length:513}, (_, index) => page.chrome.storage.local.get(String(index)));
  const results = await Promise.allSettled(calls);
  assert.equal(results.filter(result => result.status === 'fulfilled').length, 512);
  assert.equal(results[512].status, 'rejected');
  assert.match(results[512].reason.message, /queue is full/);
  assert.equal(server.requests.length, 512);
  assert.equal(await page.chrome.storage.local.get('after'), 'after');
});
