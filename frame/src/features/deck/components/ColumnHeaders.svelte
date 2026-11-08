<script lang="ts">
  import { Settings01Icon, Refresh01Icon, PuzzleIcon, Cancel01Icon } from "@hugeicons/core-free-icons";
  import { columnIcon } from "../lib/column-icon";
  import { deck, targetName } from "$domain/deck";
  import { layout } from "$domain/layout";
  import { columnPanel } from "$domain/deck";
  import { tabs } from "$domain/tabs";
  import { commands } from "$shared/ipc/bindings";
  import Icon from "$shared/ui/Icon";
  import IconButton from "$shared/ui/IconButton";

</script>

<div class="column-headers" id="deck-columns">
  {#each layout.columns() as frame (frame.id)}
    {@const column = deck.active()?.columns.find((item) => item.id === frame.id)}
    {#if column}
      {@const account = deck.state()?.accounts.find((item) => item.id === column.account)}
      <header class="column-header" data-tauri-drag-region="deep" style:left={`${frame.x}px`} style:top={`${frame.y}px`} style:width={`${frame.width}px`} style:height={`${frame.height}px`}>
        <span class="column-icon"><Icon icon={columnIcon(column.target)} size={17} /></span>
        <strong title={`${targetName(column.target)} · ${account ? deck.displayName(account.id) : ""}`}>{targetName(column.target)}{#if account} · {deck.displayName(account.id)}{/if}</strong>
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
  .column-headers { position: absolute; inset: 0; clip-path: inset(48px 0 0 56px); pointer-events: none; background: var(--color-surface); color: var(--color-text); }
  .column-header { pointer-events: auto; position: absolute; display: flex; align-items: center; gap: 8px; padding-inline: 12px 6px; box-sizing: border-box; background: var(--color-surface); border-block-end: 1px solid var(--color-border); }
  .column-icon { display: flex; flex: none; color: var(--color-muted); }
  strong { flex: 1; min-width: 0; font-size: 13px; font-weight: 600; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
</style>
