# Yamanami — WebAssembly Application Server

WasmアプリケーションをHTTPで配備し、指定したパスから実行する、小さな実験用アプリケーションサーバーです。

**初段の実験実装です。** 実行ファイルは `yamanami` 一本で、サーバーと管理CLIを提供します。配備対象は標準WASI HTTPハンドラを公開する **WASI 0.2コンポーネント形式のWasm 1本**です。

利用案内の机は[docs/yamanami_readme.tabletop](docs/yamanami_readme.tabletop)、開発記録は[docs_dev/](docs_dev/)、多言語デモの検討机は[exsample/docs/yamanami_exsample.tabletop](exsample/docs/yamanami_exsample.tabletop)に置きます。

多言語電卓デモは[exsample/README.md](exsample/README.md)を参照してください。

## 初段でできること

- CLI・HTTP管理APIによる配備、一覧、詳細取得、再配備による更新
- パス接頭辞の最長一致によるアプリ割り当て
- メソッド・URI・ヘッダー・本文の受け渡しと、Wasmによるレスポンス生成
- SQLiteへのWasm BLOB、アプリ、変更不可のRelease、配備履歴の保存
- SHA-256によるWasmの重複排除、コンパイル済みコードのメモリキャッシュ
- リクエストごとに新しいStore・インスタンスを作るメモリ分離と並列実行
- 再起動後の復元、無効な更新時の旧版維持
- ゲストメモリ、fuel、実行期限、同時実行数、HTTP本文、アップロードの制限

初段は一般HTTP・管理HTTPの両方をloopback限定とします。管理APIにはBearerトークンが必要です。外向きHTTP・ソケット・ホストのファイル・環境変数はゲストへ許可しません。標準WASIの時刻・乱数は利用できます。

## 開発環境の準備

Linux環境での開発を想定しています。Rustup、Cコンパイラ・リンカ、Python 3が必要です。SQLiteは同梱ソースからビルドするため、SQLiteサーバーや開発用パッケージは不要です。

~~~bash
# rust-toolchain.tomlに指定したRust、rustfmt、Clippy、WASIターゲットを準備
rustup show

# サーバーとCLIをビルド
cargo build --locked

# サンプルをビルド
cargo build --locked --manifest-path examples/hello/Cargo.toml \
  --target wasm32-wasip2 --release
~~~

Rust 1.95.0とwasm32-wasip2ターゲットを指定しています。Wasmtime、wasmtime-wasi、wasmtime-wasi-httpは48.0.7に固定し、その他の依存もCargo.lockに記録しています。サンプルには独立したCargo.lockがあります。

## 起動・配備・実行

端末Aでサーバーを起動します。トークンは例示文字列をそのまま使わず、16文字以上のランダムな値を設定してください。

~~~bash
export YAMANAMI_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
cargo run --locked -- serve
~~~

既定のアドレスと保存先は次のとおりです。

| 用途 | 既定値 |
| --- | --- |
| アプリHTTP | http://127.0.0.1:8080 |
| 管理API | http://127.0.0.1:9090 |
| SQLite | var/yamanami.sqlite3 |

端末Bには端末Aと同じトークンを設定します。

~~~bash
export YAMANAMI_TOKEN='端末Aと同じトークン'

target/debug/yamanami ctl deploy \
  examples/hello/target/wasm32-wasip2/release/yamanami_hello.wasm \
  --app hello --path /hello

target/debug/yamanami ctl list
target/debug/yamanami ctl info hello

curl -i -X POST 'http://127.0.0.1:8080/hello/echo?q=1' \
  -H 'x-demo: sample' --data 'request body'
~~~

サンプルはバージョン、メソッド、パスとクエリ、x-demoヘッダー、本文、インスタンス内の呼出回数を返します。リクエストごとにインスタンスを作るため、呼出回数は毎回1になります。

