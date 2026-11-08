<script lang="ts">
  import { onMount } from "svelte";
  import { webext } from "$domain/webext";
  import Button from "$shared/ui/Button";
  import SettingsGroup from "$shared/ui/SettingsGroup";
  import WebExtensionAccessPrompt from "./WebExtensionAccessPrompt.svelte";
  import ExtensionRow from "./ExtensionRow.svelte";
  import WebExtensionReview from "./WebExtensionReview.svelte";
  let listing = $state("");
  onMount(() => { void webext.refresh(); return () => { if (webext.pending()) void webext.cancel(); }; });
</script>

<div class="manager">
  {#if webext.failure() && !webext.pending() && !webext.accessRequest()}<p role="alert" class="error">{webext.failure()}</p>{/if}
  <SettingsGroup title="インストール済み">
  <ul class="installed">{#each webext.list() as extension (extension.install_id)}<ExtensionRow {extension} />{:else}<li class="empty">インストール済みの拡張機能はありません。</li>{/each}</ul>
  </SettingsGroup>
  <SettingsGroup title="拡張機能を追加">
  <form onsubmit={(event) => { event.preventDefault(); void webext.prepareListing(listing); }}>
    <label for="extension-listing">Chrome Web StoreのURL</label>
    <div class="install"><input id="extension-listing" type="url" bind:value={listing} maxlength={4096} required disabled={webext.working()} /><Button type="submit" disabled={webext.working() || !listing.trim()}>追加</Button></div>
  </form>
  <div class="files"><Button disabled={webext.working()} onclick={() => void webext.chooseFile(false)}>CRX・ZIPから追加</Button><Button disabled={webext.working()} onclick={() => void webext.chooseFile(true)}>フォルダーから追加</Button></div>
  </SettingsGroup>
  {#if webext.pending()}<WebExtensionReview />{/if}
  {#if webext.accessRequest()}
    {@const request = webext.accessRequest()}
    {#if request}<WebExtensionAccessPrompt {request} />{/if}
  {/if}

</div>

<style>
  .manager { min-width: 0; }
  form { display: grid; gap: 8px; }
  label { font-size: 13px; font-weight: 500; }
  .files { margin-block-start: 12px; }
  form, .files { padding: 16px; }
  .install, .files { display: flex; gap: 8px; flex-wrap: wrap; }
  input { flex: 1; min-width: 200px; padding: 8px 12px; border: 1px solid var(--color-border); border-radius: var(--radius-control); background: var(--color-raised); color: var(--color-text); font: inherit; }
  .installed { margin: 0; padding: 0; list-style: none; border-radius: var(--radius-panel); background: var(--color-raised); overflow: hidden; }
  .empty { padding: 24px; color: var(--color-muted); }
  .error { color: var(--color-danger); }
</style>
