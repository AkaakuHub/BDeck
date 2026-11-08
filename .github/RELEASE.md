# BDeckのリリース

通常のpush・PR・手動実行では`ci.yml`、`v*`タグのpushでは`release.yml`が共通の`build.yml`を呼び出します。

- macOS(Apple Silicon): `aarch64-apple-darwin`のDMG
- macOS(Intel): `x86_64-apple-darwin`のDMG
- Windows(x64): `x86_64-pc-windows-msvc`のNSISインストーラー

pnpmのパッケージとRustの依存ビルドをキャッシュします。Rustキャッシュはターゲットとツールチェーン・依存関係で分離し、デフォルトブランチとリリースタグからだけ保存します。ビルドジョブはリポジトリの読み取り権限のみ、Draft Release作成ジョブだけ書き込み権限を持ちます。

## タグの作成

1. `desktop/Cargo.toml`のバージョンを更新し、`cargo check -p zephium-desktop`で`Cargo.lock`へ反映してコミットします。
2. 同じバージョンのタグをpushします。

```sh
git tag -a v1.0.4 -m "BDeck v1.0.4"
git push origin v1.0.4
```

全ビルドが成功すると、3つのインストーラーと`SHA256SUMS`をDraft Releaseに保存します。公開はGitHubのReleases画面から行います。公開済みの同じタグの成果物は上書きしません。失敗した実行はActions画面から再実行できます。

現在、macOSはアドホック署名で公証なし、Windowsは発行元署名なしです。Apple DeveloperやWindows署名サービスの秘密情報は必要ありません。自動更新の配信は行いません。