~~~text
Hello from Yamanami v1
method=Method::Post
path=/hello/echo?q=1
x-demo=sample
calls=1
body=request body
~~~

Ctrl+Cで停止し、同じデータベースで再起動すると、配備済みアプリを復元します。

## 更新

v2をビルドし、同じアプリ名へ再配備します。

~~~bash
cargo build --locked --manifest-path examples/hello/Cargo.toml \
  --target wasm32-wasip2 --release --features v2

target/debug/yamanami ctl deploy \
  examples/hello/target/wasm32-wasip2/release/yamanami_hello.wasm \
  --app hello --path /hello

curl http://127.0.0.1:8080/hello
~~~

新しいリクエストからv2を使い、すでに実行中のリクエストは選択済みのReleaseで処理を続けます。コンパイルやリンク検証に失敗した更新では、現在のReleaseを維持します。実行時に初めて起こるトラップや制限超過まで配備時に検出するものではありません。

## 設定と制限

~~~bash
target/debug/yamanami serve --help
target/debug/yamanami ctl --help

# ポートと実行制限を指定する例
target/debug/yamanami serve \
  --listen 127.0.0.1:8081 \
  --admin-listen 127.0.0.1:9091 \
  --database var/experiment.sqlite3 \
  --memory-mib 64 --fuel 10000000 --timeout-ms 3000 \
  --concurrency 16 --app-concurrency 4 --body-limit 1048576

target/debug/yamanami ctl --endpoint http://127.0.0.1:9091 list
~~~

| 項目 | 既定値・動作 |
| --- | --- |
| ゲスト線形メモリ | 64 MiB / メモリ、既定1個。ホスト全体のRSS上限ではない |
| fuel | 1リクエストあたり10,000,000 |
| ゲスト実行期限 | 3秒。インスタンス生成・本文生成完了・ハンドラ終了を含む |
| 全体 / アプリ同時実行数 | 16 / 4。空きがなければ503、待ち行列なし |
| リクエスト / レスポンス本文 | 各1 MiB。初段はサイズ制限付きバッファ方式 |
| Wasmアップロード | 既定16 MiB。--upload-limitで最大128 MiBまで設定可能 |
| 同時配備 | 1件。競合は503 |
| アプリ数 | 最大32 |
| コンパイルキャッシュ | 最近使用した8件と、現在有効・処理中のReleaseが保持するコード |
| HTTP接続数 | 一般128、管理8。超過接続は閉じる |
| HTTP接続期限 | 一般65秒、管理120秒。keep-aliveなし |
| 本文受信期限 | 一般10秒、配備30秒 |

--max-memories / --max-instances / --max-tablesでコンポーネント内部の資源個数の上限を指定できます。--compiler-opt-levelはspeed（既定）またはnoneです。コンパイルは別のブロッキング処理で、ランタイムの並列コンパイルを利用します。

fuelは計算量の制限で、CPU使用率の指定ではありません。一定fuelごとの実行譲渡とepoch interruptionにより、CPUループがTokioを占有し続けることを防ぎます。実行失敗・ゲスト制限超過は502、一般本文の超過は413を返します。

配備はコンパイルとリンク検証を終えてからSQLiteトランザクションで保存し、有効Releaseとルートを切り替えます。保存・公開の途中でクライアントが離れても処理を完了するため、配備要求がタイムアウトした場合はinfoで結果を確認してください。コンパイル処理自体には強制終了の期限を設けていません。

データベースは1サーバープロセスで使用し、ロックファイルで通常の二重起動を防ぎます。運用時は同じDBを別名・別経路から開かないでください。初段には削除・履歴の掃除機能がなく、過去ReleaseとWasmは保存し続けます。

## 管理API

アプリHTTPと管理HTTPは別のポートです。管理操作は一般HTTPポートでは処理しません。

