<script lang="ts">
  import { onMount } from "svelte";
  let { label, side = "right", onclick }: { label: string; side?: "left" | "right"; onclick: (event: MouseEvent) => void } = $props();
  let canvas: HTMLCanvasElement;
  onMount(() => {
    const context = canvas.getContext("2d");
    if (!context) return;
    context.scale(2, 2);
    context.strokeStyle = getComputedStyle(canvas).color;
    context.lineWidth = 1.8;
    context.lineCap = "round";
    context.lineJoin = "round";
    context.beginPath();
    context.moveTo(38, 146);
    context.bezierCurveTo(66, 140, 89, 66, 65, 61);
    context.bezierCurveTo(25, 49, 22, 84, 70, 86);
    context.bezierCurveTo(111, 88, 108, 34, 103, 7);
    context.moveTo(94, 17);
    context.lineTo(103, 7);
    context.lineTo(114, 16);
    context.stroke();
  });
</script>
<div class="account-guide" class:left={side === "left"}>
  <canvas bind:this={canvas} width="256" height="312" aria-hidden="true"></canvas>
  <button {onclick}>{label}</button>
</div>
<style>
  .account-guide { position: absolute; inset-block-start: 34px; inset-inline-end: -8px; display: flex; flex-direction: column; align-items: flex-end; width: max-content; max-width: calc(100vw - 80px); color: var(--color-text); }
  .account-guide.left { inset-inline-end: auto; inset-inline-start: 0; align-items: flex-start; }
  .left canvas { transform: scaleX(-1); }
  canvas { width: 128px; height: 156px; pointer-events: none; }
  button { padding: 8px 0; border: 0; background: transparent; color: inherit; font-size: 14px; cursor: pointer; }
  button:focus-visible { outline: 2px solid var(--color-ring); outline-offset: 4px; }
</style>
