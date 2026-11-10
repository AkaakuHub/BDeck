# BDeckの開発

Node.jsとRustのバージョンは`.node-version`と`rust-toolchain.toml`、pnpmのバージョンは`package.json`で固定しています。ネイティブアプリの開発はmacOSまたはWindowsで行います。

```sh
pnpm install --frozen-lockfile
pnpm dev
```

変更後は`pnpm --dir frame typecheck`と`pnpm --dir frame build`を実行します。ネイティブ処理の変更は対応OSで検証し、CIの結果も確認してください。

[コード配置](docs/bdeck-code-layout.md)と[フロントエンドの規約](docs/frontend.md)を参照してください。PRには変更内容と検証結果を記載し、UIを変更した場合は画像を添付します。
