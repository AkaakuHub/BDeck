<script lang="ts">
  import { Cancel01Icon } from "@hugeicons/core-free-icons";
  import { onMount, untrack } from "svelte";
  import type { WebExtensionView } from "$shared/ipc/bindings";
  import { webext } from "$domain/webext";
  import Button from "$shared/ui/Button";
  import Icon from "$shared/ui/Icon";

  let { extension, onclose }: { extension: WebExtensionView; onclose: () => void } = $props();
  let sites = $state(untrack(() => extension.access === "sites" ? [...extension.sites] : []));
  let draft = $state("");
  let field = $state<HTMLInputElement>();

  onMount(() => field?.focus());

  function add() {
    const site = draft.trim().toLowerCase();
    if (site && !sites.includes(site)) sites = [...sites, site];
    draft = "";
  }

  async function done() {
    if (draft.trim()) add();
    await webext.setAccess(extension.id, "sites", sites);
    if (!webext.failure()) onclose();
  }
</script>

<div class="editor" role="group" aria-label={`${extension.name}のサイトへのアクセス`}>
  {#if sites.length === 0}
    <p class="note">許可したサイトはありません。</p>
  {:else}
    <ul class="sites">
      {#each sites as site (site)}
        <li>
          <span>{site}</span>
          <button
            type="button"
            disabled={webext.working()}
            aria-label={`${site}を削除`}
            onclick={() => (sites = sites.filter((entry) => entry !== site))}
            ><Icon icon={Cancel01Icon} size={12} /></button
          >
        </li>
      {/each}
    </ul>
  {/if}
  <form
    class="add"
    onsubmit={(event) => {
      event.preventDefault();
      add();
    }}
  >
    <input
      bind:this={field}
      bind:value={draft}
      type="text"
      inputmode="url"
      autocomplete="off"
      spellcheck="false"
      aria-label="サイトを追加"
      disabled={webext.working()}
      onkeydown={(event) => {
        if (event.key === "Escape") {
          event.stopPropagation();
          onclose();
        }
      }}
    />
    <Button size="compact" variant="secondary" type="submit" disabled={webext.working() || !draft.trim()}>サイトを追加</Button>
    <Button size="compact" variant="ghost" disabled={webext.working()} onclick={onclose}>キャンセル</Button>
    <Button size="compact" variant="primary" disabled={webext.working()} onclick={() => void done()}>保存</Button>
  </form>
</div>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0 12px 12px 52px;
    padding: 10px 12px;
    border-radius: var(--radius-row);
    background: var(--color-fill);
  }


  .note {
    margin: 0;
    font-size: var(--text-label);
    color: var(--color-faint);
  }

  .sites {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .sites li {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 3px 4px 3px 9px;
    border-radius: 999px;
    background: var(--color-raised);
    font-size: var(--text-label);
    color: var(--color-text);
  }

  .sites button {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: 999px;
    background: none;
    color: var(--color-muted);
    cursor: pointer;
  }

  .sites button:hover {
    background: var(--row-active);
    color: var(--color-text);
  }

  .add {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .add input {
    flex: 1;
    min-width: 140px;
    height: var(--control-regular);
    padding-inline: var(--control-inset);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-field);
    background: var(--color-page);
    font: inherit;
    font-size: var(--text-label);
    color: var(--color-text);
  }

  .add input:focus-visible {
    border-color: var(--color-accent);
    outline: none;
  }
</style>