| メソッド | パス | 内容 |
| --- | --- | --- |
| GET | /v1/apps | 有効なアプリ一覧 |
| GET | /v1/apps/{app} | アプリ名、割当パス、Release ID、SHA-256 |
| PUT | /v1/apps/{app} | Wasm本体を配備。x-yamanami-pathで割当パスを指定 |

すべての管理操作にAuthorization: Bearerトークンが必要です。

~~~bash
curl -H "Authorization: Bearer $YAMANAMI_TOKEN" \
  -H 'Content-Type: application/wasm' \
  -H 'x-yamanami-path: /hello' \
  --upload-file examples/hello/target/wasm32-wasip2/release/yamanami_hello.wasm \
  http://127.0.0.1:9090/v1/apps/hello
~~~

アプリ名は1〜64文字の英数字・ハイフン・アンダースコアです。割当パスはASCIIの絶対パスとし、空セグメント、ドットセグメント、末尾スラッシュ（ルート以外）は認めません。/helloは/helloと/hello/以下に一致し、/helloworldには一致しません。ゲストには元のURIを渡します。

## テスト

~~~bash
cargo fmt --all -- --check
cargo fmt --manifest-path examples/hello/Cargo.toml -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked

# 実バイナリ・CLI・SQLite・Wasmを使ったE2Eテスト
python3 tests/e2e.py
~~~

E2Eテストはホストとサンプルのv1/v2をビルドし、一時ディレクトリと空きポートを使用します。配備・リクエストの受け渡し・更新・更新失敗・再起動・重複排除・並行処理・実行メモリ分離・管理認証・外向きHTTP拒否・各種制限を確認します。

サンプルの/sleep、/hold、/spin、/memory、/large、/networkは制限確認用のエンドポイントです。通常のアプリに組み込む機能ではありません。

確認した結果と簡易性能測定は[初段の動作確認](docs_dev/first-stage-verification.md)を参照してください。

GitHub Actions用の[CIテンプレート](docs_dev/github-actions-ci.yml)も用意しています。現在のpush用トークンにはworkflow権限がないため、まだ有効化していません。対応する権限で.github/workflows/ci.ymlに配置すると有効になります。性能の本格評価は次の段階とし、リリースビルドで初回コンパイル、応答時間、スループット、p95、RSSを条件付きで測定します。

## 設計と今後

初段のデータモデルはApplication、Release、Moduleを分離します。ModuleはSHA-256で識別し、ReleaseとModuleは対応テーブルで結びます。初段の実行対象はmainの1個ですが、1アプリ＝1 Wasmをデータ構造に固定しません。

HTTP境界には独自ABIを設けず、標準WASI HTTPを使用します。言語間の互換性は「同じWITインターフェースのコンポーネントを実行できること」で、既存Webフレームワークが無変更で動作することを意味しません。現時点で動作確認するゲストはRustのみです。

後続の候補は次のとおりです。

- アプリ専用/dataの永続ディレクトリとファイル権限
- 外向き通信の明示的な許可設定
- 別言語のサンプルとWASI 0.3のasync対応
- 複数モジュール構成、静的ファイルを含む配備、ロールバック
- HTTPS、外部公開、運用向けの制限・監視
- Wasm内部のマルチスレッド、アプリ間通信、分散実行

当初の調査・構想は[開発準備メモ](docs_dev/initial-design.md)に保存しています。現在の実装範囲と操作方法はこのREADMEを参照してください。

## ライセンス

Yamanami — MIT License。全文は[LICENSE](LICENSE)を参照してください。

YamanamiのMITライセンスは、Yamanami上で動くWasmアプリケーションには自動的に適用されません。利用者は商用・非公開を含む独自ライセンスのWasmアプリを配備できます。Yamanami上で実行することだけを理由に、ソースコード公開やMITへの変更を求めることはありません。

アプリと依存ライブラリにはそれぞれのライセンス条件が適用されます。Yamanamiが利用する第三者ライブラリも、それぞれのライセンスに従います。
