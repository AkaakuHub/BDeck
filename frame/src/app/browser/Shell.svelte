<script lang="ts">
  import { IS_WINDOWS } from "$shared/platform";
  import { deck } from "$domain/deck";
  import { tabs } from "$domain/tabs";
  import DeckChrome, { ColumnHeaders, Settings, ComposerHeader, WindowToolbar, ColumnScroll, Welcome } from "$features/deck";
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
  {:else}<ColumnHeaders /><ColumnScroll />{#if deck.state() && !deck.active()?.columns.length}<Welcome />{/if}{/if}
  <ComposerHeader />
</div>

<style>
  .bdeck-shell::before { content: ""; position: absolute; inset: 48px auto 0 55px; width: 1px; background: var(--color-border-strong); pointer-events: none; z-index: 2; }
  .bdeck-shell {
    position: relative;
    display: flex;
    width: 100vw;
    height: 100vh;
    flex-direction: row;
    overflow: hidden;
  }

</style>
