<script lang="ts">
  import { Refresh01Icon } from "@hugeicons/core-free-icons";
  import Slider from "$shared/ui/Slider";
  import IconButton from "$shared/ui/IconButton";
  let { label, labelHidden = false, value, defaultValue, min, max, step = 1, unit = "", disabled = false, onchange }: {
    label: string; labelHidden?: boolean; value: number; defaultValue: number; min: number; max: number;
    step?: number; unit?: string; disabled?: boolean; onchange: (value: number) => void;
  } = $props();
</script>
<div class="range-setting">
  {#if !labelHidden}<span class="caption">{label}</span>{/if}
  <div class="control"><Slider {label} {min} {max} {step} {value} {disabled} format={(current) => `${current}${unit}`} oncommit={onchange} /><IconButton icon={Refresh01Icon} label={`${label}を初期値(${defaultValue}${unit})に戻す`} disabled={disabled || value === defaultValue} onclick={() => onchange(defaultValue)} /></div>
</div>
<style>
  .range-setting { inline-size: 100%; min-inline-size: 0; }
  .control { display: flex; align-items: center; gap: 8px; }
  .caption { display: block; margin-block-end: 4px; font-size: 13px; }
</style>
