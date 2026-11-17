<script lang="ts">
  import { IncognitoIcon } from "@hugeicons/core-free-icons";
  import Icon from "$shared/ui/Icon";
  let { x, y, width, height }: { x: number; y: number; width: number; height: number } = $props();
</script>

<div class="secret-column" role="img" aria-label="シークレット" style:left={`${x}px`} style:top={`${y}px`} style:width={`${width}px`} style:height={`${height}px`}>
  <Icon icon={IncognitoIcon} size={36} />
</div>

<style>
  .secret-column { position: absolute; z-index: 1; display: grid; place-items: center; box-sizing: border-box; background: var(--color-surface); color: var(--color-muted); pointer-events: auto; }
</style>
