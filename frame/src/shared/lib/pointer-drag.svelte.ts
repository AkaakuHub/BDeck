export type DragPosition = { x: number; y: number };

export function createPointerDrag<T>(options: {
  ondrop: (item: T, target: Element | null, at: DragPosition) => void;
}) {
  let carrying = $state<T | null>(null);
  let at = $state<DragPosition>({ x: 0, y: 0 });
  let origin: DragPosition | null = null;
  let pending: T | null = null;
  let queued: DragPosition = { x: 0, y: 0 };
  let frame = 0;
  let consumed = false;

  function reset() {
    if (frame !== 0) cancelAnimationFrame(frame);
    frame = 0;
    origin = null;
    pending = null;
    carrying = null;
  }

  function release(event: PointerEvent) {
    const target = event.currentTarget;
    if (target instanceof HTMLElement && target.hasPointerCapture(event.pointerId))
      target.releasePointerCapture(event.pointerId);
  }

  return {
    get item() { return carrying; },
    get at() { return at; },
    absorbClick() {
      if (!consumed) return false;
      consumed = false;
      return true;
    },
    begin(event: PointerEvent, item: T) {
      if (event.button !== 0) return;
      consumed = false;
      origin = { x: event.clientX, y: event.clientY };
      at = origin;
      pending = item;
    },
    move(event: PointerEvent) {
      if (origin === null || pending === null) return;
      if (carrying === null && Math.abs(event.clientX - origin.x) + Math.abs(event.clientY - origin.y) <= 4) return;
      if (carrying === null) {
        const target = event.currentTarget;
        if (target instanceof HTMLElement) target.setPointerCapture(event.pointerId);
        carrying = pending;
      }
      queued = { x: event.clientX, y: event.clientY };
      if (frame !== 0) return;
      frame = requestAnimationFrame(() => { frame = 0; at = queued; });
    },
    end(event: PointerEvent) {
      const item = carrying;
      release(event);
      reset();
      if (item === null) return;
      consumed = true;
      options.ondrop(item, document.elementFromPoint(event.clientX, event.clientY), {
        x: event.clientX, y: event.clientY,
      });
    },
    cancel(event?: PointerEvent) {
      if (event) release(event);
      reset();
    },
  };
}
