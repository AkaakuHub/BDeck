<script lang="ts">
  import Button from "$shared/ui/Button";
  import SettingsGroup from "$shared/ui/SettingsGroup";
  import SettingsRow from "$shared/ui/SettingsRow";
  import { createLayoutTransfer } from "../../../lib/layout-transfer.svelte";

  const transfer = createLayoutTransfer();
</script>

<SettingsGroup title="カラム配置の入出力">
  <SettingsRow title="配置ファイル">
    <div class="actions">
      <Button pending={transfer.pending() === "import"} disabled={transfer.pending() !== null} onclick={() => void transfer.transfer("import")}>読み込む</Button>
      <Button pending={transfer.pending() === "export"} disabled={transfer.pending() !== null} onclick={() => void transfer.transfer("export")}>書き出す</Button>
    </div>
  </SettingsRow>
</SettingsGroup>
{#if transfer.message()}<p role={transfer.failed() ? "alert" : "status"} class:failed={transfer.failed()}>{transfer.message()}</p>{/if}

<style>
  .actions { display: flex; flex-wrap: wrap; gap: 8px; }
  p { font-size: var(--text-body); color: var(--color-muted); }
  p.failed { color: var(--color-danger); }
</style>
