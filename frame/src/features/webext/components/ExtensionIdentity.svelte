<script lang="ts">
  import { PuzzleIcon } from "@hugeicons/core-free-icons";
  import Icon from "$shared/ui/Icon";
  let { name, icon, version }: { name: string; icon: string | null; version?: string } = $props();
</script>

<div class="identity">
  <span class="icon" aria-hidden="true">{#if icon}<img src={icon} alt="" />{:else}<Icon icon={PuzzleIcon} size={22} />{/if}</span>
  <strong>{name}</strong>
  {#if version}<span class="version">{version}</span>{/if}
</div>

<style>
  .identity { display: flex; align-items: center; flex: 1; min-width: 0; gap: 12px; }
  .icon { display: grid; flex: none; place-items: center; width: 40px; height: 40px; overflow: hidden; border-radius: var(--radius-control-compact); background: var(--color-fill); color: var(--color-muted); }
  img { width: 32px; height: 32px; object-fit: contain; }
  strong { min-width: 0; overflow-wrap: anywhere; font-size: 14px; font-weight: 550; }
  .version { flex: none; padding: 2px 7px; border-radius: var(--radius-capsule); color: var(--color-muted); background: var(--color-fill); font-size: 11px; font-variant-numeric: tabular-nums; }
</style>
