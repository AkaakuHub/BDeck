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
    context.moveTo(34, 144);
    context.bezierCurveTo(56, 132, 74, 108, 74, 80);
    context.bezierCurveTo(74, 53, 38, 53, 38, 76);
    context.bezierCurveTo(38, 99, 102, 103, 102, 58);
    context.lineTo(102, 8);
    context.moveTo(94, 17);
    context.lineTo(102, 8);
    context.lineTo(110, 17);
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
