<script lang="ts">
  import { Cancel01Icon } from "@hugeicons/core-free-icons";
  import { layout } from "$domain/layout";
  import { accountAvatars, deck } from "$domain/deck";
  import { commands } from "$shared/ipc/bindings";
  import IconButton from "$shared/ui/IconButton";
  import AccountAvatar from "./accounts/AccountAvatar.svelte";
  import AccountPanel from "./AccountPanel.svelte";
  const frame = $derived(layout.composer());
</script>
{#if frame?.managing}
  <AccountPanel {frame} />
{:else if frame}
  <header class="composer-header" data-tauri-drag-region="deep" style:left={`${frame.x}px`} style:top={`${frame.y}px`} style:width={`${frame.width}px`}>
    <div class="accounts" role="group" aria-label="投稿するアカウント">
      {#each deck.state()?.accounts ?? [] as account (account.id)}
        <button class="account" aria-label={deck.displayName(account.id)} title={deck.displayName(account.id)} aria-pressed={frame.account === account.id} onclick={() => void commands.composerOpen(account.id)}><AccountAvatar url={accountAvatars.url(account.profile)} size={24} /></button>
      {/each}
    </div>
    <IconButton icon={Cancel01Icon} label="投稿パネルを閉じる" buttonSize={26} size={15} onclick={() => void commands.runCommand("composer.close")} />
  </header>
{/if}
<style>
  .composer-header { position: absolute; z-index: 3; display: flex; align-items: center; gap: 8px; height: 30px; padding-inline: 6px; box-sizing: border-box; background: var(--color-surface); color: var(--color-text); border-block-end: 1px solid var(--color-border); }
  .accounts { display: flex; align-items: center; gap: 4px; flex: 1; min-width: 0; overflow-x: auto; }
  .account { display: inline-flex; flex: none; align-items: center; justify-content: center; padding: 1px; border: 1px solid transparent; border-radius: 50%; background: transparent; cursor: pointer; }
  .account[aria-pressed="true"] { border-color: var(--color-ring); background: var(--color-fill); }
  .account:focus-visible { outline: 2px solid var(--color-ring); outline-offset: 1px; }
</style>
