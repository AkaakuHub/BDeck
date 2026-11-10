(dismiss = false) => {
  const open = [...document.querySelectorAll('[role="menu"], [role="listbox"], [role="radio"], input[type="radio"]')]
    .some(node => node.getClientRects().length && getComputedStyle(node).visibility !== "hidden");
  if (open && dismiss) {
    for (const type of ["keydown", "keyup"]) {
      (document.activeElement || document.body).dispatchEvent(new KeyboardEvent(type, {
        key: "Escape", code: "Escape", keyCode: 27, which: 27, bubbles: true, cancelable: true,
      }));
    }
  }
  return open;
}
