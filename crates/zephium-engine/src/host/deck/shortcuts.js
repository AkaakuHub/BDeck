(() => {
  const host = location.hostname.toLowerCase();
  if (window !== window.top || !(host === "x.com" || host.endsWith(".x.com") || host === "twitter.com" || host.endsWith(".twitter.com"))) return;
  const send = (command) => window.ipc.postMessage(command);
  document.addEventListener("click", event => {
    if (!event.isTrusted || event.defaultPrevented) return;
    const anchor = event.composedPath().find(node => node instanceof HTMLAnchorElement);
    if (!anchor) return;
    const target = new URL(anchor.href);
    const host = target.hostname.toLowerCase().replace(/\.$/, "");
    if (!["http:", "https:"].includes(target.protocol) || target.username || target.password || host === "x.com" || host.endsWith(".x.com") || host === "twitter.com" || host.endsWith(".twitter.com")) return;
    anchor.target = "_blank";
    anchor.rel = "noopener noreferrer";
  }, true);
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
  const measure = () => {
    document.documentElement.setAttribute("data-bdeck-compose", "");
    const editor = document.querySelector('[data-testid="tweetTextarea_0"][contenteditable="true"]');
    const toolbar = document.querySelector('[data-testid="toolBar"]');
    if (!(editor instanceof HTMLElement) || !(toolbar instanceof HTMLElement)) return;
    let content = editor.parentElement;
    while (content && !content.contains(toolbar)) content = content.parentElement;
    for (let node = content; node && node !== document.body; node = node.parentElement) node.classList.add("bdeck-compose-layout");
    const height = Math.max(160, Math.min(640, Math.ceil(toolbar.getBoundingClientRect().bottom + 12)));
    if (height !== lastHeight) { lastHeight = height; send(`composer.height=${height}`); }
  };
  let lastHeight = 0;
  let measuring = false;
  const scheduleMeasure = () => {
    if (measuring) return;
    measuring = true;
    requestAnimationFrame(() => { measuring = false; measure(); });
  };
  const readyEditor = () => {
    const editor = document.querySelector('[data-testid="tweetTextarea_0"][contenteditable="true"]');
    if (!(editor instanceof HTMLElement)) return false;
    send("composer.ready");
    return true;
  };
  const ready = () => {
    const sizing = new ResizeObserver(scheduleMeasure);
    sizing.observe(document.body);
    new MutationObserver(scheduleMeasure).observe(document.documentElement, { subtree: true, childList: true, characterData: true });
    scheduleMeasure();
    if (readyEditor()) return;
    const observer = new MutationObserver(() => { if (readyEditor()) observer.disconnect(); });
    observer.observe(document.documentElement, { subtree: true, childList: true });
    setTimeout(() => observer.disconnect(), 10000);
  };
  if (document.body) ready();
  else document.addEventListener("DOMContentLoaded", ready, { once: true });
})();
