(() => {
  if (window !== window.top || !/^(?:.*\.)?(?:x|twitter)\.com$/.test(location.hostname)) return;
  let active = null;
  let columnWidth = 0;
  let closeButton = null;
  let consumeClick = false;
  let pendingVideo = null;
  const stopPendingVideo = () => {
    pendingVideo?.disconnect();
    pendingVideo = null;
  };
  const restoreColumn = () => {
    if (active || Math.abs(innerWidth - columnWidth) > 1) return;
    document.documentElement.removeAttribute("data-bdeck-media");
    document.documentElement.style.removeProperty("--bdeck-media-column-width");
  };
  const release = element => {
    element.hidePopover();
    element.removeAttribute("popover");
    element.removeAttribute("data-bdeck-media-content");
  };
  const close = () => {
    stopPendingVideo();
    if (!active) return;
    const { element, controls, slot } = active;
    slot.removeAttribute("data-bdeck-media-slot");
    slot.style.removeProperty("--bdeck-media-slot-height");
    if (element instanceof HTMLVideoElement) {
      element.pause();
      element.controls = controls;
    }
    release(element);
    closeButton.hidePopover();
    active = null;
    window.ipc.postMessage("media.close");
    restoreColumn();
  };
  const present = element => {
    element.setAttribute("popover", "manual");
    element.setAttribute("data-bdeck-media-content", "");
    element.showPopover();
    closeButton.showPopover();
  };
  const play = video => {
    video.controls = true;
    video.play().catch(() => {});
  };
  const open = element => {
    stopPendingVideo();
    columnWidth = innerWidth;
    document.documentElement.style.setProperty("--bdeck-media-column-width", `${columnWidth}px`);
    document.documentElement.setAttribute("data-bdeck-media", "");
    if (!closeButton) {
      closeButton = document.createElement("button");
      closeButton.type = "button";
      closeButton.textContent = "×";
      closeButton.setAttribute("aria-label", "閉じる");
      closeButton.setAttribute("popover", "manual");
      closeButton.setAttribute("data-bdeck-media-close", "");
      closeButton.addEventListener("click", close);
      document.body.appendChild(closeButton);
    }
    const slot = element.parentElement;
    slot.style.setProperty("--bdeck-media-slot-height", `${slot.getBoundingClientRect().height}px`);
    slot.setAttribute("data-bdeck-media-slot", "");
    active = { element, slot, controls: element instanceof HTMLVideoElement && element.controls };
    present(element);
    window.ipc.postMessage("media.open");
    if (element instanceof HTMLVideoElement) play(element);
  };
  const activate = event => {
    if (!event.isTrusted || active || (event.type === "pointerdown" && event.button !== 0)) return;
    const path = event.composedPath();
    const player = path.find(node => node instanceof HTMLElement && node.matches('[data-testid="videoPlayer"]'));
    const photo = path.find(node => node instanceof HTMLElement && node.matches('[data-testid="tweetPhoto"]'));
    const video = player?.querySelector("video");
    if (photo?.querySelector('[data-testid="playButton"]') || (player && !video)) {
      const tweet = (photo || player).closest('[data-testid="tweet"]');
      if (!tweet) return;
      stopPendingVideo();
      pendingVideo = new MutationObserver(records => {
        for (const record of records) for (const node of record.addedNodes) {
          const added = node instanceof HTMLVideoElement ? node : node instanceof Element ? node.querySelector("video") : null;
          if (!(added instanceof HTMLVideoElement)) continue;
          open(added);
          return;
        }
      });
      pendingVideo.observe(tweet, { childList: true, subtree: true });
      return;
    }
    const element = video || photo?.querySelector('[style*="background-image"]');
    if (!(element instanceof HTMLElement)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    consumeClick = event.type === "pointerdown";
    open(element);
  };
  window.addEventListener("pointerdown", event => {
    consumeClick = false;
    stopPendingVideo();
    activate(event);
  }, true);
  window.addEventListener("click", event => {
    if (!event.isTrusted) return;
    if (consumeClick) {
      consumeClick = false;
      event.preventDefault();
      event.stopImmediatePropagation();
      return;
    }
    if (active && event.composedPath().includes(active.element)) {
      event.stopImmediatePropagation();
      if (!(active.element instanceof HTMLVideoElement)) event.preventDefault();
      return;
    }
    activate(event);
  }, true);
  window.addEventListener("keydown", event => {
    if (!event.isTrusted || event.key !== "Escape") return;
    stopPendingVideo();
    if (!active) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    close();
  }, true);
  window.addEventListener("resize", restoreColumn);
})();
