<script lang="ts">
  let { x, y, width, height }: { x: number; y: number; width: number; height: number } = $props();
</script>

<div class="secret-column" aria-hidden="true" style:left={`${x}px`} style:top={`${y}px`} style:width={`${width}px`} style:height={`${height}px`}></div>

<style>
  .secret-column { position: absolute; z-index: 1; box-sizing: border-box; border: 1px solid var(--color-border-strong); background: var(--color-surface); pointer-events: auto; }
</style>
