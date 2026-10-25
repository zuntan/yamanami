# 多言語電卓デモの検証

確認日：2026年10月25日。Linux x86_64で実際にビルドし、Yamanamiへ独立した5アプリとして配備しました。

## 使用した構成

| 要素 | 確認した版 |
| --- | --- |
| Rust | 1.95.0、wasm32-wasip2 |
| Wasmtime | 48.0.7 |
| MiniJinja | 2.24.0 |
| C++ | C++20、WASI SDK 34、Clang 23.1.0-wasi-sdk |
| wit-bindgen | 0.63.0（Cバインディング生成） |
| kainjow/Mustache | v4.1、c66fdd87f01f0b2d1ba704192f27355b77746a5b |
| Pythonビルド環境 | Python 3.12.3 |
| componentize-py / Chevron | 0.25.0 / 0.14.0 |
| Wasm内Python | CPython 3.14.0（画面のsys.versionから確認） |
| Node.js / TypeScript | 24.22.1 / 7.0.2 |
| Jco / ComponentizeJS | 1.39.0 / 0.23.0 |
| Mustache.js / esbuild | 4.2.0 / 0.28.2 |
| Playwright | 1.58.2、headless Chromium 145 |
| HTTPインターフェース | WASI HTTP 0.2.12 |

## 生成サイズ

下記は今回のローカル成果物のサイズです。ランタイムの初期化スナップショットなどを含むため、再ビルドでハッシュやサイズが変化する場合があります。実際の値はbuild.pyが生成するbuild/artifacts.jsonで確認できます。

| アプリ | バイト数 | MiB |
| --- | ---: | ---: |
| index | 1,214,853 | 1.16 |
| calculator-rust（v2） | 1,264,009 | 1.21 |
| calculator-cpp | 3,323,153 | 3.17 |
| calculator-python | 18,913,590 | 18.04 |
| calculator-typescript | 13,006,761 | 12.40 |

Rustのv1は1,264,010バイトでした。PythonとTypeScriptにはそれぞれ言語ランタイムが含まれます。外部のPythonやNode.jsへ処理を転送するものではありません。

## 実施したテスト

- 各言語の計算単体テスト：四則演算、負数、小数、不正入力、ゼロ除算、オーバーフロー
- Rust workspaceのwasm32-wasip2向けClippy（警告をエラー扱い）
- TypeScriptの型チェック
- 実CLIによる5アプリの配備・一覧・詳細取得
- 全言語へのHTTP POSTによる計算と異常系
- 入力にHTMLを含めた場合のエスケープ
- GETによるクリア、未知のサブパスへの404
- 4アプリへの並行HTTPリクエスト
- headless Chromiumで4つのiframe内を実際に操作：6×7、ゼロ除算、クリア
- デスクトップ2列・モバイル1列の表示とスクリーンショット確認
- Rustだけをdemo-v2へ更新し、他4アプリのSHA-256が不変であること
- サーバー再起動後の4電卓の応答とRust v2の維持

exsample/scripts/test_http.pyは上記を一連で実行し、約68.8秒で完了しました。これはテスト全体の所要時間で、アプリの起動時間や単独リクエストのベンチマークではありません。

本体の単体テスト2件と既存E2E6件も通過しています。従来の認証・実行制限・分離・配備更新のテストを維持しています。

## 本体の調整

アップロード上限は既定16 MiBのまま、設定可能にしました（許容最大128 MiB）。デモでは32 MiBを使用しました。内部のmemory、instance、table数も有限の上限で設定できます。

デモ検証ではメモリ256 MiB / memory、memory数8、instance数128、table数32、fuel10億、実行期限10秒、全体同時実行4、アプリ単位1を使用しました。これは余裕を持たせた検証条件で、必要最小値を求めたものではありません。

Craneliftの並列コンパイルを有効化し、none / speedを選べるようにしました。開発ビルドでもコンパイラの主要クレートを最適化しています。最適化前の開発ビルドでは大型Wasmの初回コンパイルが長時間かかったための調整です。コンパイルキャッシュは同じEngine内でのみ共有し、別設定のコードを使い回しません。

管理ポートのloopback限定・認証、ファイル非公開、外向きHTTPとソケットの拒否は維持しました。

## 選定時からの調整

C++のHTTP境界にはwit-bindgenの標準Cバインディングを採用しました。計算・テンプレート描画はC++です。独自のHTTP ABIやPOSIX互換層はありません。

TypeScriptのfetch-event方式では標準HTTPハンドラのexport生成が一致しなかったため、標準WASI HTTPリソースを直接操作するincomingHandlerを公開しました。WASI 0.2.12へそろえた構成で動作確認しています。Node固有のHTTPサーバーAPIは使用していません。

PythonのChevronはPure PythonのままCPython入りWasmへ取り込み、実行時のテンプレートファイル読込なしで動作しました。
