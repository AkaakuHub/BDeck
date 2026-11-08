<script lang="ts">
  import { Cancel01Icon } from "@hugeicons/core-free-icons";
  import { layout } from "$domain/layout";
  import { accountAvatars, columnPanel, deck } from "$domain/deck";
  import { commands } from "$shared/ipc/bindings";
  import IconButton from "$shared/ui/IconButton";
  import Button from "$shared/ui/Button";
  import AccountAvatar from "./accounts/AccountAvatar.svelte";
  import AccountPanel from "./AccountPanel.svelte";
  let menuFailed = $state(false);
  async function openMenu(event: MouseEvent) {
    menuFailed = false;
    try { await columnPanel.composer(event); } catch { menuFailed = true; }
  }
  const frame = $derived(layout.composer());
  const account = $derived(deck.state()?.accounts.find((account) => account.id === frame?.account));
</script>
{#if frame?.managing}
  <AccountPanel {frame} />
{:else if frame}
  <header class="composer-header" data-tauri-drag-region="deep" style:left={`${frame.x}px`} style:top={`${frame.y}px`} style:width={`${frame.width}px`}>
    <strong>新しいポスト</strong>
    <div class="account"><Button aria-label={menuFailed ? "アカウントメニューを開けませんでした" : "投稿するアカウントを選択"} onclick={(event) => void openMenu(event)}><AccountAvatar url={account ? accountAvatars.url(account.profile) : null} size={22} />{deck.displayName(frame.account)}</Button></div>
    <IconButton icon={Cancel01Icon} label="投稿パネルを閉じる" buttonSize={26} size={15} onclick={() => void commands.runCommand("composer.close")} />
  </header>
{/if}
<style>
  .composer-header { position: absolute; z-index: 3; display: flex; align-items: center; gap: 10px; height: 40px; padding-inline: 12px 6px; box-sizing: border-box; background: var(--color-surface); color: var(--color-text); border-block-end: 1px solid var(--color-border); }
  strong { flex: 1; font-size: 13px; font-weight: 600; white-space: nowrap; }
  .account { min-width: 0; max-width: 160px; }
</style>
