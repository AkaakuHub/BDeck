<script lang="ts">
  import type { ColumnDraft } from "$shared/ipc/bindings";
  import Icon from "$shared/ui/Icon";
  import { columnIcon } from "../../lib/column-icon";
  let { value, onchange }: { value: ColumnDraft["kind"]; onchange: (kind: ColumnDraft["kind"]) => void } = $props();
  const options: { kind: ColumnDraft["kind"]; label: string }[] = [
    { kind: "home", label: "ホーム" },
    { kind: "notifications", label: "通知" },
    { kind: "profile", label: "プロフィール" },
    { kind: "messages", label: "メッセージ" },
    { kind: "explore", label: "トレンド" },
    { kind: "search", label: "検索" },
    { kind: "list", label: "リスト" },
  ];
</script>
<fieldset>
  <legend>種類</legend>
  <div class="kinds">
    {#each options as option (option.kind)}
      <label class="kind">
        <input type="radio" name="column-kind" value={option.kind} checked={value === option.kind} onchange={() => onchange(option.kind)} />
        <Icon icon={columnIcon({ kind: option.kind })} size={18} /><span>{option.label}</span>
      </label>
    {/each}
  </div>
</fieldset>
<style>
  fieldset { border: 0; padding: 0; margin: 0; min-inline-size: 0; }
  legend { padding: 0; margin-block-end: 6px; color: var(--color-muted); font-size: 12px; }
  .kinds { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 6px; }
  .kind { position: relative; display: flex; align-items: center; gap: 8px; padding: 10px; min-inline-size: 0; border: 1px solid var(--color-border); border-radius: 8px; font-size: 13px; cursor: pointer; }
  .kind:has(input:checked) { background: var(--row-active); border-color: var(--color-compose); }
  .kind:has(input:focus-visible) { outline: 2px solid var(--color-compose); outline-offset: 2px; }
  input { position: absolute; opacity: 0; inline-size: 1px; block-size: 1px; }
  span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
