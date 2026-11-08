<script lang="ts">
  import { IS_WINDOWS } from "$shared/platform";
  import { tabs } from "$domain/tabs";
  import DeckChrome, { ColumnHeaders, Settings, ComposerHeader, WindowToolbar, ColumnScroll } from "$features/deck";
  import { surface } from "$domain/surface";
</script>

<div
  class="shell bdeck-shell"
  data-caption={IS_WINDOWS || undefined}
  data-zephium-active-tab={tabs.activeId() ?? ""}
  data-zephium-surface="deck"
>
  <WindowToolbar />
  <DeckChrome />
  {#if surface.visible()}
    <Settings />
  {:else}<ColumnHeaders /><ColumnScroll />{/if}
  <ComposerHeader />
</div>

<style>
  .bdeck-shell {
    position: relative;
    display: flex;
    width: 100vw;
    height: 100vh;
    flex-direction: row;
    overflow: hidden;
  }

</style>
