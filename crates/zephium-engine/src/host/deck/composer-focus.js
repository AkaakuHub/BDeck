(() => {
  const host = location.hostname.toLowerCase();
  if (!(host === "x.com" || host.endsWith(".x.com") || host === "twitter.com" || host.endsWith(".twitter.com")) || !location.pathname.startsWith("/compose/")) return;
  const editors = [...document.querySelectorAll('[data-testid="tweetTextarea_0"][contenteditable="true"]')];
  const editor = editors.find(node => node.closest('[role="dialog"]')) || editors.find(node => node.getClientRects().length && !node.closest('[aria-hidden="true"]'));
  if (editor instanceof HTMLElement) editor.focus({ preventScroll: true });
})();
