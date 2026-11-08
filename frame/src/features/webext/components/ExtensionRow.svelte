<script lang="ts">
  import type { WebExtensionView } from "$shared/ipc/bindings";
  import { webext } from "$domain/webext";
  import { MoreHorizontalIcon } from "@hugeicons/core-free-icons";
  import Button from "$shared/ui/Button";
  import Icon from "$shared/ui/Icon";
  import Menu, { type MenuEntry } from "$shared/ui/Menu";
  import { IS_WINDOWS } from "$shared/platform";
  import SiteAccessEditor from "./SiteAccessEditor.svelte";
  import Switch from "$shared/ui/Switch";
  import ExtensionIdentity from "./ExtensionIdentity.svelte";
  let { extension }: { extension: WebExtensionView } = $props();
  let removing = $state(false);
  let editingSites = $state(false);
  let entries = $derived<MenuEntry[]>([
    ...(extension.has_options ? [{ kind: "item" as const, id: "options", label: "設定", disabled: !extension.enabled || extension.state !== "running" }] : []),
    ...(extension.site_scoped ? [
      { kind: "heading" as const, label: "サイトへのアクセス" },
      { kind: "item" as const, id: "all", label: "許可したすべてのサイト", checked: extension.access === "all" },
      ...(!IS_WINDOWS ? [{ kind: "item" as const, id: "click", label: "クリック時", checked: extension.access === "click" }] : []),
      { kind: "item" as const, id: "sites", label: "指定したサイト", checked: extension.access === "sites" },
      { kind: "separator" as const },
    ] : []),
    ...(!extension.sideloaded ? [{ kind: "item" as const, id: "update", label: "更新を確認" }] : []),
    { kind: "item", id: "remove", label: "削除", danger: true },
  ]);
  function select(id: string) {
    if (id === "options") void webext.openOptions(extension.id);
    else if (id === "all" || id === "click") void webext.setAccess(extension.id, id, []);
    else if (id === "sites") editingSites = true;
    else if (id === "update") void webext.reviewUpdate(extension.id);
    else if (id === "remove") removing = true;
  }
</script>
<li>
  <div class="row" class:off={!extension.enabled}>
    <ExtensionIdentity compact name={extension.name} icon={extension.icon} version={extension.version} />
    <div class="controls">
      <Switch label={`${extension.name}を有効にする`} checked={extension.enabled} disabled={webext.working()} onchange={(enabled) => void webext.setEnabled(extension.id, enabled)} />
      <Menu label={`${extension.name}の操作`} {entries} disabled={webext.working()} triggerClass="extension-more" align="end" onselect={select} returnFocus={() => !editingSites}>
        {#snippet trigger()}<Icon icon={MoreHorizontalIcon} size={18} />{/snippet}
      </Menu>
    </div>
  </div>
  {#if extension.error}<p role="alert" class="error">{extension.error}</p>{/if}
  {#if extension.state === "starting" || extension.state === "failed" || extension.held_update}
    <div class="actions">
      {#if extension.state === "starting"}<span role="status">起動中</span>{/if}
      {#if extension.state === "failed"}<Button disabled={webext.working()} onclick={() => void webext.retry(extension)}>再試行</Button>{/if}
      {#if extension.held_update}<Button disabled={webext.working()} onclick={() => void webext.reviewUpdate(extension.id)}>{extension.held_update}への更新を確認</Button>{/if}
    </div>
  {/if}
  {#if removing}<div class="actions"><span>{extension.name}を削除しますか？</span><Button disabled={webext.working()} onclick={() => removing = false}>キャンセル</Button><Button variant="danger" disabled={webext.working()} onclick={() => void webext.uninstall(extension.id)}>削除する</Button></div>{/if}
  {#if editingSites}<SiteAccessEditor {extension} onclose={() => editingSites = false} />{/if}
</li>
<style>
  li:not(:first-child) { border-block-start: 1px solid var(--color-border); }
  .row, .controls, .actions { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .row { justify-content: space-between; min-height: 52px; padding: 8px 12px; box-sizing: border-box; }
  .controls { flex: none; gap: 12px; }
  .off { opacity: 0.65; }
  span { color: var(--color-muted); font-size: 12px; }
  .actions { justify-content: flex-end; padding: 0 12px 10px; }
  .controls :global(.extension-more) { display: grid; place-items: center; width: 28px; height: 28px; padding: 0; border: 0; border-radius: var(--radius-control-compact); background: none; color: var(--color-muted); }
  .controls :global(.extension-more:hover), .controls :global(.extension-more[data-state="open"]) { background: var(--row-active); color: var(--color-text); }
  .controls :global(.extension-more:focus-visible) { outline: 2px solid var(--color-ring); outline-offset: 2px; }
  .error { margin: 0; padding: 0 12px 10px; color: var(--color-danger); font-size: 13px; overflow-wrap: anywhere; }
</style>
