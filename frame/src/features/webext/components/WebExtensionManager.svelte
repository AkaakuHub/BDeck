<script lang="ts">
  import { Download01Icon } from "@hugeicons/core-free-icons";
  import { onMount, tick } from "svelte";
  import { commands } from "$shared/ipc/bindings";
  import { webext } from "$domain/webext";
  import Button from "$shared/ui/Button";
  import Icon from "$shared/ui/Icon";
  import Menu from "$shared/ui/Menu";
  import Disclosure from "$shared/ui/Disclosure";
  import WebExtensionAccessPrompt from "./WebExtensionAccessPrompt.svelte";
  import ExtensionRow from "./ExtensionRow.svelte";
  import WebExtensionReview from "./WebExtensionReview.svelte";
  let listing = $state("");
  let field = $state<HTMLInputElement>();
  onMount(() => { void webext.refresh(); return () => { if (webext.pending()) void webext.cancel(); }; });
</script>

<section class="library-shell" aria-label="拡張機能">
  <div class="internal-toolbar">
    <Button size="compact" variant="ghost" onclick={() => void commands.diagnosticsShowLogs()}>ログを開く</Button>
    <div class="install-actions">
    <Menu label="ファイルから追加" disabled={webext.working()} triggerClass="toolbar-action" align="end"
      entries={[{kind:"item",id:"package",label:"CRX・ZIPから追加"},{kind:"item",id:"folder",label:"フォルダーから追加"}]}
      onselect={(id) => void webext.chooseFile(id === "folder")}>
      {#snippet trigger()}ファイルから追加{/snippet}
    </Menu>
    <Disclosure label="Chrome Web Storeから追加" disabled={webext.working()} triggerClass="toolbar-action" panelClass="extension-store-panel" align="end" onopen={() => { void tick().then(() => field?.focus()); }}>
      {#snippet trigger()}<Icon icon={Download01Icon} size={15} />ストアから追加{/snippet}
      <form onsubmit={(event) => { event.preventDefault(); void webext.prepareListing(listing); }} aria-busy={webext.isPreparing()}>
        <label for="extension-listing">Chrome Web StoreのURL</label>
        <input bind:this={field} id="extension-listing" type="url" bind:value={listing} maxlength={4096} required disabled={webext.working()} />
        <Button type="submit" variant="primary" pending={webext.isPreparing()} disabled={webext.working() || !listing.trim()}>{webext.isPreparing() ? "取得中…" : "追加"}</Button>
      </form>
    </Disclosure>
    </div>
  </div>
  <div class="content">
    {#if webext.failure() && !webext.pending() && !webext.accessRequest()}<p role="alert" class="failure">{webext.failure()}</p>{/if}
    {#if webext.list().length}<h2 class="heading">インストール済み <span class="count">{webext.list().length}</span></h2>{/if}
    <ul class="group">{#each webext.list() as extension (extension.install_id)}<ExtensionRow {extension} />{:else}<li class="empty">インストール済みの拡張機能はありません。</li>{/each}</ul>
  </div>
</section>
{#if webext.pending()}<WebExtensionReview />{/if}
{#if webext.accessRequest()}
  {@const request = webext.accessRequest()}
  {#if request}<WebExtensionAccessPrompt {request} />{/if}
{/if}

<style>
  .library-shell { min-width: 0; }
  .internal-toolbar { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 12px; margin-block-end: 20px; font-size: 13px; color: var(--color-muted); }
  .install-actions { display: flex; flex-wrap: wrap; gap: 8px; }
  .internal-toolbar :global(.toolbar-action) { display: flex; align-items: center; gap: 6px; height: 32px; padding-inline: 12px; border: 0; border-radius: var(--radius-control-compact); background: var(--color-control); font: inherit; color: var(--color-muted); cursor: pointer; transition: background-color var(--motion-instant) var(--ease-smooth), color var(--motion-instant) var(--ease-smooth); }
  .internal-toolbar :global(.toolbar-action:hover), .internal-toolbar :global(.toolbar-action[data-state="open"]) { background: var(--row-active); color: var(--color-text); }
  .internal-toolbar :global(.toolbar-action:focus-visible) { outline: 2px solid var(--color-ring); outline-offset: 2px; }
  .internal-toolbar :global(.toolbar-action:disabled) { opacity: 0.5; }
  .internal-toolbar :global(.extension-store-panel) { width: 312px; }
  .failure { margin: 0 0 12px; font-size: var(--text-label); color: var(--color-danger); }
  .heading { display: flex; align-items: flex-end; gap: 8px; margin: 4px 0 8px; padding-inline: 10px; font-size: var(--text-label); font-weight: 550; letter-spacing: 0.01em; color: var(--color-muted); }
  .count { font-weight: 400; color: var(--color-faint); }
  .group { margin: 0; padding: 0; list-style: none; border: 1px solid var(--color-border); border-radius: var(--radius-card); background: var(--color-raised); }
  .empty { padding: 24px; color: var(--color-muted); }
  form { display: grid; gap: 10px; }
  label { font-size: var(--text-label); font-weight: 500; }
  input { width: 100%; min-width: 0; box-sizing: border-box; height: var(--control-regular); padding-inline: var(--control-inset); border: 1px solid var(--color-border); border-radius: var(--radius-field); background: var(--color-field); color: var(--color-text); font: inherit; }
  input:focus-visible { outline: 2px solid var(--color-ring); outline-offset: 2px; }
</style>
