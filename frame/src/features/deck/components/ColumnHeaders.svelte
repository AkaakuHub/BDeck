<script lang="ts">
  import { Settings01Icon, Refresh01Icon, PuzzleIcon, Cancel01Icon, GripVerticalIcon } from "@hugeicons/core-free-icons";
  import { columnIcon } from "../lib/column-icon";
  import { deck, targetName } from "$domain/deck";
  import { layout } from "$domain/layout";
  import { columnPanel } from "$domain/deck";
  import { tabs } from "$domain/tabs";
  import { commands } from "$shared/ipc/bindings";
  import Icon from "$shared/ui/Icon";
  import IconButton from "$shared/ui/IconButton";
  import { createPointerDrag } from "$shared/lib/pointer-drag.svelte";
  import { columnDrop } from "$domain/deck/column-drop";

  const drag = createPointerDrag<string>({
    ondrop(id, _target, at) {
      const drop = destination(id, at);
      if (drop) void deck.reorderColumn(id, drop.before, drop.row);
    },
  });
  function destination(id: string, at: { x: number; y: number }) {
    const frames = layout.columns().flatMap((frame) =>
      frame.x === null || frame.y === null || frame.width === null || frame.height === null
        ? [] : [{ id: frame.id, x: frame.x, y: frame.y, width: frame.width, height: frame.height }]);
    return columnDrop(frames, deck.active()?.columns ?? [], id, at, {
      left: 56, top: 48, right: window.innerWidth,
      bottom: layout.scroll()?.y ?? window.innerHeight,
    });
  }
  const insertion = $derived(drag.item ? destination(drag.item, drag.at) : null);
  $effect(() => {
    void commands.deckDragPreview(insertion?.preview ?? null).catch(() => console.error("column drop preview failed"));
  });
  $effect(() => () => { void commands.deckDragPreview(null).catch(() => console.error("column drop preview cleanup failed")); });
  $effect(() => {
    if (!drag.item) return;
    const timer = setInterval(() => {
      const scroll = layout.scroll();
      if (!scroll || !drag.item || scroll.offset === null || scroll.max_offset === null) return;
      const delta = drag.at.x < 88 ? -28 : drag.at.x > window.innerWidth - 32 ? 28 : 0;
      const offset = Math.max(0, Math.min(scroll.max_offset, scroll.offset + delta));
      if (offset !== scroll.offset) void commands.runCommand(`deck.scroll=${offset}`);
    }, 60);
    return () => clearInterval(timer);
  });

</script>

<svelte:window onkeydown={(event) => { if (event.key === "Escape") drag.cancel(); }} onblur={() => drag.cancel()} />

<div class="column-headers" id="deck-columns">
  {#each layout.columns() as frame (frame.id)}
    {@const column = deck.active()?.columns.find((item) => item.id === frame.id)}
    {#if column}
      {@const account = deck.state()?.accounts.find((item) => item.id === column.account)}
      <header class="column-header" data-tauri-drag-region="deep" style:left={`${frame.x}px`} style:top={`${frame.y}px`} style:width={`${frame.width}px`} style:height={`${frame.height}px`}>
        <button class="column-grip" data-tauri-drag-region="false" aria-label={`${targetName(column.target)}を移動`} title="カラムを移動"
          onpointerdown={(event) => { event.preventDefault(); event.currentTarget.setPointerCapture(event.pointerId); drag.begin(event, column.id); }} onpointermove={drag.move} onpointerup={drag.end}
          onpointercancel={drag.cancel} onlostpointercapture={() => drag.cancel()}
          onkeydown={(event) => { if (event.altKey && (event.key === "ArrowLeft" || event.key === "ArrowRight")) { event.preventDefault(); void deck.moveColumn(column.id, event.key === "ArrowLeft" ? -1 : 1); } }}>
          <Icon icon={GripVerticalIcon} size={14} />
        </button>
        <span class="column-icon"><Icon icon={columnIcon(column.target)} size={17} /></span>
        <strong title={account?.identity ? `${targetName(column.target)} · ${account.identity.display_name}` : targetName(column.target)}>{targetName(column.target)}{#if account?.identity} · {account.identity.display_name}{/if}</strong>
        <IconButton icon={Refresh01Icon} label={`${targetName(column.target)}を更新`} buttonSize={26} size={15} onclick={() => deck.columnAction(column.id, "reload")} />
        <IconButton icon={Settings01Icon} label={`${targetName(column.target)}の設定`} buttonSize={26} size={15} onclick={(event) => void columnPanel.open(column.id, event)} />
      </header>
    {:else if tabs.tabs().some((tab) => tab.id === frame.id && tab.content === "extension_owned")}
      <header class="column-header" data-tauri-drag-region="deep" style:left={`${frame.x}px`} style:top={`${frame.y}px`} style:width={`${frame.width}px`} style:height={`${frame.height}px`}>
        <span class="column-icon"><Icon icon={PuzzleIcon} size={17} /></span>
        <strong>拡張機能の設定</strong>
        <IconButton icon={Cancel01Icon} label="拡張機能の設定を閉じてカラムに戻る" buttonSize={26} size={15} onclick={() => void commands.runCommand("tab.close")} />
      </header>
    {/if}
  {/each}
</div>

<style>
  .column-headers { position: absolute; inset: 0; clip-path: inset(48px 0 0 56px); pointer-events: none; color: var(--color-text); }
  .column-header { pointer-events: auto; position: absolute; display: flex; align-items: center; gap: 8px; padding-inline: 2px 6px; box-sizing: border-box; background: var(--color-surface); border-block-end: 1px solid var(--color-border); }
  .column-grip { display: grid; place-items: center; flex: none; width: 18px; height: 28px; padding: 0; border: 0; background: transparent; color: var(--color-muted); cursor: grab; touch-action: none; user-select: none; }
  .column-grip:active { cursor: grabbing; }
  .column-grip:focus-visible { outline: 2px solid var(--color-ring); }
  .column-icon { display: flex; flex: none; color: var(--color-muted); }
  strong { flex: 1; min-width: 0; font-size: 13px; font-weight: 600; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
</style>
