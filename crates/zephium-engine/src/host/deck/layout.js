const primarySelector = '[data-testid="primaryColumn"]';
const tweetSelector = '[data-testid="tweet"]';
const applyProfileHeader = () => {
  const primary = document.querySelector(primarySelector);
  const profile = !!primary?.querySelector('[data-testid="UserName"]');
  document.documentElement.toggleAttribute("data-bdeck-profile", profile);
  if (!profile) return;
  const heading = primary.querySelector("h2");
  const back = primary.querySelector('[data-testid="app-bar-back"]');
  if (!heading || !back) return;
  let row = heading.parentElement;
  while (row && row !== primary && !row.contains(back)) row = row.parentElement;
  if (!row || row === primary) return;
  for (let node = row; node && node !== primary && !node.querySelector('[data-testid="UserName"]'); node = node.parentElement) {
    if (!node.hasAttribute("data-bdeck-profile-header")) node.setAttribute("data-bdeck-profile-header", "");
  }
  if (back.parentElement && !back.parentElement.hasAttribute("data-bdeck-profile-back")) back.parentElement.setAttribute("data-bdeck-profile-back", "");
};
const applyNotificationHeader = () => {
  const notifications = location.pathname === "/notifications" || location.pathname.startsWith("/notifications/");
  document.documentElement.toggleAttribute("data-bdeck-notifications", notifications);
  if (!notifications) return;
  const primary = document.querySelector(primarySelector);
  const heading = primary?.querySelector("h2");
  const settings = primary?.querySelector('a[href="/settings/notifications"]');
  if (!heading || !settings) return;
  let row = heading.parentElement;
  while (row && row !== primary && !row.contains(settings)) row = row.parentElement;
  if (!row || row === primary) return;
  for (let node = row; node && node !== primary && !node.querySelector("nav"); node = node.parentElement) {
    if (!node.hasAttribute("data-bdeck-notification-header")) node.setAttribute("data-bdeck-notification-header", "");
  }
  if (!heading.hasAttribute("data-bdeck-notification-title")) heading.setAttribute("data-bdeck-notification-title", "");
};
const applyChat = () => {
  const root = document.querySelector('[data-testid="xchatEmbedRoute"]')?.shadowRoot;
  if (!root || root.getElementById("bdeck-chat-style")) return;
  const style = document.createElement("style");
  style.id = "bdeck-chat-style";
  style.textContent = chatCss;
  root.appendChild(style);
};
let homeComposer = null;
const applyHomeComposer = () => {
  const home = location.pathname === "/home";
  document.documentElement.toggleAttribute("data-bdeck-home", home);
  const primary = document.querySelector(primarySelector);
  const button = home && primary?.querySelector('[data-testid="tweetButtonInline"]');
  const editor = home && primary?.querySelector('[data-testid="tweetTextarea_0"]');
  let composer = null;
  if (button && editor) {
    for (let node = button.parentElement; node && node !== primary; node = node.parentElement) {
      if (node.querySelector(tweetSelector)) break;
      if (node.contains(editor) && node.querySelector('[data-testid^="UserAvatar-Container"]')) {
        composer = node;
        break;
      }
    }
  }
  if (homeComposer !== composer) homeComposer?.classList.remove("bdeck-home-composer");
  if (composer && !composer.classList.contains("bdeck-home-composer")) composer.classList.add("bdeck-home-composer");
  homeComposer = composer;
};
const apply = () => {
  const style = document.getElementById("bdeck-column-style") || document.createElement("style");
  style.id = "bdeck-column-style";
  if (style.textContent !== columnCss) style.textContent = columnCss;
  if (!style.isConnected) (document.head || document.documentElement).appendChild(style);
  if (style.hasAttribute("data-chat-layout")) return;
  style.setAttribute("data-chat-layout", "");
  const homeSelection = createHomeSelection(createHomePreference);
  let frame = 0;
  let refreshOnVisible = false;
  let layers = null;
  const schedule = () => {
    if (document.hidden) {
      refreshOnVisible = true;
      return;
    }
    if (!frame) frame = requestAnimationFrame(refresh);
  };
  const scaffold = new MutationObserver(schedule);
  const header = new MutationObserver(schedule);
  const bootstrap = new MutationObserver(schedule);
  let overlay = false;
  const refreshOverlay = () => {
    const active = !!layers?.querySelector('[role="dialog"]:not(:has([data-testid="tweetTextarea_0"]))');
    if (active === overlay) return;
    overlay = active;
    window.ipc.postMessage(active ? "overlay.open" : "overlay.close");
  };
  const menu = new MutationObserver(() => {
    homeSelection.refreshMenu();
    refreshOverlay();
  });
  const refresh = () => {
    frame = 0;
    const primary = document.querySelector(primarySelector);
    const content = primary || document.querySelector('[data-testid="xchatEmbedRoute"]');
    const currentLayers = document.getElementById("layers");
    if (currentLayers !== layers) {
      menu.disconnect();
      layers = currentLayers;
      if (layers) menu.observe(layers, { childList: true, subtree: true });
    }
    refreshOverlay();
    scaffold.disconnect();
    header.disconnect();
    if (!content) {
      bootstrap.observe(document.documentElement, { childList: true, subtree: true });
      return;
    }
    bootstrap.disconnect();
    for (let node = content; node instanceof Element; node = node.parentElement) {
      scaffold.observe(node, { childList: true });
    }
    let child = content.firstElementChild;
    for (let depth = 0; child && depth < 3; depth++, child = child.firstElementChild) {
      if (child.closest('section[role="region"], [data-testid="cellInnerDiv"], [data-testid="tweet"]')) break;
      scaffold.observe(child, { childList: true });
    }
    applyChat();
    applyHomeComposer();
    applyNotificationHeader();
    applyProfileHeader();
    homeSelection.refresh();
    const nav = primary?.querySelector('nav:has([role="tab"])');
    if (nav) header.observe(nav, { childList: true, subtree: true, attributes: true, attributeFilter: ["aria-selected", "aria-haspopup"] });
    primary?.querySelectorAll("[data-bdeck-profile-header], [data-bdeck-notification-header]").forEach(node => {
      header.observe(node, { childList: true, subtree: true });
    });
    if (homeComposer) header.observe(homeComposer, { childList: true, subtree: true });
    homeSelection.refreshMenu();
    refreshOverlay();
  };
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) {
      cancelAnimationFrame(frame);
      frame = 0;
      refreshOnVisible = true;
    } else if (refreshOnVisible) {
      refreshOnVisible = false;
      refresh();
    }
  });
  refresh();
};
if (document.documentElement) apply();
else {
  const rootObserver = new MutationObserver(() => {
    if (!document.documentElement) return;
    rootObserver.disconnect();
    apply();
  });
  rootObserver.observe(document, { childList: true });
}
