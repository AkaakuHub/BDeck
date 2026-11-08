<script lang="ts">
  import { Dialog } from "bits-ui";
  import { webext } from "$domain/webext";
  import Button from "$shared/ui/Button";
  import ExtensionIdentity from "./ExtensionIdentity.svelte";
  import PromptSheet from "./PromptSheet.svelte";
  const review = $derived(webext.pending());
</script>
{#if review}
  <PromptSheet labelledby="extension-review-title" busy={webext.working()} onclose={() => void webext.cancel()}>
    <Dialog.Title id="extension-review-title">{review.update ? "拡張機能を更新" : "拡張機能をインストール"}</Dialog.Title>
    <ExtensionIdentity name={review.name} icon={review.icon} version={review.version} />
    {#if review.description}<p>{review.description}</p>{/if}
    {#if review.warnings.length}<ul>{#each review.warnings as warning}<li>{warning}</li>{/each}</ul>{/if}
    {#if webext.failure()}<p role="alert" class="error">{webext.failure()}</p>{/if}
    <div class="actions"><Button disabled={webext.working()} onclick={() => void webext.cancel()}>キャンセル</Button><Button variant="primary" pending={webext.working()} onclick={() => void webext.confirm()}>{review.update ? "更新する" : "インストールする"}</Button></div>
  </PromptSheet>
{/if}
<style>
  :global(#extension-review-title) { margin: 0 0 20px; font-size: 17px; font-weight: 600; }
  p, li { font-size: 13px; line-height: 1.5; overflow-wrap: anywhere; }
  ul { margin-block: 18px; padding-inline-start: 20px; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-block-start: 24px; }
  .error { color: var(--color-danger); }
</style>
