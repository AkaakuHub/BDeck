(() => {
  if (location.pathname.startsWith("/compose/")) return false;
  let node = document.elementFromPoint(innerWidth * pointX, innerHeight * pointY);
  if (!node || node.closest("input, textarea, select, [contenteditable]")) return false;
  for (; node; node = node.parentElement) {
    if (node.scrollWidth > node.clientWidth + 1 && /^(auto|scroll|overlay)$/.test(getComputedStyle(node).overflowX)) return false;
  }
  return true;
})();
