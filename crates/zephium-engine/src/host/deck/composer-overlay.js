(dismiss = false) => {
  const overlays = [...document.querySelectorAll('[data-testid="hoverCardParent"], [role="menu"], [role="listbox"]')]
    .filter(node => node.getClientRects().length && getComputedStyle(node).visibility !== "hidden");
  if (dismiss) {
    for (const overlay of overlays) {
      if (overlay.matches('[data-testid="hoverCardParent"]')) {
        const backdrop = overlay.previousElementSibling;
        if (backdrop instanceof HTMLElement) backdrop.click();
        continue;
      }
      for (const type of ["keydown", "keyup"]) {
        overlay.dispatchEvent(new KeyboardEvent(type, {
          key: "Escape", code: "Escape", keyCode: 27, which: 27, bubbles: true, cancelable: true,
        }));
      }
    }
  }
  return overlays.length > 0;
}
