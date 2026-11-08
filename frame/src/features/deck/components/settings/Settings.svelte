<script lang="ts">
  import { onMount } from "svelte";
  import { Cancel01Icon } from "@hugeicons/core-free-icons";
  import { surface } from "$domain/surface";
  import { deck } from "$domain/deck";
  import { layout } from "$domain/layout";
  import { commands } from "$shared/ipc/bindings";
  import IconButton from "$shared/ui/IconButton";
  import AppearancePage from "./sections/AppearancePage.svelte";
  import AccountsPage from "./sections/AccountsPage.svelte";

  const extensionsPage = () => import("$features/webext").then(module => module.default);
  const title = $derived(surface.section() === "accounts" ? "アカウント" : surface.section() === "extensions" ? "拡張機能" : "設定");

  onMount(() => {
    const close = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !document.querySelector('[data-state="open"][role="listbox"]')) {
        if (layout.composer()?.managing) void commands.runCommand("composer.close");
        else void surface.close();
      }
    };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  });
</script>

<section class="settings-shell" aria-label={title}>
  {#if surface.section() === "extensions"}
    {#await extensionsPage() then Page}<Page />{:catch}<p role="alert">拡張機能の画面を開けませんでした。</p>{/await}
  {:else}
  <div class="settings-content">
    <div class="settings-page">
      <header class="settings-page-heading" data-tauri-drag-region="deep">
        <h1>{title}</h1>
        <IconButton icon={Cancel01Icon} label={`${title}を閉じる`} buttonSize={30} onclick={() => void surface.close()} />
      </header>
      {#if surface.failed() || deck.failed()}<p class="settings-feedback" role="alert">変更を反映できませんでした。</p>{/if}
      {#if surface.section() === "accounts"}<AccountsPage />{:else}<AppearancePage />{/if}
    </div>
  </div>
  {/if}
</section>

<style>
  .settings-shell {
    display: flex;
    flex: 1;
    min-width: 0;
    height: calc(100% - 48px);
    margin-block-start: 48px;
    padding-block-start: 0;
    box-sizing: border-box;
    color: var(--color-text);
    background: var(--color-page);
  }
  .settings-content { flex: 1; min-width: 0; overflow-y: auto; container-type: inline-size; }
  .settings-page { max-width: 850px; margin-inline: auto; padding: 16px clamp(24px, 5cqi, 60px) 64px; }
  .settings-page-heading { display: flex; align-items: center; justify-content: space-between; gap: 24px; margin-block-end: 32px; }
  h1 { margin: 0; font-size: 27px; line-height: 1.25; font-weight: 550; letter-spacing: -0.035em; }
  .settings-feedback { padding: 12px 16px; border-radius: var(--radius-control); background: var(--color-fill); color: var(--color-danger); }
</style>
