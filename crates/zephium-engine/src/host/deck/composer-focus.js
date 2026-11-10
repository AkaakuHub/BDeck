(() => {
  const host = location.hostname.toLowerCase();
  if (!(host === "x.com" || host.endsWith(".x.com") || host === "twitter.com" || host.endsWith(".twitter.com")) || !location.pathname.startsWith("/compose/")) return;
  const editors = [...document.querySelectorAll('[data-testid="tweetTextarea_0"][contenteditable="true"]')];
  const editor = editors.find(node => node.closest('[role="dialog"]')) || editors.find(node => node.getClientRects().length && !node.closest('[aria-hidden="true"]'));
  if (!(editor instanceof HTMLElement)) return;
  if (mention !== undefined) editor.dataset.bdeckMention = mention || "";
  if (Object.hasOwn(editor.dataset, "bdeckMention")) {
    const cleanup = () => {
      editor.removeEventListener("focus", update);
      window.removeEventListener("focus", update);
    };
    const update = () => {
      if (!editor.isConnected || !Object.hasOwn(editor.dataset, "bdeckMention")) { cleanup(); return; }
      if (!document.hasFocus()) return;
      composerOverlay(true);
      if (document.activeElement !== editor) return;
      const recipient = editor.dataset.bdeckMention;
      const previous = editor.dataset.bdeckInsertedMention;
      const text = editor.textContent || "";
      const prefixLength = (name, value) => {
        if (!name) return 0;
        const prefix = `@${name}`;
        if (value.slice(0, prefix.length).toLowerCase() !== prefix.toLowerCase()) return 0;
        if (value.length === prefix.length) return prefix.length;
        return /\s/.test(value[prefix.length]) ? prefix.length + 1 : 0;
      };
      const remove = prefixLength(previous, text);
      if (remove && recipient.toLowerCase() === previous.toLowerCase()) {
        delete editor.dataset.bdeckMention;
        cleanup();
        return;
      }
      const insert = recipient && !prefixLength(recipient, text.slice(remove)) ? `@${recipient} ` : "";
      if (remove || insert) {
        const selection = window.getSelection();
        if (!selection) return;
        const range = document.createRange();
        range.selectNodeContents(editor);
        range.collapse(true);
        if (remove) {
          const walker = document.createTreeWalker(editor, NodeFilter.SHOW_TEXT);
          let remaining = remove;
          let node = walker.nextNode();
          while (node && remaining > node.textContent.length) {
            remaining -= node.textContent.length;
            node = walker.nextNode();
          }
          if (!node) return;
          range.setEnd(node, remaining);
        }
        selection.removeAllRanges();
        selection.addRange(range);
        if (!document.execCommand("insertText", false, insert)) return;
      }
      if (insert) editor.dataset.bdeckInsertedMention = recipient;
      else delete editor.dataset.bdeckInsertedMention;
      delete editor.dataset.bdeckMention;
      cleanup();
    };
    editor.addEventListener("focus", update);
    window.addEventListener("focus", update);
    editor.focus({ preventScroll: true });
    update();
  } else {
    editor.focus({ preventScroll: true });
  }
})();
