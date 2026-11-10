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
    context.moveTo(22, 140);
    context.bezierCurveTo(52, 134, 80, 104, 80, 78);
    context.bezierCurveTo(80, 49, 40, 49, 40, 74);
    context.bezierCurveTo(40, 105, 106, 105, 106, 60);
    context.bezierCurveTo(106, 40, 104, 22, 100, 8);
    context.moveTo(93, 20);
    context.lineTo(100, 8);
    context.lineTo(112, 15);
    context.stroke();
  });
</script>
<div class="account-guide" class:left={side === "left"}>
  <canvas bind:this={canvas} width="256" height="312" aria-hidden="true"></canvas>
  <button {onclick}>{label}</button>
</div>
<style>
  .account-guide { position: absolute; inset-block-start: 34px; inset-inline-end: -3px; display: flex; flex-direction: column; align-items: flex-end; width: max-content; max-width: calc(100vw - 80px); color: var(--color-text); }
  .account-guide.left { inset-inline-end: auto; inset-inline-start: 0; align-items: flex-start; }
  .left canvas { transform: scaleX(-1); }
  canvas { width: 96px; height: 117px; pointer-events: none; }
  button { padding: 8px 0; border: 0; background: transparent; color: inherit; font-size: 14px; cursor: pointer; }
  button:focus-visible { outline: 2px solid var(--color-ring); outline-offset: 4px; }
</style>
