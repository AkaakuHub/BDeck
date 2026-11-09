<script lang="ts">
  import { onMount } from "svelte";
  import { commands, type UpdateStatus } from "$shared/ipc/bindings";
  import { events } from "$shared/ipc/native-events";
  import { accepted, runOperation } from "$shared/ipc/operation";
  import SettingsGroup from "$shared/ui/SettingsGroup";
  import SettingsRow from "$shared/ui/SettingsRow";
  import Button from "$shared/ui/Button";
  import Switch from "$shared/ui/Switch";

  let status = $state<UpdateStatus | null>(null);
  let automatic = $state(true);
  let saving = $state(false);
  let error = $state("");
  const busy = $derived(status !== null && ["checking", "downloading", "verifying", "installing"].includes(status.phase));
  const message = $derived(status?.phase === "checking" ? "更新を確認しています…"
    : status?.phase === "current" ? "最新版です。"
    : status?.phase === "available" ? `バージョン${status.version}を利用できます。`
    : status?.phase === "downloading" ? "更新をダウンロードしています…"
    : status?.phase === "verifying" ? "署名を検証しています…"
    : status?.phase === "installing" ? "更新を適用して再起動しています…"
    : status?.error ?? "");

  function receive(next: UpdateStatus) {
    if (status === null || next.revision >= status.revision) status = next;
  }
  onMount(() => {
    let disposed = false;
    const listening = events.updater.listen(({ payload }) => { if (!disposed) receive(payload); });
    const preferences = events.uiCommand.listen(({ payload }) => {
      if (payload.startsWith("preference.updates.auto-check=")) automatic = payload.endsWith("=true");
    });
    void commands.updaterStatus().then((value) => { if (!disposed) receive(value); }).catch(() => { if (!disposed) error = "更新状態を取得できませんでした。"; });
    void commands.settingGet("updates.auto-check").then((value) => { if (!disposed) automatic = value !== "false"; }).catch(() => { if (!disposed) error = "設定を取得できませんでした。"; });
    return () => { disposed = true; void listening.then((stop) => stop()); void preferences.then((stop) => stop()); };
  });
  async function setAutomatic(value: boolean) {
    saving = true;
    error = "";
    try {
      if (accepted(await runOperation(() => commands.settingSet("updates.auto-check", String(value))))) automatic = value;
      else error = "設定を保存できませんでした。";
    } catch { error = "設定を保存できませんでした。"; }
    finally { saving = false; }
  }
  async function update(install: boolean) {
    error = "";
    try { if (install) await commands.updaterInstall(); else await commands.updaterCheck(); }
    catch { error = "更新処理を開始できませんでした。"; }
  }
</script>

<SettingsGroup title="BDeckの更新">
  <SettingsRow title="バージョン"><span>{status?.current_version ?? "…"}</span></SettingsRow>
  <SettingsRow title="起動時に更新を確認"><Switch label="起動時に更新を確認" checked={automatic} disabled={saving} onchange={(value) => void setAutomatic(value)} /></SettingsRow>
  <SettingsRow title="アプリの更新">
    {#if status?.phase === "available"}
      <Button variant="primary" onclick={() => void update(true)}>更新して再起動</Button>
    {:else}
      <Button pending={busy} onclick={() => void update(false)}>更新を確認</Button>
    {/if}
  </SettingsRow>
</SettingsGroup>
{#if message}<p role="status">{message}</p>{/if}
{#if status?.phase === "downloading"}<progress aria-label="更新のダウンロード" value={status.downloaded} max={status.total ?? undefined}></progress>{/if}
{#if error}<p role="alert">{error}</p>{/if}
<style>
  p { font-size: var(--text-body); color: var(--color-muted); }
  p[role="alert"] { color: var(--color-danger); }
  progress { inline-size: 100%; accent-color: var(--color-compose); }
</style>
