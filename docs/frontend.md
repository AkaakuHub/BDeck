# BDeckのフロントエンド

Svelte 5、TypeScript、Viteを使用します。`browser.html`がメイン画面、`column.html`がカラム設定と投稿パネルの入口です。

- `app/`: 画面の組み立て。
- `features/deck/`: カラム、アカウント、設定の表示と操作。
- `domain/deck/`: Rustの状態通知と操作要求。
- `shared/ipc/`: IPCの型と通信。
- `shared/ui/`: 共通部品。

Rustが永続状態を所有します。操作の受付だけで保存・削除の成功を表示せず、処理完了と状態通知を確認します。

`src/shared/ipc/bindings.ts`は生成物です。IPCの型はRust側で変更し、対応OSで`cargo test -p zephium-desktop export_typescript_bindings`を実行して更新します。

ページのWebViewと操作画面は別のネイティブ領域です。ページに重なるメニューやパネルは、既存のネイティブ表示経路を使用します。動的な座標はRustのレイアウト通知を参照します。

アイコンは既存のHugeiconsと`shared/ui/Icon`を使用します。色は既存の意味別トークンを使用し、キーボード操作とフォーカス表示を維持します。

検証コマンドは`frame/package.json`と`.github/workflows/build.yml`を参照してください。
