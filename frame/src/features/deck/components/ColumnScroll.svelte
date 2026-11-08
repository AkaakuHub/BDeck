<script lang="ts">
  import { layout } from "$domain/layout";
  import { commands } from "$shared/ipc/bindings";
  const frame = $derived(layout.scroll());
  let element = $state<HTMLDivElement>();
  let failed = $state(false);
  let pendingOffset = $state<number | null>(null);
  $effect(() => {
    if (!element || !frame) return;
    const offset = frame.offset ?? 0;
    if (pendingOffset !== null) {
      const expected = Math.min(pendingOffset, frame.max_offset ?? 0);
      if (Math.abs(offset - expected) > 0.5) return;
      pendingOffset = null;
    }
    if (Math.abs(element.scrollLeft - offset) > 0.5) element.scrollLeft = offset;
  });
  async function scroll(event: Event) {
    if (!(event.currentTarget instanceof HTMLElement) || !frame) return;
    const offset = event.currentTarget.scrollLeft;
    if (Math.abs(offset - (frame.offset ?? 0)) < 0.5) return;
    pendingOffset = offset;
    failed = false;
    try {
      const result = await commands.runCommand(`deck.scroll=${offset}`);
      if (!result.accepted && pendingOffset === offset) { pendingOffset = null; failed = true; }
    } catch {
      if (pendingOffset === offset) { pendingOffset = null; failed = true; }
    }
  }
</script>
{#if frame}
  <div class="column-scroll" bind:this={element} role="scrollbar" tabindex="0" aria-controls="deck-columns" aria-orientation="horizontal" aria-valuemin={0} aria-valuemax={frame.max_offset ?? 0} aria-valuenow={frame.offset ?? 0} aria-label={failed ? "カラムの移動を反映できませんでした" : "カラムを横にスクロール"} style:left={`${frame.x}px`} style:top={`${frame.y}px`} style:width={`${frame.width}px`} style:height={`${frame.height}px`} onkeydown={(event) => { const movements: Record<string, number> = { ArrowLeft: -40, ArrowRight: 40, PageUp: -(frame.width ?? 0), PageDown: frame.width ?? 0, Home: -(frame.max_offset ?? 0), End: frame.max_offset ?? 0 }; const movement=movements[event.key]; if (movement !== undefined) { event.preventDefault(); event.currentTarget.scrollLeft += movement; } }} onscroll={(event) => void scroll(event)}>
    <div class="scroll-content" style:width={`${(frame.width ?? 0)+(frame.max_offset ?? 0)}px`}></div>
  </div>
{/if}
<style>
  .column-scroll { position: absolute; overflow-x: scroll; overflow-y: hidden; background: var(--color-page); }
  .scroll-content { height: 1px; }
</style>
