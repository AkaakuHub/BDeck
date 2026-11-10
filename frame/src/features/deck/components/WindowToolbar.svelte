<script lang="ts">
  import { PuzzleIcon, Settings01Icon, UserCircleIcon } from "@hugeicons/core-free-icons";
  import { surface } from "$domain/surface";
  import { deck } from "$domain/deck";
  import { IS_MAC } from "$shared/platform";
  import { ExtensionActions } from "$features/extensions";
  import AccountGuide from "./accounts/AccountGuide.svelte";
  import IconButton from "$shared/ui/IconButton";
</script>

<header class="window-toolbar" class:mac={IS_MAC} data-tauri-drag-region="deep">
  <strong>{deck.active()?.name ?? "BDeck"}</strong>
  <div class="window-actions">
    <ExtensionActions />
    <IconButton icon={PuzzleIcon} label="拡張機能" buttonSize={30} size={18} active={surface.visible() && surface.section() === "extensions"} onclick={() => void surface.open("extensions")} />
    <div class="account-action">
    <IconButton icon={UserCircleIcon} label="アカウント" buttonSize={30} size={18} active={surface.visible() && surface.section() === "accounts"} onclick={() => void surface.open("accounts")} />
      {#if deck.state()?.accounts.length === 0 && !surface.visible()}<AccountGuide label="ここからアカウントを追加" onclick={() => void surface.open("accounts")} />{/if}
    </div>
    <IconButton icon={Settings01Icon} label="設定" buttonSize={30} size={18} active={surface.visible() && surface.section() === "appearance"} onclick={() => void surface.open("appearance")} />
  </div>
</header>

<style>
  .window-toolbar { position: absolute; z-index: 5; inset: 0 0 auto; display: flex; align-items: center; gap: 16px; height: 48px; padding-inline: 16px 12px; box-sizing: border-box; border-block-end: 1px solid var(--color-border-strong); color: var(--color-text); user-select: none; }
  .window-toolbar.mac { padding-inline-start: 96px; }
  strong { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; font-weight: 600; pointer-events: none; }
  .window-actions { display: flex; align-items: center; gap: 6px; }
  .account-action { position: relative; display: flex; }
</style>
