<script lang="ts">
  import type { Snippet } from "svelte";
  import "../Menu/surface.css";
  import { menuMaterialFor } from "../Menu/material";

  let {
    label,
    disabled = false,
    trigger,
    children,
    menu = false,
    align = "start",
    triggerClass = "",
    panelClass = "",
    onopen,
    onclose,
    triggerTitle,
  }: {
    label: string;
    disabled?: boolean;
    trigger: Snippet;
    children: Snippet;
    menu?: boolean;
    align?: "start" | "end";
    triggerClass?: string;
    panelClass?: string;
    onopen?: () => void;
    onclose?: () => void;
    triggerTitle?: string;
  } = $props();

  let open = $state(false);
  let material = $state<"opaque" | undefined>();
  let root = $state<HTMLElement>();
  let panel = $state<HTMLElement>();
  let button = $state<HTMLButtonElement>();

  const items = () => [
    ...(panel?.querySelectorAll<HTMLElement>(
      '[role="menuitem"]:not(:disabled):not([aria-disabled="true"])',
    ) ?? []),
  ];

  function show(last = false) {
    material = menuMaterialFor(button);
    open = true;
    onopen?.();
    if (!menu) return;
    queueMicrotask(() => {
      if (open) (last ? items().at(-1) : items()[0])?.focus();
    });
  }

  function dismiss(restore: boolean) {
    if (!open) return;
    open = false;
    onclose?.();
    if (restore) button?.focus();
  }

  $effect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (!(event.target instanceof Node) || !root?.contains(event.target)) dismiss(false);
    };
    const away = () => dismiss(false);
    window.addEventListener("pointerdown", outside, true);
    window.addEventListener("blur", away);
    return () => {
      window.removeEventListener("pointerdown", outside, true);
      window.removeEventListener("blur", away);
    };
  });

  function onkeydown(event: KeyboardEvent) {
    if (menu && !open && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
      event.preventDefault();
      show(event.key === "ArrowUp");
      return;
    }
    if (event.key === "Escape" && open) {
      event.stopPropagation();
      dismiss(true);
      return;
    }
    if (!menu || !open) return;
    const list = items();
    const at = list.indexOf(document.activeElement as HTMLElement);
    const step =
      event.key === "ArrowDown"
        ? 1
        : event.key === "ArrowUp"
          ? -1
          : event.key === "Home"
            ? 0
            : null;
    if (event.key === "End") {
      event.preventDefault();
      list.at(-1)?.focus();
    } else if (step === 0) {
      event.preventDefault();
      list[0]?.focus();
    } else if (step !== null && list.length > 0) {
      event.preventDefault();
      list[(at + step + list.length) % list.length]?.focus();
    }
  }
</script>
<div
  bind:this={root}
  class="ui-disclosure"
  role="presentation"
  {onkeydown}
  onfocusout={(event) => {
    const next = event.relatedTarget;
    if (next instanceof Node && !root?.contains(next)) dismiss(false);
  }}
>
  <button
    bind:this={button}
    type="button"
    {disabled}
    class={triggerClass}
    aria-label={label}
    title={triggerTitle}
    aria-haspopup={menu ? "menu" : "dialog"}
    aria-expanded={open}
    data-state={open ? "open" : "closed"}
    onclick={() => (open ? dismiss(false) : show())}
  >
    {@render trigger()}
  </button>
  <div
    data-menu-material={material}
    bind:this={panel}
    class={["ui-menu", "ui-disclosure-panel", !menu && "ui-popover", panelClass]}
    role={menu ? "menu" : "group"}
    onclick={(event) => {
      const target = event.target as Element | null;
      if (
        target?.closest("[data-dismiss]") ||
        (menu && !target?.closest("[data-keep-open]") && target?.closest('[role="menuitem"]'))
      )
        dismiss(true);
    }}
    aria-label={label}
    data-align={align}
    data-open={open}
    inert={!open}
  >
    {@render children()}
  </div>
</div>

<style>
  .ui-disclosure {
    position: relative;
    display: flex;
    min-width: 0;
  }
</style>
