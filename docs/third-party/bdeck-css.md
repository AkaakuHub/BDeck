# BDeckのCSS移植元

BDeckは、次のMITライセンスソフトウェアの表示規則と操作構成を参照しています。CSSはカラム、アバター、メディア、スクロールバー、ナビゲーションに分割し、利用者が個別に選択できます。旧クライアントのAPI通信、自動投稿、自動巡回、投稿内容の取得処理は移植していません。投稿にはアカウントごとの公式Xの投稿画面を使用します。

## OldTweetDeck

Copyright 2023 dimden (dimden.dev)

参照元: https://github.com/AkaakuHub/OldTweetDeckCSS、https://github.com/dimdenGD/OldTweetDeck/

## BetterTweetDeck

Copyright (c) 2014 Damien Erambert

参照元: https://github.com/eramdam/BetterTweetDeck

## MIT License

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## 機能ごとの参照元

- カラムの見出し・余白・本文の折り返し: OldTweetDeckのカラム構造を参照し、現在のXのDOMへ適応しました。
- チャットの列幅: 現在のXのShadow DOM内にある受信箱の最小幅400pxを解除し、列幅に合わせます。テーマの色や会話内容は変更しません。
- ナビゲーションのタブと投稿ツールは列内で折り返し、横スクロールを作りません。
- アバターの角丸: BetterTweetDeckの`src/features/changeAvatarShape.css`を参照しています。
- 細いスクロールバー・非表示: BetterTweetDeckの`src/features/changeScrollbars.css`を参照しています。
- 配色はX自身の表示設定を使用します。BDeckから背景色・文字色・アイコン色を上書きしません。
- カラムの設定・投稿パネル: OldTweetDeckの操作構成を参照しています。入力欄の外で押されたNキーで公式Xの投稿画面を開き、投稿欄へフォーカスします。投稿ボタンのクリックや送信は自動化しません。

旧TweetDeck固有のセレクターやAPIフックは現在のXには適用できないため、表示用の規則だけを現在のXに合わせています。
