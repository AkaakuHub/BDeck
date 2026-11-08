export function layoutAnchor(trigger: HTMLElement | null | undefined) {
  if (!trigger) return null;
  return {
    contextElement: trigger,
    getBoundingClientRect(): DOMRect {
      const rect = trigger.getBoundingClientRect();
      const scale = parseFloat(getComputedStyle(trigger).scale);
      if (!scale || scale === 1) return rect;
      const width = rect.width / scale;
      const height = rect.height / scale;
      return new DOMRect(
        rect.x + (rect.width - width) / 2,
        rect.y + (rect.height - height) / 2,
        width,
        height,
      );
    },
  };
}
