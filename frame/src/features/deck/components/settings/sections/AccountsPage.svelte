<script lang="ts">
  import { accountDeletion, deck } from "$domain/deck";
  import AccountRow from "../AccountRow.svelte";
  import Button from "$shared/ui/Button";
  let pending = $state(false);
  async function create() {
    if (pending) return;
    pending = true;
    await deck.createAccount();
    pending = false;
  }
</script>
{#if accountDeletion.failed()}<p role="alert">アカウントを削除できませんでした。</p>{/if}
  <ul class="accounts" aria-label="登録済みアカウント" aria-busy={accountDeletion.pending()}>
    {#each deck.state()?.accounts ?? [] as account (account.id)}
      <AccountRow {account} />
    {/each}
  </ul>
<Button {pending} disabled={accountDeletion.pending()} onclick={() => void create()}>アカウントを追加</Button>
<style>
  .accounts { margin: 0 0 24px; padding: 0; list-style: none; background: var(--color-surface); border-radius: var(--radius-control); }
</style>
