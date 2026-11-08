<script lang="ts">
  import { Select } from "bits-ui";
  import Icon from "../Icon/Icon.svelte";
  import { ArrowUpDownIcon } from "@hugeicons/core-free-icons";
  import Mark from "../Menu/Mark.svelte";
  import "../Menu/popover.css";
  import "../Shine/shine.css";
  import "../Button/control.css";
  import { menuMaterialFor } from "../Menu/material";
  import { layoutAnchor } from "../Menu/anchor";
  import { untrack } from "svelte";
  let {
    label,
    labelHidden = false,
    options,
    value = $bindable(""),
    disabled = false,
    onchange,
  }: {
    label: string;
    labelHidden?: boolean;
    options: ReadonlyArray<{ value: string; label: string; disabled?: boolean }>;
    value?: string;
    disabled?: boolean;
    onchange?: (value: string) => void;
  } = $props();
  const uid = $props.id();
  let triggerElement = $state<HTMLButtonElement | null>(null);
  let material = $state<"opaque" | undefined>();
  let contentElement = $state<HTMLElement | null>(null);
  let current = $derived(options.find((option) => option.value === value)?.label ?? "");
  const popup = $derived(
    triggerElement
      ? {
          contextElement: triggerElement,
          getBoundingClientRect: (): DOMRect => untrack(() => place()),
        }
      : null,
  );
  let pressed: { x: number; y: number } | null = null;
  const travelled = (event: PointerEvent) =>
    !pressed || Math.hypot(event.clientX - pressed.x, event.clientY - pressed.y) > 4;

  function place(): DOMRect {
    {
      const box = layoutAnchor(triggerElement)?.getBoundingClientRect() ?? new DOMRect();
      const content = contentElement;
      const first = content?.querySelector<HTMLElement>(".ui-menu-item");
      const row = content?.querySelector<HTMLElement>(".ui-menu-item[data-selected]");
      if (!content || !triggerElement || !first || !row) {
        return new DOMRect(box.x, box.bottom + 6, box.width, 0);
      }
      const panel = getComputedStyle(content);
      const scroll = content.querySelector<HTMLElement>(".ui-menu-viewport")?.scrollTop ?? 0;
      const rowTop = parseFloat(panel.paddingTop) + row.offsetTop - first.offsetTop - scroll;
      const inset =
        parseFloat(panel.paddingInlineStart) +
        parseFloat(getComputedStyle(row).paddingInlineStart) -
        parseFloat(getComputedStyle(triggerElement).paddingInlineStart);
      const left = box.left - inset;
      const room = window.innerHeight - 12;
      const top = Math.min(
        Math.max(12, box.top + box.height / 2 - rowTop - row.offsetHeight / 2),
        Math.max(12, room - content.offsetHeight),
      );
      content.style.transformOrigin = `${box.left + box.width / 2 - left}px ${box.top + box.height / 2 - top}px`;
      return new DOMRect(left, top, box.width + inset * 2, 0);
    }
  }
</script>

<div class="field">
  <label for={uid} class:sr-only={labelHidden}>{label}</label>
  <Select.Root
    onOpenChange={(open) => {
      if (open) material = menuMaterialFor(triggerElement);
    }}
    type="single"
    bind:value
    {disabled}
    items={[...options]}
    onValueChange={(next) => onchange?.(next)}
  >
    <Select.Trigger
      bind:ref={triggerElement}
      id={uid}
      class="ui-select ui-control ui-shine ui-shine-press"
      aria-label={label}
      onpointerdown={(event) => (pressed = { x: event.clientX, y: event.clientY })}
    >
      <span class="value">{current}</span>
      <span class="chevron"><Icon icon={ArrowUpDownIcon} size={14} /></span>
    </Select.Trigger>
    <Select.Portal>
      <Select.Content
        bind:ref={contentElement}
        data-menu-material={material}
        class="ui-menu ui-select-menu"
        side="bottom"
        align="start"
        customAnchor={popup}
        sideOffset={0}
        avoidCollisions={false}
        collisionPadding={12}
      >
        <Select.Viewport class="ui-menu-viewport">
          {#each options as option (option.value)}
            <Select.Item
              class="ui-menu-item"
              value={option.value}
              label={option.label}
              disabled={option.disabled}
              onpointerup={(event) => {
                if (travelled(event)) pressed = null;
                else event.preventDefault();
              }}
            >
              {#snippet children({ selected })}
                {option.label}
                <Mark on={selected} />
              {/snippet}
            </Select.Item>
          {/each}
        </Select.Viewport>
      </Select.Content>
    </Select.Portal>
  </Select.Root>
</div>

<style>
  .field {
    display: grid;
    gap: 6px;
    min-width: 0;
  }

  .field > label {
    font-size: var(--text-label);
    font-weight: 500;
    color: var(--color-muted);
  }

  .field :global(.ui-select) {
    justify-content: space-between;
    gap: 8px;
    min-width: 120px;
    padding-inline: var(--control-inset) 10px;
    font-weight: 450;
  }

  .value {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    text-align: start;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chevron {
    flex: none;
    color: var(--color-muted);
  }

  :global(.ui-select-menu) {
    min-width: var(--bits-floating-anchor-width, 0);
    background: var(--color-float);
    backdrop-filter: none;
    animation:
      ui-menu-fade var(--motion-fast) var(--ease-out),
      ui-select-in var(--motion-base) var(--ease-out);
  }

  @media (prefers-reduced-motion: reduce) {
    :global(.ui-select-menu) {
      animation: ui-menu-fade var(--motion-instant) linear;
    }
  }
</style>
