<script lang="ts">
  import { Dialog } from "bits-ui";
  import type { WebExtensionAccessRequestView } from "$shared/ipc/bindings";
  import { webext } from "$domain/webext";
  import Button from "$shared/ui/Button";
  import ExtensionIdentity from "./ExtensionIdentity.svelte";
  import PromptSheet from "./PromptSheet.svelte";
  let { request }: { request: WebExtensionAccessRequestView } = $props();
  const extension = $derived(webext.list().find((item) => item.id === request.extension_id));
</script>
<PromptSheet labelledby="extension-access-title" busy={webext.working()} onclose={() => void webext.answerAccess(false)}>
  <Dialog.Title id="extension-access-title">追加のアクセス許可</Dialog.Title>
  <ExtensionIdentity name={extension?.name ?? request.extension_id} icon={extension?.icon ?? null} />
  <ul>{#each request.warnings.length ? request.warnings : [...request.permissions, ...request.patterns] as permission}<li>{permission}</li>{/each}</ul>
  {#if webext.failure()}<p role="alert">{webext.failure()}</p>{/if}
  <div class="actions"><Button disabled={webext.working()} onclick={() => void webext.answerAccess(false)}>許可しない</Button><Button variant="primary" pending={webext.working()} onclick={() => void webext.answerAccess(true)}>許可する</Button></div>
</PromptSheet>
<style>
  :global(#extension-access-title) { margin: 0 0 20px; font-size: 17px; font-weight: 600; }
  ul { margin-block: 18px; padding-inline-start: 20px; font-size: 13px; line-height: 1.5; overflow-wrap: anywhere; }
  p { color: var(--color-danger); font-size: 13px; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-block-start: 24px; }
</style>
