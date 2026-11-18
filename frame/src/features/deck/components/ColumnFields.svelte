<script lang="ts">
  import type { AccountOption, ColumnDraft } from "$shared/ipc/bindings";
  import AccountPicker from "./accounts/AccountPicker.svelte";
  import { DEFAULT_COLUMN_WIDTH, MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH } from "$domain/deck";
  import ColumnKindPicker from "./columns/ColumnKindPicker.svelte";
  import ResettableRange from "./controls/ResettableRange.svelte";
  let { accounts, account, width, adding = false, kind = "home", text = "", onchange }: {
    accounts: AccountOption[]; account: string; width: number; adding?: boolean;
    kind?: ColumnDraft["kind"]; text?: string; onchange: (update: Partial<ColumnDraft>) => void;
  } = $props();
</script>
<div class="fields">
  {#if adding}
    <ColumnKindPicker value={kind} onchange={(kind) => onchange({ kind, text: "" })} />
    {#if kind === "search" || kind === "list"}<input class="settings-input" aria-label={kind === "search" ? "検索語" : "リストURL"} placeholder={kind === "search" ? "検索語" : "https://x.com/i/lists/…"} type={kind === "list" ? "url" : "text"} value={text} oninput={(event) => onchange({ text: event.currentTarget.value })} required maxlength={256} />{/if}
  {/if}
  <div class="account-field"><span>アカウント</span><AccountPicker {accounts} value={account} onchange={(account) => onchange({ account })} /></div>
  <ResettableRange label="幅" value={width} defaultValue={DEFAULT_COLUMN_WIDTH} min={MIN_COLUMN_WIDTH} max={MAX_COLUMN_WIDTH} step={10} unit="px" onchange={(width) => onchange({ width })} />
</div>
<style>
  .fields { display: grid; gap: 14px; min-width: 0; }
  .account-field { display: grid; gap: 6px; font-size: 12px; color: var(--color-muted); }
  .settings-input { width: 100%; }
</style>
