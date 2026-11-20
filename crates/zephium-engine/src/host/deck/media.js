(() => {
  if (window !== window.top || !/^(?:.*\.)?(?:x|twitter)\.com$/.test(location.hostname)) return;
  let active = null;
  let columnWidth = 0;
  let columnHeight = 0;
  let closeButton = null;
  let mediaActions = null;
  let pendingVideo = null;
  let swipeDistance = 0;
  let wheelBlockedUntil = 0;
  let swipeHandled = false;
  let gallery = null;
  let pointerEvents = null;
  let timelinePosition = null;
  let restoringPosition = null;
  let scrollRemainder = 0;
  const setPointerInside = (inside) =>
    document.documentElement.toggleAttribute("data-bdeck-media-pointer", inside);
  const restoreTimelinePosition = () => {
    const saved = restoringPosition;
    if (!saved || active || innerWidth !== columnWidth || innerHeight !== columnHeight) return;
    scrollTo({ left: saved.x, top: saved.y, behavior: "instant" });
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        if (restoringPosition !== saved || active) return;
        const link = [
          ...document.querySelectorAll('[data-testid="tweet"] a[href*="/status/"]:has(time)'),
        ].find((node) => node.href === saved.href);
        const tweet = link?.closest('[data-testid="tweet"]');
        if (tweet && typeof saved.top === "number") {
          const correction = tweet.getBoundingClientRect().top - saved.top + scrollRemainder;
          const pixels = Math.round(correction);
          scrollRemainder = correction - pixels;
          scrollBy({ top: pixels, behavior: "instant" });
        }
        restoringPosition = null;
      }),
    );
  };
  const stopPendingVideo = () => {
    pendingVideo?.disconnect();
    pendingVideo = null;
  };
  const restoreColumn = (resized = false) => {
    if (active || Math.abs(innerWidth - columnWidth) > 1) return;
    if (resized) columnHeight = innerHeight;
    document.documentElement.removeAttribute("data-bdeck-media");
    document.documentElement.style.removeProperty("--bdeck-media-column-width");
    restoreTimelinePosition();
  };
  const stopMediaClick = (event) => {
    event.stopPropagation();
    if (active?.video) return;
    event.preventDefault();
    const image = active?.image;
    if (!image?.naturalWidth || !image.naturalHeight) return;
    const bounds = active.element.getBoundingClientRect();
    const scale = Math.min(bounds.width / image.naturalWidth, bounds.height / image.naturalHeight);
    const width = image.naturalWidth * scale;
    const height = image.naturalHeight * scale;
    if (
      Math.abs(event.clientX - bounds.left - bounds.width / 2) > width / 2 ||
      Math.abs(event.clientY - bounds.top - bounds.height / 2) > height / 2
    )
      close();
  };
  const release = (element) => {
    element.removeEventListener("click", stopMediaClick);
    element.hidePopover();
    element.removeAttribute("popover");
    element.removeAttribute("data-bdeck-media-content");
    element.removeAttribute("data-bdeck-media-video");
  };
  const releaseActive = () => {
    const { element, video, controls, slot } = active;
    if (video) {
      slot.removeAttribute("data-bdeck-media-slot");
      slot.style.removeProperty("--bdeck-media-slot-height");
      video.pause();
      video.controls = controls;
    }
    release(element);
    if (!video) element.remove();
    active = null;
  };
  const close = () => {
    stopPendingVideo();
    if (!active) return;
    restoringPosition = timelinePosition;
    timelinePosition = null;
    releaseActive();
    window.removeEventListener("wheel", onMediaWheel, true);
    gallery?.dispose();
    gallery = null;
    closeButton.hidePopover();
    mediaActions.hide();
    pointerEvents.abort();
    pointerEvents = null;
    setPointerInside(false);
    window.ipc.postMessage("media.close");
    restoreColumn();
  };
  const present = (element) => {
    element.setAttribute("popover", "manual");
    element.setAttribute("data-bdeck-media-content", "");
    element.addEventListener("click", stopMediaClick);
    element.showPopover();
    gallery?.show();
    closeButton.hidePopover();
    closeButton.showPopover();
    mediaActions.show(active.video);
  };
  const play = (video) => {
    video.controls = true;
    video.play().catch(() => {});
  };
  const imageForPhoto = (photo) => {
    const image = photo.querySelector("img");
    const source = image?.currentSrc || image?.getAttribute("src");
    if (!source) return null;
    const url = new URL(source, document.baseURI);
    if (url.hostname === "pbs.twimg.com" && url.pathname.startsWith("/media/"))
      url.searchParams.set("name", "orig");
    return { imageUrl: url.href, alt: image.alt };
  };
  const selectImage = ({ imageUrl, alt }) => {
    const { element, image } = active;
    image.alt = alt;
    image.src = imageUrl;
    active = { element, image, imageUrl };
    mediaActions.show(false);
  };
  const open = (source, video, image) => {
    stopPendingVideo();
    pointerEvents = new AbortController();
    const options = { capture: true, signal: pointerEvents.signal };
    window.addEventListener("pointerover", showControls, options);
    window.addEventListener("pointermove", showControls, options);
    window.addEventListener("pointerout", hideControlsOnExit, options);
    window.addEventListener("blur", () => setPointerInside(false), options);
    window.addEventListener("wheel", onMediaWheel, { capture: true, passive: false });
    wheelBlockedUntil = 0;
    swipeHandled = false;
    swipeDistance = 0;
    restoringPosition = null;
    const tweet = source.closest('[data-testid="tweet"]');
    timelinePosition = {
      x: scrollX,
      y: scrollY,
      href: tweet?.querySelector('a[href*="/status/"]:has(time)')?.href,
      top: tweet?.getBoundingClientRect().top,
    };
    columnWidth = innerWidth;
    columnHeight = innerHeight;
    document.documentElement.style.setProperty("--bdeck-media-width", `${innerWidth}px`);
    document.documentElement.style.setProperty("--bdeck-media-height", `${innerHeight}px`);
    document.documentElement.style.setProperty("--bdeck-media-column-width", `${columnWidth}px`);
    document.documentElement.setAttribute("data-bdeck-media", video ? "video" : "image");
    gallery = video ? null : createMediaGallery(source, imageForPhoto, selectImage);
    if (!closeButton) {
      closeButton = createMediaButton("閉じる", "close");
      closeButton.setAttribute("popover", "manual");
      closeButton.setAttribute("data-bdeck-media-close", "");
      closeButton.addEventListener("click", close);
      document.body.appendChild(closeButton);
      mediaActions = createMediaActions(() => active);
    }
    if (video) {
      const slot = source.parentElement;
      slot.style.setProperty(
        "--bdeck-media-slot-height",
        `${slot.getBoundingClientRect().height}px`,
      );
      slot.setAttribute("data-bdeck-media-slot", "");
      source.setAttribute("data-bdeck-media-video", "");
      active = { element: source, video, slot, controls: video.controls };
    } else {
      const element = document.createElement("div");
      const expanded = document.createElement("img");
      expanded.alt = image.alt;
      expanded.src = image.imageUrl;
      element.appendChild(expanded);
      document.body.appendChild(element);
      active = { element, image: expanded, imageUrl: image.imageUrl };
    }
    present(active.element);
    window.ipc.postMessage("media.open");
    if (video) play(video);
  };
  const activate = (event) => {
    if (!event.isTrusted || event.button !== 0) return;
    const path = event.composedPath();
    const photo = path.find(
      (node) => node instanceof HTMLElement && node.matches('[data-testid="tweetPhoto"]'),
    );
    if (active) {
      if (active.image && photo) {
        event.preventDefault();
        event.stopImmediatePropagation();
      }
      return;
    }
    stopPendingVideo();
    const player = path.find(
      (node) => node instanceof HTMLElement && node.matches('[data-testid="videoPlayer"]'),
    );
    const video = player?.querySelector("video");
    if (photo?.querySelector('[data-testid="playButton"]') || (player && !video)) {
      const tweet = (photo || player).closest('[data-testid="tweet"]');
      if (!tweet) return;
      pendingVideo = new MutationObserver((records) => {
        for (const record of records)
          for (const node of record.addedNodes) {
            const added =
              node instanceof HTMLVideoElement
                ? node
                : node instanceof Element
                  ? node.querySelector("video")
                  : null;
            if (!(added instanceof HTMLVideoElement) || !added.isConnected) continue;
            setPointerInside(event.detail > 0);
            open(added.parentElement, added);
            return;
          }
      });
      pendingVideo.observe(tweet, { childList: true, subtree: true });
      return;
    }
    const source = video ? video.parentElement : photo;
    if (!(source instanceof HTMLElement)) return;
    const image = video ? null : imageForPhoto(source);
    if (!video && !image) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    setPointerInside(event.detail > 0);
    open(source, video, image);
  };
  window.addEventListener("click", activate, true);
  const showControls = (event) => {
    if (event.isTrusted && event.pointerType !== "touch") setPointerInside(true);
  };
  const hideControlsOnExit = (event) => {
    if (event.isTrusted && event.relatedTarget === null) setPointerInside(false);
  };
  window.addEventListener(
    "contextmenu",
    (event) => {
      if (!active) return;
      event.preventDefault();
      event.stopImmediatePropagation();
    },
    true,
  );
  window.addEventListener(
    "keydown",
    (event) => {
      if (!event.isTrusted || !active || document.fullscreenElement) return;
      const direction = event.key === "ArrowLeft" ? -1 : event.key === "ArrowRight" ? 1 : 0;
      if (event.key !== "Escape" && !(gallery && direction)) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      if (direction) gallery.move(direction);
      else close();
    },
    true,
  );
  const onMediaWheel = (event) => {
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
    if (!gallery?.move(Math.sign(swipeDistance))) close();
  };
  window.addEventListener("resize", () => {
    restoreColumn(true);
    if (active && (innerWidth !== columnWidth || innerHeight !== columnHeight)) {
      document.documentElement.style.setProperty("--bdeck-media-width", `${innerWidth}px`);
      document.documentElement.style.setProperty("--bdeck-media-height", `${innerHeight}px`);
    }
  });
})();
