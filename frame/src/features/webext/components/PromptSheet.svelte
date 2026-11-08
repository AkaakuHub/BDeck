<script lang="ts">
  import type { Snippet } from "svelte";
  import { Dialog } from "bits-ui";
  let { labelledby, busy, onclose, children }: { labelledby: string; busy: boolean; onclose: () => void; children: Snippet } = $props();
</script>

<Dialog.Root open={true} onOpenChange={(open) => { if (!open && !busy) onclose(); }}>
  <Dialog.Portal>
    <Dialog.Overlay class="webext-scrim" />
    <Dialog.Content class="webext-sheet" aria-labelledby={labelledby} aria-describedby={undefined} onInteractOutside={(event) => event.preventDefault()} onEscapeKeydown={(event) => { event.stopPropagation(); if (busy) event.preventDefault(); }}>
      {@render children()}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.webext-scrim) { position: fixed; inset: 0; z-index: 50; background: color-mix(in srgb, var(--color-page) 55%, transparent); }
  :global(.webext-sheet) { position: fixed; inset: 50% auto auto 50%; translate: -50% -50%; z-index: 51; box-sizing: border-box; width: min(440px, calc(100vw - 48px)); max-height: calc(100vh - 96px); overflow-y: auto; padding: 24px; border: 1px solid var(--color-border-strong); border-radius: var(--radius-panel); background: var(--color-raised); color: var(--color-text); box-shadow: var(--shadow-menu); outline: none; }
</style>
