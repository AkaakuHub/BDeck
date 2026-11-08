<script lang="ts">
  import { ArrowLeft02Icon, Refresh01Icon, Cancel01Icon } from "@hugeicons/core-free-icons";
  import { deck } from "$domain/deck";
  import { commands, type ComposerFrame } from "$shared/ipc/bindings";
  import IconButton from "$shared/ui/IconButton";
  let { frame }: { frame: ComposerFrame } = $props();
</script>

<div class="account-backdrop" aria-hidden="true"></div>
<div class="account-panel" role="dialog" aria-modal="true" aria-label={`${deck.displayName(frame.account)}のアカウント管理`} tabindex="-1" style:left={`${frame.x}px`} style:top={`${frame.y}px`} style:width={`${frame.width}px`} style:height={`${frame.height}px`}>
  <header data-tauri-drag-region="deep">
    <IconButton icon={ArrowLeft02Icon} label="アカウント画面で戻る" buttonSize={28} onclick={() => void commands.tabsBack(frame.tab)} />
    <IconButton icon={Refresh01Icon} label="アカウント画面を更新" buttonSize={28} onclick={() => void commands.tabsReload(frame.tab)} />
    <strong>{deck.displayName(frame.account)}</strong>
    <IconButton icon={Cancel01Icon} label="アカウント画面を閉じる" buttonSize={28} onclick={() => void commands.runCommand("composer.close")} />
  </header>
</div>

<style>
  .account-backdrop { position: fixed; inset: 48px 0 0; z-index: 4; background: color-mix(in srgb, var(--color-text) 18%, transparent); }
  .account-panel { position: absolute; z-index: 5; background: var(--color-surface); box-shadow: var(--shadow-menu); border-radius: 10px; }
  header { display: flex; align-items: center; gap: 4px; height: 40px; padding-inline: 8px; box-sizing: border-box; border-block-end: 1px solid var(--color-border); }
  strong { flex: 1; min-width: 0; padding-inline: 6px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; font-weight: 600; }
</style>
