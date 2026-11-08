# BDeck

Xを複数のカラムで表示する、macOS・Windows向けのデスクトップアプリです。[Zephium](https://github.com/zephium-browser/Zephium)のコードを削減し、必要なブラウザ基盤だけを再利用しています。

アカウントごとのログイン領域を分離し、カラムの配置・幅・表示スタイルを調整できます。投稿などの操作はX公式ページで行います。自動投稿・自動巡回・X内部APIによる操作は実装しません。

設定の「カラム配置の入出力」で配置とアカウントの識別情報を移せます。ログイン情報は含みません。読み込み先では、アカウント画面からそれぞれログインしてください。

ブラウザエンジン本体は同梱せず、macOSではWKWebView(WebKit)、WindowsではMicrosoft Edge WebView2を利用します。エンジン本体の実装と脆弱性修正はApple・Microsoftが管理し、OSやWebView2の更新で反映されます。BDeck自身のUI・ネイティブ連携・依存ライブラリの保守は、このリポジトリで行います。

## 開発

Rust・Node.jsのバージョンは`rust-toolchain.toml`と`.node-version`、pnpmは`package.json`で指定しています。各OSの[Tauri開発環境](https://v2.tauri.app/start/prerequisites/)が必要です。

```sh
pnpm install --frozen-lockfile
pnpm dev
```

ローカルのインストーラー生成は`pnpm build`で行います。

## 配布

通常のpush・PRではGitHub Actionsでビルドします。`desktop/Cargo.toml`のバージョンと一致する`v*`タグをpushすると、macOS(Apple Silicon・Intel)用DMGとWindows(x64)用インストーラーをDraft Releaseに保存します。手順は[リリース設定](.github/RELEASE.md)を参照してください。

## ライセンス

[MPL-2.0](LICENSE)。元Zephiumのライセンスを継承し、配布物には第三者ライブラリのライセンス表示を含めます。
