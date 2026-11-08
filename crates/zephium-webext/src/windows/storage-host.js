(() => {
  const runtime = chrome.runtime;
  const origin = runtime.getURL('/');
  const ports = new Map(), pending = new Map();
  let next = 1;
  chrome.webview.addEventListener('message', ({data}) => {
    if (data?.kind !== 'storage') return;
    if (data.type === 'changed') {
      for (const [port, untrusted] of ports) {
        if (!untrusted || data.contentScripts === true) {
          try { port.postMessage(data); } catch {}
        }
      }
      return;
    }
    const request = pending.get(data.request);
    if (!request) return;
    pending.delete(data.request);
    clearTimeout(request.timeout);
    try { request.port.postMessage({...data, request:request.id}); } catch {}
  });
  runtime.onConnect.addListener(port => {
    if (port.name !== '__zephium_shared_storage' || port.sender?.id !== runtime.id) return;
    if (ports.size >= 128) { port.disconnect(); return; }
    const untrusted = typeof port.sender.url !== 'string' || !port.sender.url.startsWith(origin);
    ports.set(port, untrusted);
    port.onDisconnect.addListener(() => {
      ports.delete(port);
      for (const [id, request] of pending) {
        if (request.port === port) { clearTimeout(request.timeout); pending.delete(id); }
      }
    });
    port.onMessage.addListener(message => {
      if (message?.type === 'hello') { port.postMessage({type:'hello'}); return; }
      if (!Number.isSafeInteger(message?.request) || message.request < 1) return;
      if (pending.size >= 512 || next > Number.MAX_SAFE_INTEGER) {
        port.postMessage({request:message.request, error:'Extension storage is busy.'}); return;
      }
      const id = next++;
      const timeout = setTimeout(() => {
        pending.delete(id);
        try { port.postMessage({request:message.request, error:'Extension storage request timed out.'}); } catch {}
      }, 15000);
      pending.set(id, {port, id:message.request, timeout});
      chrome.webview.postMessage(JSON.stringify({kind:'storage', request:id, area:message.area,
        operation:message.operation, contentScript:untrusted}));
    });
  });
})();
