# BDeckの構成

BDeckは公式Xを複数のネイティブWebViewで表示します。macOSはWKWebView、WindowsはWebView2を使用します。

- `crates/zephium-core`: カラム・アカウント・配置のモデルと検証。
- `crates/zephium-app`: 状態変更、保存、WebViewのライフサイクルの調整。
- `crates/zephium-store`: SQLiteによる設定・プロファイルの保存。
- `crates/zephium-engine`: OS別のWebView操作とX用の表示処理。
- `crates/zephium-ipc`: Rustと画面の間で使用する型。
- `desktop`: Tauriの起動、IPC、ネイティブパネル、自動更新。
- `frame`: Svelteによる操作画面。

カラム・アカウントの保存状態はRust側が所有します。画面はIPCで操作を送り、処理完了とRust側からの通知を確認して表示を更新します。

機能別の配置は[コード配置](bdeck-code-layout.md)を参照してください。
