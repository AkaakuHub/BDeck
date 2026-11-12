(() => {
  if (window !== window.top || !/^(?:.*\.)?(?:x|twitter)\.com$/.test(location.hostname)) return;
  let active = null;
  let columnWidth = 0;
  let closeButton = null;
  let consumeClick = false;
  let pendingVideo = null;
  let swipeDistance = 0;
  let wheelBlockedUntil = 0;
  let swipeHandled = false;
  let gallery = null;
  const stopPendingVideo = () => {
    pendingVideo?.disconnect();
    pendingVideo = null;
  };
  const restoreColumn = () => {
    if (active || Math.abs(innerWidth - columnWidth) > 1) return;
    document.documentElement.removeAttribute("data-bdeck-media");
    document.documentElement.style.removeProperty("--bdeck-media-column-width");
  };
  const stopMediaClick = event => {
    event.stopPropagation();
    if (active?.video) return;
    event.preventDefault();
    const image = active?.element.parentElement.querySelector("img");
    if (!image?.naturalWidth || !image.naturalHeight) return;
    const bounds = active.element.getBoundingClientRect();
    const scale = Math.min(bounds.width / image.naturalWidth, bounds.height / image.naturalHeight);
    const width = image.naturalWidth * scale;
    const height = image.naturalHeight * scale;
    if (Math.abs(event.clientX - bounds.left - bounds.width / 2) > width / 2 ||
        Math.abs(event.clientY - bounds.top - bounds.height / 2) > height / 2) close();
  };
  const release = element => {
    element.removeEventListener("click", stopMediaClick);
    element.hidePopover();
    element.removeAttribute("popover");
    element.removeAttribute("data-bdeck-media-content");
    element.removeAttribute("data-bdeck-media-video");
  };
  const releaseActive = () => {
    const { element, video, controls, slot } = active;
    slot.removeAttribute("data-bdeck-media-slot");
    slot.style.removeProperty("--bdeck-media-slot-height");
    if (video) {
      video.pause();
      video.controls = controls;
    }
    release(element);
    active = null;
  };
  const close = () => {
    stopPendingVideo();
    if (!active) return;
    releaseActive();
    gallery?.dispose();
    gallery = null;
    closeButton.hidePopover();
    window.ipc.postMessage("media.close");
    restoreColumn();
  };
  const present = element => {
    element.setAttribute("popover", "manual");
    element.setAttribute("data-bdeck-media-content", "");
    element.addEventListener("click", stopMediaClick);
    element.showPopover();
    gallery?.show();
    closeButton.hidePopover();
    closeButton.showPopover();
  };
  const play = video => {
    video.controls = true;
    video.play().catch(() => {});
  };
  const open = (element, video = null) => {
    stopPendingVideo();
    const opening = !active;
    if (opening) {
      swipeDistance = 0;
      columnWidth = innerWidth;
      document.documentElement.style.setProperty("--bdeck-media-column-width", `${columnWidth}px`);
      document.documentElement.setAttribute("data-bdeck-media", "");
      gallery = video ? null : createMediaGallery(element, open);
    } else {
      releaseActive();
    }
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
    active = { element, video, slot, controls: video?.controls };
    if (video) element.setAttribute("data-bdeck-media-video", "");
    present(element);
    if (opening) window.ipc.postMessage("media.open");
    if (video) play(video);
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
          if (!(added instanceof HTMLVideoElement) || !added.isConnected) continue;
          open(added.parentElement, added);
          return;
        }
      });
      pendingVideo.observe(tweet, { childList: true, subtree: true });
      return;
    }
    const element = video ? video.parentElement : photo?.querySelector('[style*="background-image"]');
    if (!(element instanceof HTMLElement)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    consumeClick = event.type === "pointerdown";
    open(element, video);
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
    if (active && event.composedPath().includes(active.element)) return;
    activate(event);
  }, true);
  window.addEventListener("keydown", event => {
    if (!event.isTrusted || !active || document.fullscreenElement) return;
    const direction = event.key === "ArrowLeft" ? -1 : event.key === "ArrowRight" ? 1 : 0;
    if (event.key !== "Escape" && !(gallery && direction)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    if (direction) gallery.move(direction);
    else close();
  }, true);
  window.addEventListener("wheel", event => {
    if (!active && event.timeStamp >= wheelBlockedUntil) return;
    event.preventDefault();
    if (event.timeStamp >= wheelBlockedUntil) {
      swipeDistance = 0;
      swipeHandled = false;
    }
    wheelBlockedUntil = event.timeStamp + 180;
    if (!active || swipeHandled || Math.abs(event.deltaX) <= Math.abs(event.deltaY)) return;
    swipeDistance += event.deltaX;
    if (Math.abs(swipeDistance) < 60) return;
    swipeHandled = true;
    if (gallery) gallery.move(Math.sign(swipeDistance));
    else close();
  }, { capture: true, passive: false });
  window.addEventListener("resize", restoreColumn);
})();
