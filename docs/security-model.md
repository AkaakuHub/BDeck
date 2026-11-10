# BDeckのセキュリティ境界

- ページのWebViewと、TauriのIPCを使用する操作画面を分離します。
- IPCの呼び出し元の許可と入力検証は`desktop`側で扱います。
- ログイン領域はアカウントに対応するプロファイルで分離します。
- アカウント削除は保存とネイティブデータの消去が完了した通知を確認します。
- 更新ファイルはTauriアップデーターで署名とバージョンを検証します。

CSPは`desktop/tauri.conf.json`、呼び出し元の許可は`desktop/src/lib.rs`、アカウント管理は`desktop/src/deck/`と`crates/zephium-app/src/shell/deck/`を参照してください。

macOSとWindowsのインストーラー生成・RustテストはGitHub Actionsで確認します。静的検証とビルド成功は、実機での表示・操作確認を代替しません。
