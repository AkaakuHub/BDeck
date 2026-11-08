<script lang="ts" module>
  import type { IconSvgElement } from "@hugeicons/svelte";
  export type SegmentedOption = {
    value: string;
    label: string;
    icon?: IconSvgElement;
    disabled?: boolean;
  };
</script>

<script lang="ts">
  import Icon from "../Icon/Icon.svelte";

  let {
    label,
    options,
    value = $bindable(""),
    size = "regular",
    full = false,
    iconOnly = false,
    disabled = false,
    onchange,
  }: {
    label: string;
    options: ReadonlyArray<SegmentedOption>;
    value?: string;
    size?: "compact" | "regular";
    full?: boolean;
    iconOnly?: boolean;
    disabled?: boolean;
    onchange?: (value: string) => void;
  } = $props();

  let index = $derived(options.findIndex((option) => option.value === value));
  let segments: HTMLButtonElement[] = $state([]);

  function pick(next: string) {
    if (next === value) return;
    value = next;
    onchange?.(next);
  }
  function keydown(event: KeyboardEvent) {
    const step =
      event.key === "ArrowRight" || event.key === "ArrowDown"
        ? 1
        : event.key === "ArrowLeft" || event.key === "ArrowUp"
          ? -1
          : null;
    const at = Math.max(0, index);
    let next: number | null = null;
    if (step !== null) {
      const rtl = getComputedStyle(event.currentTarget as HTMLElement).direction === "rtl";
      next = (at + (rtl ? -step : step) + options.length) % options.length;
    } else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = options.length - 1;
    if (next === null) return;
    event.preventDefault();
    const option = options[next];
    if (!option || option.disabled) return;
    pick(option.value);
    segments[next]?.focus();
  }
</script>

<div
  class="segmented"
  role="radiogroup"
  aria-label={label}
  aria-disabled={disabled || undefined}
  data-size={size}
  data-full={full}
  data-icon-only={iconOnly}
  style:--seg-count={options.length}
  style:--seg-index={Math.max(0, index)}
>
  <span class="thumb" aria-hidden="true" data-visible={index >= 0}></span>
  {#each options as option, at (option.value)}
    <button
      bind:this={segments[at]}
      type="button"
      role="radio"
      class="segment"
      aria-checked={option.value === value}
      tabindex={at === Math.max(0, index) ? 0 : -1}
      disabled={disabled || option.disabled}
      title={iconOnly ? option.label : undefined}
      onclick={() => pick(option.value)}
      onkeydown={keydown}
    >
      {#if option.icon}<Icon icon={option.icon} size={13} />{/if}
      <span class="text">{option.label}</span>
    </button>
  {/each}
</div>

<style>
  .segmented {
    position: relative;
    display: inline-grid;
    grid-auto-flow: column;
    grid-auto-columns: 1fr;
    box-sizing: border-box;
    min-width: 0;
    height: var(--seg-height);
    padding: var(--seg-pad);
    border-radius: var(--seg-radius);
    background: var(--color-fill);
  }

  .segmented[data-size="regular"] {
    --seg-height: var(--control-regular);
    --seg-pad: 3px;
    --seg-radius: var(--radius-control);
  }

  .segmented[data-size="compact"] {
    --seg-height: var(--control-compact);
    --seg-pad: 2px;
    --seg-radius: var(--radius-control-compact);
  }

  .segmented[data-full="true"] {
    display: grid;
    width: 100%;
  }

  .thumb {
    position: absolute;
    inset-block: var(--seg-pad);
    inset-inline-start: var(--seg-pad);
    width: calc((100% - var(--seg-pad) * 2) / var(--seg-count));
    border-radius: calc(var(--seg-radius) - var(--seg-pad));
    background: var(--row-active);
    box-shadow: var(--shine-top);
    translate: calc(var(--seg-index) * 100%) 0;
    transition:
      translate var(--motion-base) var(--ease-spring),
      scale var(--motion-fast) var(--ease-out);
    pointer-events: none;
  }

  :global([dir="rtl"]) .thumb {
    translate: calc(var(--seg-index) * -100%) 0;
  }

  .thumb[data-visible="false"] {
    visibility: hidden;
  }

  .segmented:active .thumb {
    scale: 0.97;
  }

  .segment {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-width: 0;
    padding-inline: 10px;
    border: 0;
    border-radius: calc(var(--seg-radius) - var(--seg-pad));
    background: transparent;
    color: var(--color-muted);
    font: inherit;
    font-size: var(--text-body);
    font-weight: 500;
    letter-spacing: -0.006em;
    white-space: nowrap;
    cursor: default;
    outline: none;
    -webkit-user-select: none;
    user-select: none;
    transition: color var(--motion-instant) var(--ease-smooth);
  }

  .text {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .segmented[data-icon-only="true"] .text {
    position: absolute;
    width: 1px;
    height: 1px;
    clip-path: inset(50%);
  }

  .segment[aria-checked="false"] :global(svg) {
    opacity: 0.65;
  }

  .segment:disabled {
    opacity: 0.5;
  }

  .segment:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: -2px;
  }

  .segment:hover:not(:disabled),
  .segment[aria-checked="true"] {
    color: var(--color-text);
  }

  .segmented[data-icon-only="true"] .segment {
    padding-inline: 0;
  }

  @media (prefers-reduced-motion: reduce) {
    .thumb {
      transition: none;
    }
  }

  @media (forced-colors: active) {
    .segment[aria-checked="true"] {
      outline: 1px solid Highlight;
    }
  }
</style>
