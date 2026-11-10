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
    if (side === "left") {
      context.moveTo(112, 132);
      context.bezierCurveTo(96, 118, 62, 96, 62, 70);
      context.bezierCurveTo(62, 44, 104, 44, 104, 68);
      context.bezierCurveTo(104, 96, 36, 82, 24, 16);
      context.moveTo(21, 30);
      context.lineTo(24, 16);
      context.lineTo(37, 22);
    } else {
    context.moveTo(22, 140);
    context.bezierCurveTo(52, 134, 80, 104, 80, 78);
    context.bezierCurveTo(80, 49, 40, 49, 40, 74);
    context.bezierCurveTo(40, 105, 106, 105, 106, 60);
    context.bezierCurveTo(106, 40, 104, 22, 100, 8);
    context.moveTo(93, 20);
    context.lineTo(100, 8);
    context.lineTo(112, 15);
    }
    context.stroke();
  });
</script>
<div class="account-guide" class:left={side === "left"}>
  <canvas bind:this={canvas} width="256" height="312" aria-hidden="true"></canvas>
  <button {onclick}>{label}</button>
</div>
<style>
  .account-guide { --guide-width: 96px; position: absolute; inset-block-start: 34px; inset-inline-end: -3px; display: flex; flex-direction: column; align-items: flex-end; width: max-content; max-width: calc(100vw - 80px); color: var(--color-text); }
  .account-guide.left { inset-block-start: calc(100% + 4px); inset-inline-end: auto; inset-inline-start: 0; align-items: flex-start; }

  canvas { width: var(--guide-width); height: auto; aspect-ratio: 128 / 156; pointer-events: none; }
  .left canvas { margin-inline-start: calc(18px - var(--guide-width) * 0.1875); }
  .left button { margin-inline-start: 56px; text-align: start; white-space: nowrap; }
  button { padding: 8px 0; border: 0; background: transparent; color: inherit; font-size: 12px; cursor: pointer; }
  @media (max-height: 600px), (max-width: 760px) {  .account-guide { --guide-width: 48px; } button { max-width: 140px; text-align: end; font-size: 11px; } .left button { text-align: start; } }
  button:focus-visible { outline: 2px solid var(--color-ring); outline-offset: 4px; }
</style>
