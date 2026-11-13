<script lang="ts">
  import { DEFAULT_COLUMN_WIDTH } from "$domain/deck";
  import { onMount, tick } from "svelte";
  import "$styles/global.css";
  import { commands, type PanelView, type PanelSnapshot, type PanelAction, type ColumnDraft } from "$shared/ipc/bindings";
  import { events } from "$shared/ipc/native-events";
  import { Cancel01Icon, ArrowLeft02Icon, ArrowRight02Icon, Copy01Icon } from "@hugeicons/core-free-icons";
  import AccountAvatar from "./accounts/AccountAvatar.svelte";
  import ColumnFields from "./ColumnFields.svelte";
  import Icon from "$shared/ui/Icon";
  import IconButton from "$shared/ui/IconButton";
  import Button from "$shared/ui/Button";
  let view = $state<PanelView | null>(null);
  let revision = $state(0);
  let pending = $state(false);
  let failed = $state(false);
  let initial = $state<HTMLButtonElement>();
  let panel = $state<HTMLElement>();
  let draft = $state<ColumnDraft>({ kind: "home", account: "", text: "", width: DEFAULT_COLUMN_WIDTH, row: 0 });
  const adding = $derived(view?.id === null && !view?.composer);
  $effect(() => { if (view && !view.accounts.some((account) => account.id === draft.account)) draft.account = view.account; });
  function fields(update: Partial<ColumnDraft>) {
    if (adding) { draft = { ...draft, ...update }; if (update.account !== undefined) void action({ kind: "account", value: update.account }); return; }
    if (update.account !== undefined) void action({ kind: "account", value: update.account });
    if (update.width !== undefined) void action({ kind: "width", value: update.width });
    if (update.row !== undefined) void action({ kind: "row", value: update.row });
  }
  async function action(value: PanelAction) {
    if (pending) return;
    pending = true;
    failed = false;
    const current = revision;
    try { await commands.columnPanelAction(current, value); if (revision === current && view?.composer) pending = false; }
    catch { if (revision === current) { failed = true; pending = false; } }
  }
  onMount(() => {
    let disposed = false;
    let stops: (() => void)[] = [];
    let resize: ResizeObserver | undefined;
    const height = () => Math.max(100, Math.min(view?.composer ? 700 : 640, Math.ceil(panel?.getBoundingClientRect().height ?? 100)));
    async function present(next: PanelSnapshot | null) {
      resize?.disconnect();
      const current = next?.revision ?? 0;
      revision = current;
      view = null;
      await tick();
      if (disposed || revision !== current) return;
      view = next?.view ?? null;
      if (view?.id === null && !view.composer) draft.account = view.account;
      pending = false;
      failed = false;
      await tick();
      if (disposed || revision !== current || !view || !panel) return;
      if (!view.composer) initial?.focus();
      let lastHeight = height();
      await commands.columnPanelReady(current, lastHeight);
      if (disposed || revision !== current) return;
      resize = new ResizeObserver(() => {
        const nextHeight = height();
        if (revision !== current || nextHeight === lastHeight) return;
        lastHeight = nextHeight;
        void commands.columnPanelResize(current, nextHeight).catch(() => console.error("column panel sizing failed"));
      });
      resize.observe(panel);
    }
    void (async () => {
      stops = await Promise.all([
        events.columnPanelView.listen(({ payload }) => {
          if (payload === null) resize?.disconnect();
          if (payload && payload.revision !== revision) { void present(payload).catch(() => console.error("column panel presentation failed")); return; }
          revision = payload?.revision ?? 0;
          view = payload?.view ?? null;
          pending = false;
        }),
        events.columnPanelOpen.listen(({ payload }) => { void present(payload).catch(() => console.error("column panel presentation failed")); }),
      ]);
      if (disposed) { for (const stop of stops) stop(); return; }
      await present(await commands.columnPanelGet());
    })().catch(() => { failed = true; });
    return () => { disposed = true; for (const stop of stops) stop(); resize?.disconnect(); };
  });
