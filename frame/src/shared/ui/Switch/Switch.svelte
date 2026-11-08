<script lang="ts">
  import { untrack } from "svelte";
  import { Spring } from "svelte/motion";
  let {
    label,
    checked = false,
    disabled = false,
    onchange,
  }: {
    label: string;
    checked?: boolean;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
  } = $props();

  const track = { width: 45, height: 20, padding: 2 };
  const thumb = { width: 26, height: 16, stretchX: 6, stretchY: 0 };
  const travel = track.width - thumb.width - track.padding * 2;

  const position = new Spring(untrack(() => checked) ? travel : 0, { stiffness: 0.18, damping: 0.82 });
  const grab = new Spring(0, { stiffness: 0.16, damping: 0.75 });

  let pointer: number | null = null;
  let startX = 0;
  let startPosition = 0;
  let dragging = false;
  let suppressClick = false;

  const reduce = () =>
    window.matchMedia("(prefers-reduced-motion: reduce)").matches ||
    document.documentElement.dataset.reduceMotion === "true";

  $effect(() => {
    if (pointer === null) void position.set(checked ? travel : 0, { instant: reduce() });
  });

  let progress = $derived(Math.min(1, Math.max(0, position.current / travel)));
  let width = $derived(thumb.width + thumb.stretchX * grab.current);
  let height = $derived(thumb.height + thumb.stretchY * grab.current);

  let offset = $derived(
    Math.min(
      Math.max(0, position.current - (width - thumb.width) / 2),
      track.width - track.padding * 2 - width,
    ),
  );

  function commit(next: boolean) {
    if (next !== checked) {
      onchange?.(next);
    }
    void position.set(checked ? travel : 0, { instant: reduce() });
  }
  function down(event: PointerEvent) {
    if (disabled || (event.pointerType === "mouse" && event.button !== 0)) return;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    pointer = event.pointerId;
    startX = event.clientX;
    startPosition = position.current;
    dragging = false;
    void grab.set(1);
  }
  function move(event: PointerEvent) {
    if (pointer !== event.pointerId) return;
    const delta = event.clientX - startX;
    if (Math.abs(delta) > 3) dragging = true;
    if (!dragging) return;
    event.preventDefault();
    const rtl = getComputedStyle(event.currentTarget as HTMLElement).direction === "rtl";
    void position.set(Math.min(travel, Math.max(0, startPosition + (rtl ? -delta : delta))));
  }
  function up(event: PointerEvent) {
    if (pointer !== event.pointerId) return;
    pointer = null;
    void grab.set(0);
    if (!dragging) return;
    dragging = false;
    suppressClick = true;
    commit(position.target >= travel / 2);
  }
  function cancel() {
    pointer = null;
    dragging = false;
    void grab.set(0);
    void position.set(checked ? travel : 0);
  }
  function click() {
    if (disabled) return;
    if (suppressClick) {
      suppressClick = false;
      return;
    }
    commit(!checked);
  }
</script>

  <button
    type="button"
    class="switch"
    role="switch"
    aria-checked={checked}
    aria-label={label}
    {disabled}
    data-held={grab.current > 0.05}
    style:--progress={progress}
    style:width={`${track.width}px`}
    style:height={`${track.height}px`}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={cancel}
    onlostpointercapture={cancel}
    onclick={click}
  >
    <span class="on" aria-hidden="true"></span>
    <span
      class="thumb"
      aria-hidden="true"
      style:--held={grab.current}
      style:width={`${width}px`}
      style:height={`${height}px`}
      style:translate={`${offset}px 0`}
      style:inset-inline-start={`${track.padding}px`}
    ></span>
  </button>
<style>
  
  .switch {
    position: relative;
    flex: none;
    box-sizing: border-box;
    padding: 0;
    border: 0;
    border-radius: var(--radius-capsule);
    background: var(--color-control);
    box-shadow: inset 0 0 0 1px var(--color-border-strong);
    cursor: default;
    touch-action: pan-y;
    
    user-select: none;
    contain: layout;
    overflow: hidden;
    transition: background-color var(--motion-fast) var(--ease-smooth);
  }

  .switch:disabled {
    opacity: 0.5;
  }

  .switch:focus-visible {
    outline: 2px solid var(--color-compose);
    outline-offset: 3px;
  }

  .switch:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-control), var(--color-text) 5%);
  }

  .on {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--color-compose);
    opacity: var(--progress);
  }

  
  .thumb {
    position: absolute;
    top: 50%;
    border-radius: var(--radius-capsule);
    background: color-mix(
      in srgb,
      #fff calc(100% - var(--held) * 62%),
      rgb(255 255 255 / 0.6)
    );

    --rim: color-mix(in srgb, #fff calc(var(--held) * 100%), transparent);
    --glow: color-mix(in srgb, #fff calc(var(--held) * 40%), transparent);

    box-shadow:
      0 1px 3px rgb(0 0 0 / 0.25),
      inset 0 0 0 1px var(--rim),
      inset 0 0 3px 1px var(--glow),
      0 0 0 0.5px var(--glow);
    transform: translateY(-50%);
  }

  .switch[data-held="true"] .thumb {
    will-change: translate, width, height;
  }

  .switch[aria-checked="false"] .thumb {
    background: color-mix(
      in srgb,
      #fff calc(100% - var(--held) * 62%),
      rgb(255 255 255 / 0.6)
    );
  }

  @media (forced-colors: active) {
    .switch {
      border: 1px solid ButtonText;
    }

    .on {
      background: Highlight;
    }

    .thumb {
      background: ButtonText;
    }
  }
</style>
