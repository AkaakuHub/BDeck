<script lang="ts">
  import Icon from "../Icon/Icon.svelte";
  import { Tick02Icon } from "@hugeicons/core-free-icons";
  let { on }: { on: boolean } = $props();
</script>

<span class="ui-menu-mark" aria-hidden="true">
  {#if on}
    <Icon icon={Tick02Icon} size={12} />
  {/if}
</span>
