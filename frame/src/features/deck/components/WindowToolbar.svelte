<script lang="ts">
  import { PuzzleIcon, Settings01Icon, UserCircleIcon } from "@hugeicons/core-free-icons";
  import { surface } from "$domain/surface";
  import { deck } from "$domain/deck";
  import { IS_MAC } from "$shared/platform";
  import { ExtensionActions } from "$features/extensions";
  import IconButton from "$shared/ui/IconButton";
</script>

<header class="window-toolbar" class:mac={IS_MAC} data-tauri-drag-region="deep">
  <strong>{deck.active()?.name ?? "BDeck"}</strong>
  <div class="window-actions">
    <ExtensionActions />
    <IconButton icon={PuzzleIcon} label="拡張機能" buttonSize={30} size={18} active={surface.visible() && surface.section() === "extensions"} onclick={() => void surface.open("extensions")} />
    <IconButton icon={UserCircleIcon} label="アカウント" buttonSize={30} size={18} active={surface.visible() && surface.section() === "accounts"} onclick={() => void surface.open("accounts")} />
    <IconButton icon={Settings01Icon} label="設定" buttonSize={30} size={18} active={surface.visible() && surface.section() === "appearance"} onclick={() => void surface.open("appearance")} />
  </div>
</header>
{#if deck.state()?.accounts.length === 0 && !surface.visible()}
  <div class="account-guide">
    <span class="guide-arrow" aria-hidden="true"></span>
    <button onclick={() => void surface.open("accounts")}>右上のアカウントから追加してください</button>
  </div>
{/if}

<style>
  .window-toolbar { position: absolute; z-index: 5; inset: 0 0 auto; display: flex; align-items: center; gap: 16px; height: 48px; padding-inline: 16px 12px; box-sizing: border-box; border-block-end: 1px solid var(--color-border); color: var(--color-text); user-select: none; }
  .window-toolbar.mac { padding-inline-start: 96px; }
  strong { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; font-weight: 600; pointer-events: none; }
  .window-actions { display: flex; align-items: center; gap: 6px; }
  .account-guide { position: absolute; inset-block-start: 52px; inset-inline-end: 48px; z-index: 4; display: flex; flex-direction: column; align-items: flex-end; color: var(--color-text); }
  .guide-arrow { position: relative; width: 112px; height: 92px; margin-inline-end: 8px; border: 2px solid currentColor; border-block-start-color: transparent; border-radius: 48% 52% 44% 56%; transform: rotate(-28deg); }
  .guide-arrow::after { content: ""; position: absolute; inset-block-start: 4px; inset-inline-end: 5px; width: 16px; height: 16px; border-block-start: 2px solid currentColor; border-inline-end: 2px solid currentColor; transform: rotate(-25deg); }
  .account-guide button { padding: 12px; color: inherit; background: var(--color-surface); border-radius: var(--radius-control); cursor: pointer; }
</style>
