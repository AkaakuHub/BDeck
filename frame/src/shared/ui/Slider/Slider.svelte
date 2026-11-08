<script lang="ts">
  let {
    label,
    value = $bindable(0),
    min = 0,
    max = 100,
    step = 1,
    disabled = false,
    format = (current: number) => String(current),
    onchange,
    oncommit,
  }: {
    label: string;
    value?: number;
    min?: number;
    max?: number;
    step?: number;
    disabled?: boolean;
    format?: (value: number) => string;
    onchange?: (value: number) => void;
    oncommit?: (value: number) => void;
  } = $props();
  let dragging = $state(false);
  let rail = $state<HTMLDivElement>();
  let decimals = $derived(Math.max(0, (String(step).split(".")[1] ?? "").length));
  let ratio = $derived(max > min ? (Math.min(max, Math.max(min, value)) - min) / (max - min) : 0);
  let text = $derived(format(value));

  function quantize(next: number): number {
    const clamped = Math.min(max, Math.max(min, next));
    return Number((Math.round((clamped - min) / step) * step + min).toFixed(decimals));
  }
  function set(next: number) {
    const quantized = quantize(next);
    if (quantized === value) return;
    value = quantized;
    onchange?.(quantized);
  }
  function fromPointer(event: PointerEvent) {
    const host = rail;
    if (!host) return;
    const rect = host.getBoundingClientRect();
    const position = (event.clientX - rect.left) / rect.width;
    const along = getComputedStyle(host).direction === "rtl" ? 1 - position : position;
    set(min + Math.min(1, Math.max(0, along)) * (max - min));
  }
  function down(event: PointerEvent) {
    if (disabled || event.button !== 0) return;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    dragging = true;
    fromPointer(event);
  }
  function move(event: PointerEvent) {
    if (dragging) fromPointer(event);
  }
  function up() {
    if (!dragging) return;
    dragging = false;
    oncommit?.(value);
  }
  function key(event: KeyboardEvent) {
    if (disabled) return;
    const jump = event.shiftKey ? step * 10 : step;
    const moves: Record<string, number> = {
      ArrowRight: value + jump,
      ArrowUp: value + jump,
      ArrowLeft: value - jump,
      ArrowDown: value - jump,
      PageUp: value + step * 10,
      PageDown: value - step * 10,
      Home: min,
      End: max,
    };
    const next = moves[event.key];
    if (next === undefined) return;
    event.preventDefault();
    set(next);
    oncommit?.(value);
  }
</script>

  <div class="slim" data-dragging={dragging} data-disabled={disabled} style:--ratio={ratio}>
    <div class="rail" bind:this={rail}>
      <span class="groove" aria-hidden="true"><span class="run"></span></span>
      <span class="knob" aria-hidden="true"></span>
      <div
        class="grip"
        role="slider"
        tabindex={disabled ? -1 : 0}
        aria-label={label}
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={value}
        aria-valuetext={text}
        aria-disabled={disabled || undefined}
        onpointerdown={down}
        onpointermove={move}
        onpointerup={up}
        onpointercancel={up}
        onlostpointercapture={up}
        onkeydown={key}
      ></div>
    </div>
    <span class="reading" aria-hidden="true">{text}</span>
  </div>

<style>
  .slim {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    min-width: 0;
  }

  .slim[data-disabled="true"] {
    opacity: 0.5;
  }

  .rail {
    position: relative;
    flex: 1;
    min-width: 0;
    height: 22px;
  }

  .groove {
    position: absolute;
    inset-inline: 0;
    top: 50%;
    height: 4px;
    overflow: hidden;
    border-radius: 2px;
    background: var(--color-control);
    transform: translateY(-50%);
  }

  .run {
    position: absolute;
    inset-block: 0;
    inset-inline-start: 0;
    width: calc(var(--ratio) * 100%);
    background: var(--color-lit);
    transition: width var(--motion-base) var(--ease-out);
  }

  .knob {
    position: absolute;
    top: 50%;
    inset-inline-start: calc(var(--ratio) * (100% - 18px));
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.25);
    transform: translateY(-50%);
    pointer-events: none;
    transition:
      inset-inline-start var(--motion-base) var(--ease-out),
      scale var(--motion-fast) var(--ease-out);
  }

  .slim[data-dragging="true"] .run,
  .slim[data-dragging="true"] .knob {
    transition-duration: 0ms;
  }

  .slim[data-dragging="true"] .knob {
    box-shadow: 0 2px 6px rgb(0 0 0 / 0.3);
    scale: 1.08;
  }

  .grip {
    position: absolute;
    inset: 0;
    z-index: 2;
    touch-action: none;
    outline: none;
  }

  .slim:has(.grip:focus-visible) .knob {
    outline: 2px solid var(--color-ring);
    outline-offset: 2px;
  }

  .reading {
    flex: none;
    min-width: 3ch;
    color: var(--color-muted);
    font-size: var(--text-body);
    font-variant-numeric: tabular-nums;
    text-align: end;
  }

  @media (prefers-reduced-motion: reduce) { .run, .knob { transition: none; } }
  @media (forced-colors: active) { .groove { border: 1px solid ButtonText; } .run { background: Highlight; } .knob { background: ButtonText; } }
</style>
