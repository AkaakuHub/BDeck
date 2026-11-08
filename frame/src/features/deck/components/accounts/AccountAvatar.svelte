<script lang="ts">
  import { UserCircleIcon } from "@hugeicons/core-free-icons";
  import Icon from "$shared/ui/Icon";
  let { url, size = 28 }: { url: string | null; size?: number } = $props();
  let failed = $state<string | null>(null);
</script>
<span class="avatar" style:width={`${size}px`} style:height={`${size}px`} aria-hidden="true">
  {#if url && failed !== url}{#key url}{@const source = url}<img src={source} alt="" width={size} height={size} referrerpolicy="no-referrer" decoding="async" draggable="false" onerror={() => failed = source} />{/key}{:else}<Icon icon={UserCircleIcon} {size} />{/if}
</span>
<style>
  .avatar { display: inline-flex; flex: none; align-items: center; justify-content: center; overflow: hidden; border-radius: 50%; color: var(--color-muted); }
  img { width: 100%; height: 100%; object-fit: cover; }
</style>
