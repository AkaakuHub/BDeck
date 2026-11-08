<script lang="ts">
  import { extensions } from "$domain/extensions";
  import { tabs } from "$domain/tabs";
  import { webext } from "$domain/webext";
  import type { WebExtensionView } from "$shared/ipc/bindings";
  import Button from "$shared/ui/Button";

  let { extension }: { extension: WebExtensionView } = $props();
  let profile = $derived(extensions.profileId());
  let tab = $derived(tabs.activeId());
  let action = $derived(extensions.activeActions(profile, tab).find(
    (candidate) => candidate.runtime.install_id === extension.install_id,
  ));

  function openPopup(event: MouseEvent) {
    const button = event.currentTarget;
    if (!action || profile === null || tab === null || !(button instanceof HTMLButtonElement)) return;
    void extensions.invoke(profile, tab, action, button.getBoundingClientRect());
  }
</script>

{#if action}
  <Button disabled={!action.enabled || extensions.isInvoking(extension.install_id)} onclick={openPopup}>{action.presents_popup ? "ポップアップ" : "開く"}</Button>
{/if}
{#if extension.has_options}
  <Button disabled={webext.working() || !extension.enabled || extension.state !== "running"} onclick={() => void webext.openOptions(extension.id)}>設定</Button>
{/if}
