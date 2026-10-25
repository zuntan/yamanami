# 4 Languages, 4 Applications, 1 Yamanami.

Rust・C++・Python・TypeScriptの電卓を、同じYamanami上で独立したアプリとして動かすデモです。ディレクトリ名は利用者の指定により **exsample/** としています。既存の examples/hello は本体の最小検証用です。

indexもRust製WasmでHTMLを返します。4つの電卓はiframeに表示され、各フォームのPOSTを各言語のWasmが処理します。ブラウザのJavaScriptで計算するコードはありません。

| アプリ | 割当パス | 言語・テンプレート | Wasm内部のランタイム |
| --- | --- | --- | --- |
| index | / | Rust 1.95.0・MiniJinja 2.24.0 | Rust / WASI |
| calculator-rust | /rust | Rust 1.95.0・MiniJinja 2.24.0 | Rust / WASI |
| calculator-cpp | /cpp | C++20・kainjow/Mustache 4.1 | WASI SDK 34のlibc・libc++ |
| calculator-python | /python | Python・Chevron 0.14.0 | componentize-py 0.25.0内のCPython 3.14.0 |
| calculator-typescript | /typescript | TypeScript 7.0.2・Mustache.js 4.2.0 | ComponentizeJS 0.23.0 / StarlingMonkey |

すべてWASI HTTP 0.2.12のincoming-handlerを公開します。Rustのwasiクレートは0.14.7を入口に、ロックされたwasip2 1.0.4の0.2.12バインディングを使います。TypeScriptはJSへ変換・バンドルした後、JSエンジンと一緒にWasmへまとめます。実行ホストのNode.jsやPythonインストールには依存しません。

## 準備・ビルド

現在のセットアップスクリプトは **Linux x86_64、Python 3.12以降、Node.js 24、npm、Rustup、C++コンパイラ**を前提とします。C++コンパイラはネイティブ単体テストにも使用します。新規環境でPythonのvenvが使用できない場合は、OSのvenvパッケージを先に用意してください。

リポジトリのルートから実行します。

~~~bash
python3 exsample/scripts/setup.py
cargo build --locked
python3 exsample/scripts/build.py
~~~

setup.pyは固定版のWASI SDK 34とwit-bindgen 0.63.0を公式配布から取得し、SHA-256を検証してexsample/.tools/へ展開します。Pythonは専用venv、npmはpackage-lock.jsonで準備します。OS全体のPython環境は変更しません。

ビルド時は必要なツール・パッケージのダウンロードが発生します。ホストが大きなWasmをコンパイルする時間を減らすため、開発プロファイルでもコンパイラの主要依存を最適化しており、初回のcargo buildには時間がかかります。

ツールの保存先を変更する場合は、両方のスクリプトに同じ引数を渡します。

~~~bash
python3 exsample/scripts/setup.py --tools-dir /tmp/yamanami-tools
python3 exsample/scripts/build.py --tools-dir /tmp/yamanami-tools
~~~

生成物はexsample/build/です。各WasmにHTTP処理、計算、テンプレート処理、HTML/CSSと必要なランタイムを含みます。実行時にテンプレートファイルや設定ファイル、/dataは不要です。生成物とダウンロードしたツールはGit管理しません。

## サーバー起動

PythonのWasmは約18 MiBなので、初段の既定アップロード上限16 MiBを拡張する必要があります。デモ用の有限な設定例です。

~~~bash
export YAMANAMI_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
export RAYON_NUM_THREADS=4

target/debug/yamanami serve \
  --database var/calculators.sqlite3 \
  --upload-limit 33554432 \
  --compiler-opt-level none \
  --memory-mib 256 \
  --max-memories 8 --max-instances 128 --max-tables 32 \
  --fuel 1000000000 --timeout-ms 10000 \
  --concurrency 4 --app-concurrency 1
~~~

コンパイラのnoneは初回コンパイルの最適化を省く設定です。speedとの比較は本デモの主目的ではありません。既定はspeedのままです。RAYON_NUM_THREADSはコンパイル用スレッド数で、アプリ実行の同時実行数とは別です。

memory-mibは線形メモリ1個あたりの上限です。複数メモリを許可した場合の上限は個数分になり、ホストのコンパイルキャッシュやJS/Python以外のホスト資源を含むRSS上限ではありません。

これらは動作確認用の余裕を持たせた設定で、各ランタイムに必要な最小値を意味しません。管理・一般HTTPともloopback限定です。外向き通信・ファイル権限は追加しません。

## 配備・操作

別の端末に同じYAMANAMI_TOKENを設定し、一括配備します。

~~~bash
export YAMANAMI_TOKEN='サーバーと同じ値'
python3 exsample/scripts/deploy.py
~~~

http://127.0.0.1:8080/ を開くと4つの電卓が並びます。狭い画面は1列になります。各電卓は直接開くこともできます。

~~~bash
curl http://127.0.0.1:8080/python/

curl -X POST http://127.0.0.1:8080/typescript/ \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data 'a=6&op=mul&b=7'
~~~

演算子はadd / sub / mul / divです。有限の数値を受け付け、ゼロ除算・不正入力・結果のオーバーフローは400とエラー表示を返します。任意の式をevalしません。HTMLエスケープは各テンプレートライブラリを利用します。

入力は倍精度浮動小数点です。金融用途の十進数の厳密計算を意図したものではなく、表示の丸め方には言語ごとの差があります。

一括配備スクリプトも内部では5回のyamanami ctl deployを実行します。5アプリ全体をまとめる原子的な配備ではありません。

## 独立した更新

Rust電卓だけをv2としてビルド・更新する例です。

~~~bash
python3 exsample/scripts/build.py --language rust --rust-v2

target/debug/yamanami ctl deploy \
  exsample/build/calculator-rust.wasm \
  --app calculator-rust --path /rust
~~~

画面のdemo-v2表示で更新を確認できます。他アプリへの配備操作は行いません。v1へ戻す場合は--rust-v2を付けずにビルドし、同じアプリへ再配備します。

他言語も--language cpp / python / typescriptで個別ビルドできます。各アプリのバイナリは独立しています。サーバーを同じDBで再起動すると復元されます。

## テスト

~~~bash
# 4言語それぞれの計算単体テスト
python3 exsample/scripts/test_unit.py

# ブラウザ操作テスト用のChromium
cd exsample/typescript
npx playwright install chromium
cd ../..

# 全Wasmを事前にビルドしてから、HTTP・iframe・独立更新・再起動を検証
python3 exsample/scripts/test_http.py
~~~

test_http.pyは一時DBと空きポートを使用し、CLIから5アプリを配備します。四則演算、エラー、HTMLエスケープ、クリア、並行リクエスト、Rustだけの更新、再起動復元を確認します。さらに独立したheadless Chromiumで4つのiframeを実際に操作します。実行にはChromiumのOS依存パッケージも必要です。

テスト後のexsample/build/calculator-rust.wasmはv2になります。全体をv1にそろえる場合はbuild.pyを再実行してください。

ブラウザ検証の画像はexsample/build/demo-desktop.pngとdemo-mobile.pngです。起動済みサーバーに対してブラウザ検証だけを行うこともできます。

~~~bash
node exsample/scripts/test_browser.mjs http://127.0.0.1:8080
~~~

## 構成・選定の補足

- Rust：MiniJinjaへ埋め込み文字列を登録し、HTMLエスケープを明示します。
- C++：テンプレートと計算はC++、HTTP境界だけwit-bindgenが生成する標準Cバインディングを使います。独自ABIはありません。C++バインディングの標準ライブラリ差への依存を避けるため、この方式を選びました。
- Python：Chevronとテンプレートをビルド時に取り込み、実行時のpartialファイル読込は使いません。C拡張のテンプレートライブラリは追加していません。
- TypeScript：同梱エンジンとfetch-eventのバージョン差を避け、標準WASI HTTPリソースを直接操作するincomingHandlerを公開します。計算とMustache描画はWasm内のJSエンジンで実行します。

[生成物のサイズと検証結果](docs/verification.md)、[第三者ソースとライセンス](docs/THIRD_PARTY.md)、[検討用の机](docs/yamanami_exsample.tabletop)を参照してください。
