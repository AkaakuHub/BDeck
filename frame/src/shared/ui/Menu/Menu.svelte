<script lang="ts" module>
  import type { IconSvgElement } from "@hugeicons/svelte";
  export type MenuEntry =
    | {
        kind: "item";
        id: string;
        label: string;
        icon?: IconSvgElement;
            checked?: boolean;
        danger?: boolean;
        disabled?: boolean;
      }
    | { kind: "separator" }
    | { kind: "heading"; label: string };
</script>

<script lang="ts">
  import { DropdownMenu } from "bits-ui";
  import type { Snippet } from "svelte";
  import Icon from "../Icon/Icon.svelte";
  import Mark from "./Mark.svelte";
  import "./popover.css";
  import { menuMaterialFor } from "./material";
  import { layoutAnchor } from "./anchor";
  let {
    label,
    entries,
    trigger,
    triggerClass = "",
    contentClass = "",
    side = "bottom",
    align = "start",
    disabled = false,
    returnFocus,
    onselect,
  }: {
    label: string;
    disabled?: boolean;
    entries: MenuEntry[];
    trigger: Snippet;
    triggerClass?: string;
    contentClass?: string;
    side?: "top" | "bottom" | "left" | "right";
    align?: "start" | "center" | "end";
    returnFocus?: () => boolean;
    onselect: (id: string) => void;
  } = $props();
  let triggerElement = $state<HTMLButtonElement | null>(null);
  let material = $state<"opaque" | undefined>();
  let choice = $derived(entries.some((entry) => entry.kind === "item" && "checked" in entry));
</script>

<DropdownMenu.Root
  onOpenChange={(open) => {
    if (open) material = menuMaterialFor(triggerElement);
  }}
>
  <DropdownMenu.Trigger {disabled} bind:ref={triggerElement} class={triggerClass} aria-label={label}>
    {@render trigger()}
  </DropdownMenu.Trigger>
  <DropdownMenu.Portal>
    <DropdownMenu.Content
      data-menu-material={material}
      class={["ui-menu ui-menu-scroll", contentClass].filter(Boolean).join(" ")}
      {side}
      {align}
      customAnchor={layoutAnchor(triggerElement)}
      sideOffset={6}
      collisionPadding={10}
      onCloseAutoFocus={(event) => {
        if (returnFocus && !returnFocus()) event.preventDefault();
      }}
    >
      {#each entries as entry, index (index)}
        {#if entry.kind === "separator"}
          <DropdownMenu.Separator class="ui-menu-separator" />
        {:else if entry.kind === "heading"}
          <div class="ui-menu-heading" role="presentation">{entry.label}</div>
        {:else}
          <DropdownMenu.Item
            class="ui-menu-item"
            textValue={entry.label}
            disabled={entry.disabled}
            data-danger={entry.danger || undefined}
            onSelect={() => onselect(entry.id)}
          >
            {#if entry.icon}
              <span class="ui-menu-icon" aria-hidden="true"
                ><Icon icon={entry.icon} size={15} /></span
              >
            {/if}
            {entry.label}
            {#if choice}<Mark on={entry.checked === true} />{/if}
          </DropdownMenu.Item>
        {/if}
      {/each}
    </DropdownMenu.Content>
  </DropdownMenu.Portal>
</DropdownMenu.Root>
