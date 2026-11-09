import { commands } from "$shared/ipc/bindings";
import { accepted, runOperation } from "$shared/ipc/operation";

export function createSettingsTransfer() {
  let pending = $state<"import" | "export" | null>(null);
  let message = $state("");
  let failed = $state(false);

  async function transfer(action: "import" | "export") {
    if (pending) return;
    pending = action;
    message = "";
    failed = false;
    try {
      if (action === "export") {
        if (await commands.deckSettingsExport()) message = "設定を書き出しました。";
      } else {
        const result = await runOperation(() => commands.deckSettingsImport());
        if (result.kind === "cancelled") return;
        if (!accepted(result)) {
          failed = true;
          message = result.kind === "processed" && result.disposition.reason === "store_outcome_unknown"
            ? "保存結果を確認できません。BDeckを再起動してください。"
            : "設定を読み込めませんでした。";
        } else message = "設定を読み込みました。";
      }
    } catch (error) {
      failed = true;
      message = typeof error === "string" ? error : "設定を処理できませんでした。";
    } finally { pending = null; }
  }

  return { transfer, pending: () => pending, message: () => message, failed: () => failed };
}
