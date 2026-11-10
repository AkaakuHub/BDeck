<script lang="ts">
  import { Download01Icon } from "@hugeicons/core-free-icons";
  import { updater } from "$domain/updater";
  import { commands } from "$shared/ipc/bindings";
  import Icon from "$shared/ui/Icon";
  const status = $derived(updater.status());
  const busy = $derived(status?.phase !== "available");
  let failed = $state(false);
  const label = $derived(failed ? "更新を開始できませんでした。クリックして再試行"
    : status?.phase === "checking" ? "更新を確認しています"
    : status?.phase === "downloading" ? "更新をダウンロードしています"
    : status?.phase === "verifying" ? "更新の署名を検証しています"
    : status?.phase === "installing" ? "更新を適用して再起動しています"
    : `BDeck ${status?.version}へ更新して再起動`);
  async function install() {
    failed = false;
    try { await commands.updaterInstall(); }
    catch { failed = true; }
  }
</script>

<button type="button" class="update-notice" aria-label={label} title={label} disabled={busy} onclick={() => void install()}>
  <Icon icon={Download01Icon} size={19} />
</button>

<style>
  .update-notice { display: grid; place-items: center; inline-size: 36px; block-size: 36px; border-radius: 50%; background: var(--color-compose); color: var(--color-on-compose); cursor: pointer; }
  .update-notice:hover { background: var(--color-compose-hover); }
  .update-notice:focus-visible { outline: 2px solid var(--color-ring); outline-offset: 3px; }
  .update-notice:disabled { cursor: progress; opacity: 0.65; }
</style>
