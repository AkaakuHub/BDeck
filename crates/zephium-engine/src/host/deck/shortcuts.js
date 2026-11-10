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
      send(`composer.open=${JSON.stringify(location.href)}`);
    } else if (event.key === "Escape" && location.pathname.startsWith("/compose/")) {
      event.preventDefault();
      event.stopImmediatePropagation();
      send("composer.close");
    }
  }, true);
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
  if (!location.pathname.startsWith("/compose/")) return;
  const measure = () => {
    document.documentElement.setAttribute("data-bdeck-compose", "");
    document.querySelectorAll(".bdeck-compose-background").forEach(node => node.classList.remove("bdeck-compose-background"));
    const editors = [...document.querySelectorAll('[data-testid="tweetTextarea_0"][contenteditable="true"]')];
    const editor = editors.find(node => node.closest('[role="dialog"]')) || editors.find(node => node.getClientRects().length && !node.closest('[aria-hidden="true"]'));
    const dialog = editor?.closest('[role="dialog"]');
    if (dialog) {
      for (let node = dialog; node?.parentElement && node !== document.body; node = node.parentElement) {
        for (const sibling of node.parentElement.children) {
          if (sibling !== node && sibling instanceof HTMLElement && !sibling.querySelector('[role="dialog"]') && (sibling.matches('[role="main"], [data-testid="primaryColumn"]') || sibling.querySelector('[role="main"], [data-testid="primaryColumn"]'))) sibling.classList.add("bdeck-compose-background");
        }
      }
    }
    let content = editor?.parentElement;
    while (content && !content.querySelector('[data-testid="toolBar"]')) content = content.parentElement;
    const toolbar = content?.querySelector('[data-testid="toolBar"]');
    if (!(editor instanceof HTMLElement) || !(toolbar instanceof HTMLElement)) return;
    for (let node = toolbar.parentElement; node && node !== content; node = node.parentElement) {
      if (["absolute", "fixed", "sticky"].includes(getComputedStyle(node).position)) node.classList.add("bdeck-compose-tools");
    }
    if (!editor.getBoundingClientRect().width || !toolbar.getBoundingClientRect().height) return;
    if (toolbar.getBoundingClientRect().top < editor.getBoundingClientRect().bottom) return;
    if (editor !== readyElement) { readyElement = editor; send("composer.ready"); }
    const height = Math.max(160, Math.min(640, Math.ceil(toolbar.getBoundingClientRect().bottom + 12)));
    if (height !== lastHeight) { lastHeight = height; send(`composer.height=${height}`); }
  };
  let lastHeight = 0;
  let readyElement = null;
  let measuring = false;
  const scheduleMeasure = () => {
    if (measuring) return;
    measuring = true;
    requestAnimationFrame(() => { measuring = false; measure(); });
  };
  const ready = () => {
    const sizing = new ResizeObserver(scheduleMeasure);
    sizing.observe(document.body);
    new MutationObserver(scheduleMeasure).observe(document.documentElement, { subtree: true, childList: true, characterData: true });
    scheduleMeasure();
  };
  if (document.body) ready();
  else document.addEventListener("DOMContentLoaded", ready, { once: true });
})();
