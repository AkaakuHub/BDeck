# BDeckのリリース

通常のpush・PR・手動実行では`ci.yml`、`v*`タグのpushでは`release.yml`が共通の`build.yml`を呼び出します。

- macOS(Apple Silicon): `aarch64-apple-darwin`のDMG
- macOS(Intel): `x86_64-apple-darwin`のDMG
- Windows(x64): `x86_64-pc-windows-msvc`のNSISインストーラー

pnpmのパッケージとRustの依存ビルドをキャッシュします。Rustキャッシュはターゲットとツールチェーン・依存関係で分離し、デフォルトブランチとリリースタグからだけ保存します。ビルドジョブはリポジトリの読み取り権限のみ、Release公開ジョブだけ書き込み権限を持ちます。

## タグの作成

1. `desktop/Cargo.toml`のバージョンを更新し、`cargo check -p zephium-desktop`で`Cargo.lock`へ反映してコミットします。
2. 同じバージョンのタグをpushします。

```sh
git tag -a v1.0.4 -m "BDeck v1.0.4"
git push origin v1.0.4
```

全ビルドが成功すると、3つのインストーラー、署名付き更新パッケージ、`latest.json`と`SHA256SUMS`をReleaseとして自動公開します。公開済みの同じタグの成果物は上書きしません。失敗した実行はActions画面から再実行できます。

現在、macOSはアドホック署名で公証なし、Windowsは発行元署名なしです。Apple DeveloperやWindows署名サービスの秘密情報は必要ありません。更新パッケージはOSのコード署名とは別の専用鍵で署名します。`TAURI_SIGNING_PRIVATE_KEY`をGitHub ActionsのSecretに登録し、公開鍵をアプリに埋め込みます。秘密鍵はリポジトリに保存しません。Releaseが公開されると起動時・設定画面から更新を検知でき、署名とバージョンを検証して置き換え・再起動します。鍵を変更すると既存のアプリは更新を検証できなくなるため、保管して継続利用してください。

macOSの1Passwordデスクトップ連携は、アドホック署名の配布物では利用できません。[1Passwordが認める証明書での署名と追加ブラウザの登録](https://support.1password.com/additional-browsers/)が別途必要です。
