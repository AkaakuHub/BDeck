import { commands } from "$shared/ipc/bindings";
import { accepted, runOperation } from "$shared/ipc/operation";

export function createLayoutTransfer() {
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
        if (await commands.deckLayoutExport()) message = "配置を書き出しました。";
      } else {
        const result = await runOperation(() => commands.deckLayoutImport());
        if (result.kind === "cancelled") return;
        if (!accepted(result)) {
          failed = true;
          message = result.kind === "processed" && result.disposition.reason === "store_outcome_unknown"
            ? "保存結果を確認できません。BDeckを再起動してください。"
            : "配置を読み込めませんでした。";
        } else message = "配置を読み込みました。";
      }
    } catch (error) {
      failed = true;
      message = typeof error === "string" ? error : "配置を処理できませんでした。";
    } finally { pending = null; }
  }

  return { transfer, pending: () => pending, message: () => message, failed: () => failed };
}
