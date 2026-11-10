(createPreference) => {
  const preference = createPreference(localStorage);
  let nav = null;
  let restored = false;
  let sorting = false;
  let followingSortPending = false;
  let sortMenuArmed = false;
  let deadline = 0;
  const tabKey = tab => {
    if (tab.closest("#cpftSeparatedTweetsTab")) return "cpft:separated";
    const property = Object.keys(tab).find(key => key.startsWith("__reactFiber$"));
    let fiber = property && tab[property];
    while (fiber && fiber.stateNode !== nav) {
      if (typeof fiber.memoizedProps?.isActive === "boolean" && typeof fiber.key === "string" && fiber.key) return `react:${fiber.key}`;
      fiber = fiber.return;
    }
    return null;
  };
  const finishSort = () => {
    clearTimeout(deadline);
    deadline = 0;
    sorting = false;
    document.documentElement.removeAttribute("data-bdeck-restoring-sort");
  };
  const configureExtension = () => {
    const settings = document.getElementById("cpftSettings");
    if (!settings?.textContent) return;
    let config;
    try { config = JSON.parse(settings.textContent); } catch { return; }
    if (config.alwaysUseLatestTweets === false && config.sortFollowing === preference.get().sort) return;
    settings.textContent = JSON.stringify({ ...config, alwaysUseLatestTweets: false, sortFollowing: preference.get().sort });
  };
  const sortItems = () => [...document.querySelectorAll('[role="menu"] [data-testid="Dropdown"] [role="menuitem"]')];
  const refresh = () => {
    configureExtension();
    if (location.pathname !== "/home") {
      nav = null;
      restored = false;
      followingSortPending = false;
      finishSort();
      return;
    }
    const currentNav = document.querySelector('[data-testid="primaryColumn"] nav:has([role="tab"])');
    if (currentNav !== nav) {
      nav = currentNav;
      restored = false;
    }
    if (!nav) return;
    if (sorting) {
      const items = sortItems();
      if (items.length === 2) {
        items[preference.get().sort === "popular" ? 0 : 1].click();
        finishSort();
      }
      return;
    }
    if (!restored) {
      const tab = [...nav.querySelectorAll('[role="tab"]')].find(tab => tabKey(tab) === preference.get().tab);
      if (!tab) return;
      restored = true;
      followingSortPending = preference.get().tab === "react:home_latest";
      if (tab.getAttribute("aria-selected") !== "true") tab.click();
    }
    if (!followingSortPending) return;
    const tab = nav.querySelector('[role="tab"][aria-selected="true"]');
    if (!tab || tabKey(tab) !== "react:home_latest") return;
    followingSortPending = false;
    if (tab.getAttribute("aria-haspopup") === "menu") {
      sorting = true;
      document.documentElement.setAttribute("data-bdeck-restoring-sort", "");
      deadline = setTimeout(finishSort, 1000);
      tab.click();
    }
  };
  document.addEventListener("click", event => {
    if (!event.isTrusted || location.pathname !== "/home" || !(event.target instanceof Element)) return;
    if (sorting) finishSort();
    followingSortPending = false;
    const tab = event.target.closest('[role="tab"]');
    if (tab && nav?.contains(tab)) {
      const key = tabKey(tab);
      if (!key || tab.getAttribute("aria-disabled") === "true") return;
      preference.save(key);
      restored = true;
      sortMenuArmed = key === "react:home_latest";
      configureExtension();
      return;
    }
    const item = event.target.closest('[role="menuitem"]');
    const items = sortMenuArmed && item ? sortItems() : [];
    if (items.length === 2 && items.includes(item)) {
      preference.save(preference.get().tab, items.indexOf(item) === 0 ? "popular" : "mostRecent");
      configureExtension();
    }
    sortMenuArmed = false;
  }, true);
  return { refresh };
}
