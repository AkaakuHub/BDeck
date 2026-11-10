<script lang="ts">
  import { Edit02Icon, Add01Icon } from "@hugeicons/core-free-icons";
  import { columnPanel, deck, targetName } from "$domain/deck";
  import { tabs } from "$domain/tabs";
  import { surface } from "$domain/surface";
  import { accepted, runOperation } from "$shared/ipc/operation";
  import { commands } from "$shared/ipc/bindings";
  import AccountGuide from "./accounts/AccountGuide.svelte";
  import IconButton from "$shared/ui/IconButton";
  import Icon from "$shared/ui/Icon";
  import { createPointerDrag, type DragPosition } from "$shared/lib/pointer-drag.svelte";
  import { columnIcon } from "../lib/column-icon";
  let composerFailed = $state(false);
  let activationFailed = $state<string | null>(null);
  let list = $state<HTMLDivElement>();
  const columns = $derived(deck.active()?.columns ?? []);
  function destination(at: DragPosition, id: string): { before: string | null } | null {
    if (!list) return null;
    const bounds = list.getBoundingClientRect();
    if (at.x < bounds.left - 8 || at.x > bounds.right + 8 || at.y < bounds.top || at.y > bounds.bottom) return null;
    const rows = [...list.querySelectorAll<HTMLButtonElement>("[data-column-id]")];
    const before = rows.find((row) => row.dataset.columnId !== id && at.y < row.getBoundingClientRect().top + row.offsetHeight / 2);
    return { before: before?.dataset.columnId ?? null };
  }
  const drag = createPointerDrag<string>({
    ondrop(id, _target, at) {
      const target = destination(at, id);
      if (target) deck.reorderColumn(id, target.before);
    },
  });
  const insertion = $derived(drag.item ? destination(drag.at, drag.item) : null);
  const carried = $derived(columns.find((column) => column.id === drag.item));
  async function activate(id: string) {
    if (drag.absorbClick()) return;
    if (!await surface.close()) { activationFailed = id; return; }
    if (deck.active()?.columns.find((column) => column.id === id)?.suspended && !await deck.toggleSuspended(id)) { activationFailed = id; return; }
    try { activationFailed = accepted(await runOperation(() => commands.tabsActivate(id))) ? null : id; } catch { activationFailed = id; }
  }
</script>

<svelte:window onkeydown={(event) => { if (event.key === "Escape" && drag.item) { event.preventDefault(); drag.cancel(); } }} onblur={() => drag.cancel()} />
{#if deck.state()?.accounts.length}
<nav class="deck-rail" data-tauri-drag-region="deep" aria-label="カラム操作">
  <button class="compose-button" type="button" aria-label={composerFailed ? "投稿パネルを開けませんでした" : "新しいポスト (N)"} title={composerFailed ? "投稿パネルを開けませんでした" : "新しいポスト (N)"} onclick={async () => { composerFailed = !await deck.openComposer(); }}><Icon icon={Edit02Icon} size={20} /></button>
  <div class="rail-columns" bind:this={list} data-tauri-drag-region="false">
    {#each columns as column, index (column.id)}
      {@const account = deck.state()?.accounts.find((account) => account.id === column.account)}
      {@const label = `${index + 1}: ${targetName(column.target)} · ${account ? deck.displayName(account.id) : ""}${column.suspended ? " (休止中・クリックで再開)" : ""}`}
      <button class="column-link" class:active={tabs.activeId() === column.id} class:suspended={column.suspended} aria-current={tabs.activeId() === column.id ? "page" : undefined} class:drop-before={insertion?.before === column.id} class:drop-after={insertion !== null && insertion.before === null && index === columns.length - 1} class:dragging={drag.item === column.id} type="button" data-column-id={column.id} aria-label={activationFailed === column.id ? `${label}を開けませんでした` : label} title={label} onclick={() => void activate(column.id)}
        onpointerdown={(event) => drag.begin(event, column.id)}
        onpointermove={drag.move}
        onpointerup={drag.end}
        onpointercancel={drag.cancel}
        onlostpointercapture={() => drag.cancel()}
        onkeydown={(event) => { if (event.altKey && (event.key === "ArrowUp" || event.key === "ArrowDown")) { event.preventDefault(); deck.moveColumn(column.id, event.key === "ArrowUp" ? -1 : 1); } }}>
        <Icon icon={columnIcon(column.target)} size={20} /><span>{index + 1}</span>
      </button>
    {/each}
  </div>
  <div class="column-add">
  <IconButton icon={Add01Icon} label="カラムを追加" buttonSize={36} size={20} onclick={(event) => void columnPanel.add(event)} />
    {#if columns.length === 0 && !surface.visible()}<AccountGuide side="left" label="ここからカラムを追加" onclick={(event) => void columnPanel.add(event)} />{/if}
  </div>
</nav>
{/if}
{#if carried}
  <div class="drag-mark" aria-hidden="true" style:top={`${Math.max(48, Math.min(drag.at.y - 19, window.innerHeight - 42))}px`}>
    <Icon icon={columnIcon(carried.target)} size={20} />
  </div>
{/if}

<style>
  .column-add { position: relative; }
  .compose-button { display: grid; place-items: center; width: 36px; height: 36px; flex: none; border-radius: 50%; background: var(--color-compose); color: var(--color-on-compose); cursor: pointer; }
  .compose-button:hover { background: var(--color-compose-hover); }
  .compose-button:focus-visible { outline: 2px solid var(--color-compose); outline-offset: 3px; }
  .deck-rail { display: flex; flex-direction: column; align-items: center; gap: 10px; width: 56px; height: calc(100% - 48px); margin-block-start: 48px; box-sizing: border-box; padding-block: 12px; border-inline-end: 1px solid var(--color-border); color: var(--color-muted); }
  .rail-columns { display: flex; flex-direction: column; gap: 8px; overflow-y: auto; min-height: 0; padding-block: 6px; }
  .column-link { position: relative; display: grid; place-items: center; width: 36px; height: 38px; flex: none; border-radius: var(--radius-control); background: transparent; color: inherit; cursor: grab; touch-action: none; user-select: none; }
  .column-link:hover, .column-link:focus-visible { background: var(--row-hover); color: var(--color-text); }
  .column-link.suspended { opacity: 0.5; }
  .column-link.active { background: var(--row-active); color: var(--color-compose); }
  .column-link span { position: absolute; inset: 1px 2px auto auto; font-size: 9px; line-height: 1; }
  .column-link.dragging { opacity: 0.4; cursor: grabbing; }
  .column-link.drop-before::before, .column-link.drop-after::after { content: ""; position: absolute; inset-inline: 0; height: 2px; border-radius: 1px; background: var(--color-accent); pointer-events: none; }
  .column-link.drop-before::before { top: -5px; }
  .column-link.drop-after::after { bottom: -5px; }
  .drag-mark { position: fixed; inset-inline-start: 10px; z-index: 10; display: grid; place-items: center; width: 36px; height: 38px; border-radius: var(--radius-control); background: var(--color-raised); color: var(--color-text); box-shadow: var(--shadow-menu); pointer-events: none; cursor: grabbing; }
</style>
