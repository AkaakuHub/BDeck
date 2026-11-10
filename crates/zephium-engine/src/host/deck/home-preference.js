(storage) => {
  const key = "bdeck.home-timeline";
  const valid = value => value && typeof value === "object"
    && Object.keys(value).length === 2
    && typeof value.tab === "string" && value.tab.length > 0 && value.tab.length <= 256
    && !/[\u0000-\u001f\u007f]/.test(value.tab)
    && ["mostRecent", "popular"].includes(value.sort);
  let current = { tab: "react:home_latest", sort: "mostRecent" };
  const encoded = storage.getItem(key);
  if (encoded !== null) {
    try {
      if (encoded.length > 1024) throw new Error("oversized home timeline preference");
      const stored = JSON.parse(encoded);
      if (!valid(stored)) throw new Error("invalid home timeline preference");
      current = stored;
    } catch {
      storage.removeItem(key);
    }
  }
  return {
    get: () => current,
    save(tab, sort = current.sort) {
      const next = { tab, sort };
      if (!valid(next)) throw new Error("invalid home timeline preference");
      if (next.tab === current.tab && next.sort === current.sort) return;
      storage.setItem(key, JSON.stringify(next));
      current = next;
    },
  };
}
