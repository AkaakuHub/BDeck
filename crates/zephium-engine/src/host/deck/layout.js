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
const applyPostSpacing = article => {
  const name = article.querySelector('[data-testid="User-Name"]');
  const avatar = article.querySelector('[data-testid^="UserAvatar-Container"]');
  let body = null;
  let context = null;
  if (name && avatar) {
    for (let row = name.parentElement; row && row !== article; row = row.parentElement) {
      if (!row.contains(avatar)) continue;
      body = name;
      while (body.parentElement && body.parentElement !== row) body = body.parentElement;
      const previous = row.previousElementSibling;
      if (previous && !previous.textContent.trim() && !previous.querySelector('img, svg, a, button, [role="button"]')) context = previous;
      break;
    }
  }
  article.querySelectorAll(".bdeck-post-body").forEach(node => {
    if (node !== body) node.classList.remove("bdeck-post-body");
  });
  article.querySelectorAll(".bdeck-empty-context").forEach(node => {
    if (node !== context) node.classList.remove("bdeck-empty-context");
  });
  if (body && !body.classList.contains("bdeck-post-body")) body.classList.add("bdeck-post-body");
  if (context && !context.classList.contains("bdeck-empty-context")) context.classList.add("bdeck-empty-context");
};
const apply = () => {
  document.documentElement.classList.toggle("bdeck-scaled-content", !location.pathname.startsWith("/compose/"));
  const style = document.getElementById("bdeck-column-style") || document.createElement("style");
  style.id = "bdeck-column-style";
  if (style.textContent !== columnCss) style.textContent = columnCss;
  if (!style.isConnected) (document.head || document.documentElement).appendChild(style);
  if (style.hasAttribute("data-chat-layout")) return;
  style.setAttribute("data-chat-layout", "");
  const homeSelection = createHomeSelection(createHomePreference);
  const pending = new Set();
  let frame = 0;
  let shellChanged = false;
  let refreshOnVisible = false;
  const refresh = () => {
    applyChat();
    applyHomeComposer();
    applyNotificationHeader();
    applyProfileHeader();
    homeSelection.refresh();
    document.querySelectorAll(`${primarySelector} ${tweetSelector}`).forEach(applyPostSpacing);
  };
  const flush = () => {
    frame = 0;
    if (shellChanged) {
      shellChanged = false;
      applyChat();
      applyHomeComposer();
      applyNotificationHeader();
      applyProfileHeader();
      homeSelection.refresh();
    }
    for (const article of pending) {
      if (article.isConnected && article.closest(primarySelector)) applyPostSpacing(article);
    }
    pending.clear();
  };
  new MutationObserver(records => {
    if (document.hidden) {
      refreshOnVisible = true;
      return;
    }
    for (const record of records) {
      const target = record.target instanceof Element ? record.target : record.target.parentElement;
      const article = target?.closest(tweetSelector);
      if (article && article.closest(primarySelector)) pending.add(article);
      else shellChanged = true;
      for (const node of record.addedNodes) {
        if (!(node instanceof Element) || !node.closest(primarySelector)) continue;
        if (node.matches(tweetSelector)) pending.add(node);
        node.querySelectorAll(tweetSelector).forEach(article => pending.add(article));
      }
    }
    if (!frame && (pending.size || shellChanged)) frame = requestAnimationFrame(flush);
  }).observe(document.documentElement, { childList: true, subtree: true });
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) {
      cancelAnimationFrame(frame);
      frame = 0;
      pending.clear();
      shellChanged = false;
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
