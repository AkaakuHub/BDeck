(() => {
  const host = location.hostname.toLowerCase();
  if (window !== window.top || !(host === "x.com" || host === "twitter.com")) return;
  let reported = "";
  let identityRead = false;
  let timer = 0;
  let avatar = "";
  let identityPhoto = "";
  let photoNode = null;
  const photoObserver = new MutationObserver(() => reportAvatar());
  const reportAvatar = () => {
    const photo = document.querySelector('[data-testid="SideNav_AccountSwitcher_Button"] img');
    if (photo !== photoNode) {
      photoObserver.disconnect();
      photoNode = photo;
      if (photo) photoObserver.observe(photo, { attributes: true, attributeFilter: ["src"] });
    }
    const source = photo?.getAttribute("src") || "";
    const url = identityPhoto && identityPhoto.replace("_normal.", "_mini.") === source ? identityPhoto : source;
    if (!url || url.length > 1024) return;
    const encoded = JSON.stringify(url.startsWith("https://abs.twimg.com/sticky/default_profile_images/") ? null : url);
    if (encoded !== avatar) {
      avatar = encoded;
      window.ipc.postMessage(`account.avatar=${encoded}`);
    }
  };
  const readIdentity = () => {
    const prefix = "window.__INITIAL_STATE__=";
    const script = [...document.scripts].find(node => node.textContent.startsWith(prefix));
    if (!script || script.textContent.length > 2097152) return false;
    const end = script.textContent.indexOf(";window.__META_DATA__", prefix.length);
    if (end < 0) return false;
    try {
      const state = JSON.parse(script.textContent.slice(prefix.length, end));
      const id = state?.session?.user_id;
      const user = state?.entities?.users?.entities?.[id];
      if (typeof id !== "string" || !/^[1-9][0-9]{0,19}$/.test(id) || user?.id_str !== id) return false;
      if (typeof user.name !== "string" || typeof user.screen_name !== "string") return false;
      if (typeof user.profile_image_url_https === "string" && user.profile_image_url_https.length <= 1024) identityPhoto = user.profile_image_url_https;
      const identity = { user_id: id, display_name: user.name, screen_name: user.screen_name };
      const encoded = JSON.stringify(identity);
      if (encoded.length > 1024) return false;
      window.ipc.postMessage(`account.identity=${encoded}`);
      return true;
    } catch { return false; }
  };
  const inspect = () => {
    timer = 0;
    const accountSwitcher = document.querySelector('[data-testid="SideNav_AccountSwitcher_Button"]');
    const loggedOut = document.querySelector('[data-testid="loginButton"], [data-testid="signupButton"]')
      || (location.pathname.startsWith("/i/flow/login") && document.querySelector('input:not([type="hidden"])'));
    if (!identityRead && !loggedOut) identityRead = readIdentity();
    const loggedIn = accountSwitcher || (identityRead && !loggedOut);
    if (loggedOut && !accountSwitcher) identityRead = false;
    if (loggedIn) reportAvatar();
    const state = loggedIn ? "account.logged-in" : loggedOut ? "account.logged-out" : "";
    if (state && state !== reported) { reported = state; window.ipc.postMessage(state); }
  };
  const ready = () => {
    inspect();
    new MutationObserver(() => {
      if (!timer) timer = setTimeout(inspect, 250);
    }).observe(document.documentElement, { childList: true, subtree: true });
  };
  if (document.documentElement) ready();
  else document.addEventListener("DOMContentLoaded", ready, { once: true });
})();
