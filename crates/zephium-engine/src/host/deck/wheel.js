(() => {
  const host = location.hostname.toLowerCase();
  if (window !== window.top || !(host === "x.com" || host.endsWith(".x.com") || host === "twitter.com" || host.endsWith(".twitter.com"))) return;
  const send = command => window.ipc.postMessage(command);
  let horizontal = 0;
  let scrollFrame = 0;
  document.addEventListener("wheel", (event) => {
    if (!event.isTrusted || !event.shiftKey || event.ctrlKey || event.metaKey || location.pathname.startsWith("/compose/")) return;
    const delta = event.deltaX || event.deltaY;
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
})();
