<script lang="ts">
  import { accountDeletion, accountAvatars, accountSessions, type DeckAccount } from "$domain/deck";
  import { commands } from "$shared/ipc/bindings";
  import AccountAvatar from "../accounts/AccountAvatar.svelte";
  import Button from "$shared/ui/Button";
  let { account }: { account: DeckAccount } = $props();
  let failed = $state(false);
  const status = $derived(accountSessions.status(account.id));
  async function login() {
    failed = false;
    try { failed = !(await commands.accountOpen(account.id)).accepted; }
    catch { failed = true; }
  }
</script>

<li>
  <AccountAvatar url={accountAvatars.url(account.profile)} size={32} />
  <div class="identity">{#if account.identity}<strong>{account.identity.display_name}</strong><span class="screen-name">@{account.identity.screen_name}</span>{/if}</div>
  <span class="login-state">{status === "logged_in" ? "ログイン済み" : status === "logged_out" ? "未ログイン" : status === "account_changed" ? "アカウントが一致しません" : "未確認"}</span>
  <div class="actions">
    {#if status !== "logged_in"}<Button disabled={accountDeletion.pending()} onclick={() => void login()}>{status === "unknown" && account.identity ? "開く" : "ログイン"}</Button>{/if}
    <Button variant="ghost" disabled={accountDeletion.pending()} onclick={() => void accountDeletion.remove(account.id)}>削除</Button>
  </div>
  {#if failed}<span class="error" role="alert">操作できませんでした。</span>{/if}
</li>

<style>
  li { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; padding: 16px; }
  li:not(:first-child) { border-block-start: 1px solid var(--color-border); }
  .identity { display: grid; gap: 4px; flex: 1; min-width: 0; overflow-wrap: anywhere; }
  strong { font-size: var(--text-body); }
  .screen-name { color: var(--color-muted); font-size: 12px; }
  .login-state { font-size: 12px; white-space: nowrap; color: var(--color-muted); }
  .actions { display: flex; gap: 6px; }
  .error { width: 100%; color: var(--color-danger); font-size: 12px; }
</style>
