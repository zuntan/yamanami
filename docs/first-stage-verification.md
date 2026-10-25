# 初段の動作確認

確認日：2026年10月25日。実験ブランチ experiment/first-stage の初期実装で実行しました。

## 環境

- Rust 1.95.0、wasm32-wasip2
- Wasmtime / wasmtime-wasi / wasmtime-wasi-http 48.0.7
- Linux x86_64、AMD EPYC 7402P（OSから見える論理CPU数48）
- ホスト：devプロファイル、debug情報なし
- ゲスト：Rust製サンプル、releaseプロファイル、WASI HTTP 0.2
- SQLite：rusqliteのbundled構成

## 自動チェック

- cargo fmt：ホストとサンプル
- cargo clippy --locked --all-targets -- -D warnings
- cargo test --locked：2件
- python3 tests/e2e.py：6件

E2Eテストでは、実際のCLIと管理APIによる配備・一覧・情報取得、HTTPのメソッド・URI・ヘッダー・本文、v1からv2への更新、無効な更新時の旧版維持、処理中リクエストの旧Release維持、再起動後の復元、Wasm BLOBの重複排除を確認しました。

別アプリと別リクエストでインスタンス内のカウンタが毎回1になること、4件のWASI待機が並行して完了することも確認しました。これはゲスト状態の分離と待機の並行性を示すテストで、CPU処理のマルチコア性能を定量評価するものではありません。

管理APIの認証拒否、一般ポートと管理ポートの分離、外部アドレスへのbind拒否、同じDBの二重起動拒否、外向きHTTPの明示的拒否、fuel・メモリ・実行期限・本文サイズ・アップロードサイズ・同時実行数の制限も確認しました。

4件の250ms待機は約0.260秒で完了しました。ゲストのファイルアクセス・WASI 0.3・他言語は未検証です。

## 簡易性能測定

再現用ツールは tests/bench.py、今回の生データは[first-stage-benchmark.json](first-stage-benchmark.json)です。

~~~bash
cargo build --locked
cargo build --locked --manifest-path examples/hello/Cargo.toml \
  --target wasm32-wasip2 --release --features v2
python3 tests/bench.py --binary target/debug/yamanami --requests 100 --workers 4
~~~

| 項目 | 今回の値 |
| --- | --- |
| 初回配備（コンパイル・検証・SQLite保存を含む） | 約3.32秒 |
| warmup後のリクエスト | 100件、並列数4、全件200 |
| スループット | 約1,291 req/s |
| p50 / p95 | 約2.57 / 6.00 ms |
| ホストRSS / 測定時のピークRSS | 約47.6 MiB |
| ホスト実行ファイル | 約70.7 MiB |
| ゲストWasm | 約128.4 KiB |

ローカルHTTP/1.1で毎回TCP接続を作るPythonクライアントを使用しています。本文はリクエスト0バイト・レスポンス76バイトです。ホストは最適化したreleaseビルドではなく、短時間・少数件の測定です。クライアントの処理時間も含むため、サーバーの性能上限や本番の容量を表す数字ではありません。

次の性能検証では、ホストのreleaseビルド、測定時間の延長、クライアントの分離、並列数・本文サイズ・アプリ数の変更を行う必要があります。
