(() => {
  const host = location.hostname.toLowerCase();
  if (window !== window.top || !(host === "x.com" || host.endsWith(".x.com") || host === "twitter.com" || host.endsWith(".twitter.com"))) return;
  const send = (command) => window.ipc.postMessage(command);
  document.addEventListener("keydown", (event) => {
    if (!event.isTrusted || event.isComposing || event.metaKey || event.ctrlKey || event.altKey) return;
    const editable = event.composedPath().some(node => node instanceof Element && node.closest("input, textarea, select, [contenteditable], [role=textbox]"));
    if (event.key === "n" && !editable && !event.shiftKey) {
      event.preventDefault();
      event.stopImmediatePropagation();
      send("composer.open");
    } else if (event.key === "Escape" && location.pathname.startsWith("/compose/")) {
      event.preventDefault();
      event.stopImmediatePropagation();
      send("composer.close");
    }
  }, true);
  let horizontal = 0;
  let scrollFrame = 0;
  document.addEventListener("wheel", (event) => {
    if (!event.isTrusted || event.ctrlKey || event.metaKey || location.pathname.startsWith("/compose/")) return;
    const delta = event.shiftKey ? (event.deltaX || event.deltaY) : Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : 0;
    if (!Number.isFinite(delta) || Math.abs(delta) < 0.5) return;
    event.preventDefault();
    const scale = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? innerWidth : 1;
    horizontal = Math.max(-1000, Math.min(1000, horizontal + delta * scale));
    if (scrollFrame) return;
    scrollFrame = requestAnimationFrame(() => {
      send(`deck.scrollBy=${horizontal}`);
      horizontal = 0;
      scrollFrame = 0;
    });
  }, { passive: false });
  if (!location.pathname.startsWith("/compose/")) return;
  const focus = () => {
    const editor = document.querySelector('[data-testid="tweetTextarea_0"][contenteditable="true"]');
    if (!(editor instanceof HTMLElement)) return false;
    editor.focus({ preventScroll: true });
    send("composer.focus");
    return true;
  };
  const ready = () => {
    if (focus()) return;
    const observer = new MutationObserver(() => { if (focus()) observer.disconnect(); });
    observer.observe(document.documentElement, { subtree: true, childList: true });
    setTimeout(() => observer.disconnect(), 10000);
  };
  if (document.documentElement) ready();
  else document.addEventListener("DOMContentLoaded", ready, { once: true });
})();