</script>
<svelte:window onkeydown={(event) => { if (event.key === "Escape" && !event.defaultPrevented) { event.preventDefault(); void commands.columnPanelClose(revision); } }} />
{#if view}
<section bind:this={panel} aria-label={view.composer ? "投稿パネル" : adding ? "カラムを追加" : `${view.title}の設定`} class="panel" class:composer={view.composer} aria-busy={pending}>
  <header>
    {#if view.composer}<div class="composer-accounts" role="group" aria-label="投稿するアカウント">
      {#each view.accounts as account (account.id)}<button class="composer-account" aria-label={account.title} title={account.title} aria-pressed={view.account === account.id} onclick={() => void action({ kind: "account", value: account.id })}><AccountAvatar url={account.avatar} size={32} /></button>{/each}
    </div>{:else}<h1>{view.title}</h1>{/if}
    <button bind:this={initial} class="icon-button" style:--icon-button-size="28px" aria-label={view.composer ? "投稿パネルを閉じる" : "カラム設定を閉じる"} onclick={() => void commands.columnPanelClose(revision)}><Icon icon={Cancel01Icon} size={16} /></button></header>
  {#if view.composer}
    <div class="composer-content" style:height={`${view.content_height}px`} aria-hidden="true"></div>
  {:else}
  <form onsubmit={(event) => { event.preventDefault(); if (adding) void action({ kind: "create", value: { ...draft, row: draft.row } }); }}>
    <ColumnFields accounts={view.accounts} account={adding ? draft.account : view.account} width={adding ? draft.width : view.width} row={adding ? draft.row : view.row} {adding} kind={draft.kind} text={draft.text} onchange={fields} />
    {#if adding}<footer><Button type="submit" disabled={pending || !draft.account || ((draft.kind === "search" || draft.kind === "list") && !draft.text.trim())}>追加</Button></footer>{:else}
    <footer>
      <IconButton icon={ArrowLeft02Icon} label="前へ移動" disabled={view.index === 0 || pending} onclick={() => void action({ kind: "previous" })} />
      <IconButton icon={ArrowRight02Icon} label="次へ移動" disabled={view.index + 1 === view.count || pending} onclick={() => void action({ kind: "next" })} />
      <IconButton icon={Copy01Icon} label="カラムを複製" disabled={pending} onclick={() => void action({ kind: "duplicate" })} />
      <Button variant="ghost" disabled={pending} onclick={() => void action({ kind: "secret" })}>{view.secret ? "シークレットを解除" : "シークレットにする"}</Button>
      <Button variant="ghost" disabled={pending} onclick={() => void action({ kind: "suspend" })}>{view.suspended ? "再開" : "休止"}</Button>
      <IconButton icon={Cancel01Icon} label="カラムを削除" disabled={pending} onclick={() => void action({ kind: "remove" })} />
    </footer>
    {/if}
  </form>
  {/if}
  {#if failed || view.failed}<p role="alert">変更を反映できませんでした。</p>{/if}
</section>
{/if}
<style>
  :global(html) { background: transparent; }
  :global(body) { overflow-y: auto; }
  .panel { box-sizing: border-box; padding: 14px 18px; border: 1px solid var(--color-border-strong); border-radius: 12px; background: var(--color-surface); color: var(--color-text); overflow-y: auto; }
  header, footer { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  h1 { margin: 0; font-size: 14px; font-weight: 600; }
  .panel.composer { padding: 0; overflow: hidden; }
  .composer header { height: 40px; padding-inline: 6px; box-sizing: border-box; }
  .composer-accounts { display: flex; align-items: center; gap: 4px; flex: 1; min-width: 0; overflow-x: auto; }
  .composer-account { display: inline-flex; flex: none; padding: 1px; border: 1px solid transparent; border-radius: 50%; background: transparent; cursor: pointer; }
  .composer-account[aria-pressed="true"] { border-color: var(--color-ring); }
  .composer-account:focus-visible { outline: 2px solid var(--color-ring); outline-offset: 1px; }
  form { display: grid; gap: 14px; margin-block-start: 12px; }
  footer { border-block-start: 1px solid var(--color-border); padding-block-start: 10px; }
  p { margin: 8px 0 0; font-size: 12px; color: var(--color-danger); }
</style>
