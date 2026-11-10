(() => {
  const host = location.hostname.toLowerCase();
  if (!(host === "x.com" || host.endsWith(".x.com") || host === "twitter.com" || host.endsWith(".twitter.com")) || !location.pathname.startsWith("/compose/")) return;
  const editors = [...document.querySelectorAll('[data-testid="tweetTextarea_0"][contenteditable="true"]')];
  const editor = editors.find(node => node.closest('[role="dialog"]')) || editors.find(node => node.getClientRects().length && !node.closest('[aria-hidden="true"]'));
  if (!(editor instanceof HTMLElement)) return;
  if (mention) editor.dataset.bdeckMention = mention;
  if (editor.dataset.bdeckMention) {
    const cleanup = () => {
      editor.removeEventListener("focus", insert);
      window.removeEventListener("focus", insert);
    };
    const insert = () => {
      const recipient = editor.dataset.bdeckMention;
      if (!editor.isConnected || !recipient) { cleanup(); return; }
      if (!document.hasFocus() || document.activeElement !== editor) return;
      const prefix = `@${recipient} `;
      const text = editor.innerText || "";
      if (text.slice(0, prefix.length - 1).toLowerCase() === `@${recipient}`.toLowerCase() && (text.length === prefix.length - 1 || /\s/.test(text[prefix.length - 1]))) {
        delete editor.dataset.bdeckMention;
        cleanup();
        return;
      }
      const selection = window.getSelection();
      if (!selection) return;
      const range = document.createRange();
      range.selectNodeContents(editor);
      range.collapse(true);
      selection.removeAllRanges();
      selection.addRange(range);
      if (document.execCommand("insertText", false, prefix)) {
        delete editor.dataset.bdeckMention;
        cleanup();
      }
    };
    editor.addEventListener("focus", insert);
    window.addEventListener("focus", insert);
    editor.focus({ preventScroll: true });
    insert();
  } else {
    editor.focus({ preventScroll: true });
  }
})();
