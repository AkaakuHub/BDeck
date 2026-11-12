(() => {
  let timeline = null;
  let frame = 0;
  const isMedia = () => /\/(photo|video)(?:\/\d+)?$/.test(location.pathname);
  const preserve = () => {
    if (timeline || isMedia()) return;
    const main = document.querySelector('[role="main"]:has([data-testid="primaryColumn"])');
    if (!(main instanceof HTMLElement)) return;
    timeline = { width: innerWidth, x: scrollX, y: scrollY };
    document.documentElement.style.setProperty("--bdeck-timeline-width", `${main.getBoundingClientRect().width}px`);
    document.documentElement.setAttribute("data-bdeck-media", "");
  };
  const restore = () => {
    if (!timeline || isMedia() || innerWidth > timeline.width + 1 || frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      if (!timeline || isMedia() || innerWidth > timeline.width + 1) return;
      const { x, y } = timeline;
      timeline = null;
      document.documentElement.removeAttribute("data-bdeck-media");
      document.documentElement.style.removeProperty("--bdeck-timeline-width");
      scrollTo(x, y);
    });
  };
  document.addEventListener("click", event => {
    if (!event.isTrusted) return;
    const link = event.composedPath().find(node => node instanceof HTMLAnchorElement);
    if (link && /\/(photo|video)(?:\/\d+)?$/.test(new URL(link.href).pathname)) preserve();
  }, true);
  window.addEventListener("popstate", restore);
  window.addEventListener("resize", restore);
  return preserve;
})()
