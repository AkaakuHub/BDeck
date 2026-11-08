(() => {
  const host = location.hostname.toLowerCase();
  if (!(host === "x.com" || host.endsWith(".x.com") || host === "twitter.com" || host.endsWith(".twitter.com")) || !location.pathname.startsWith("/compose/")) return;
  const editor = document.querySelector('[data-testid="tweetTextarea_0"][contenteditable="true"]');
  if (editor instanceof HTMLElement) editor.focus({ preventScroll: true });
})();
