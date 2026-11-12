(() => {
  if (window !== window.top || !/^(?:.*\.)?(?:x|twitter)\.com$/.test(location.hostname)) return;
  let active = null;
  let columnWidth = 0;
  let closeButton = null;
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
  const open = (element, player) => {
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
    active = { element, player, slot, controls: element instanceof HTMLVideoElement && element.controls };
    present(element);
    window.ipc.postMessage("media.open");
    if (element instanceof HTMLVideoElement) play(element);
    else if (player) player.querySelector('[data-testid="playButton"], [aria-label="この動画を再生する"], [aria-label="Play Video"]')?.click();
  };
  const activate = event => {
    if (!event.isTrusted || active || (event.type === "pointerdown" && event.button !== 0)) return;
    const path = event.composedPath();
    const player = path.find(node => node instanceof HTMLElement && node.matches('[data-testid="videoPlayer"]'));
    const photo = path.find(node => node instanceof HTMLElement && node.matches('[data-testid="tweetPhoto"]'));
    const element = player?.querySelector("video") || player || photo?.querySelector('[style*="background-image"]');
    if (!(element instanceof HTMLElement)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    open(element, player);
  };
  window.addEventListener("pointerdown", activate, true);
  window.addEventListener("click", event => {
    if (!event.isTrusted) return;
    if (active && event.composedPath().includes(active.element)) {
      event.stopImmediatePropagation();
      if (!(active.element instanceof HTMLVideoElement)) event.preventDefault();
      return;
    }
    activate(event);
  }, true);
  document.addEventListener("canplay", event => {
    const video = event.target;
    if (!(video instanceof HTMLVideoElement) || !active?.player?.contains(video)) return;
    if (active.element === video) return;
    release(active.element);
    active.controls = video.controls;
    active.element = video;
    present(video);
    play(video);
  }, true);
  window.addEventListener("keydown", event => {
    if (!event.isTrusted || !active || event.key !== "Escape") return;
    event.preventDefault();
    event.stopImmediatePropagation();
    close();
  }, true);
  window.addEventListener("resize", restoreColumn);
})();
