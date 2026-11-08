<script lang="ts">
  import { surface } from "$domain/surface";
  import { accountDeletion, accountSessions, deck } from "$domain/deck";
  import AccountRow from "../AccountRow.svelte";
  import Button from "$shared/ui/Button";
  import SettingsGroup from "$shared/ui/SettingsGroup";
  let pending = $state(false);
  const connected = $derived(deck.state()?.accounts.find((account) => account.id === accountSessions.connected()));
  async function create() {
    if (pending) return;
    pending = true;
    await deck.createAccount();
    pending = false;
  }
</script>
{#if connected}
  <section class="connection-complete" aria-label="ログイン完了">
    <strong role="status">{deck.displayName(connected.id)}にログインしました</strong>
    <Button variant="ghost" onclick={async () => { if (await surface.close()) accountSessions.acknowledge(); }}>完了</Button>
  </section>
{/if}
{#if accountDeletion.pending()}<p role="status">アカウントの削除を処理しています…</p>{/if}
{#if accountDeletion.failed()}<p role="alert">アカウントを削除できませんでした。</p>{/if}
<SettingsGroup title="アカウント">
  <ul class="accounts">
    {#each deck.state()?.accounts ?? [] as account (account.id)}
      <AccountRow {account} />
    {/each}
  </ul>
</SettingsGroup>
<Button {pending} disabled={accountDeletion.pending()} onclick={() => void create()}>アカウントを追加</Button>
<style>
  .accounts { margin: 0; padding: 0; list-style: none; }
  .connection-complete { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; padding: 16px; margin-block-end: 20px; border-radius: var(--radius-control); background: var(--color-surface); box-shadow: var(--row-rim); }
  .connection-complete strong { flex: 1 1 180px; font-size: var(--text-body); }
</style>
