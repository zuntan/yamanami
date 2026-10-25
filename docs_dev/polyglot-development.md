# 多言語サンプル開発記録

## 2026-10-25 — 着手

利用者の指示により、サンプルは exsample/ に配置する（綴りを維持）。既存の examples/hello は本体の最小検証用として残す。

対象はRust製indexと、Rust・C++・Python・TypeScriptで実装した独立した電卓の計5アプリ。単一のWASI HTTP 0.2コンポーネントとして配備する。四則演算は各言語のサーバー側で行い、HTMLフォームで操作する。

選定案：Rust/MiniJinja、C++/kainjow Mustache + WASI SDK、Python/Chevron + componentize-py、TypeScript/Mustache.js + ComponentizeJS。

先にPython・TypeScriptの最小HTTPを確認する。現在のホスト制限（アップロード16MiB、memory1個、instance32個、メモリ64MiB、fuel1,000万）が必要なランタイムと適合するかを測定し、必要な上限を有限の設定値として扱う。独自ランタイムや複雑な互換層が必要なら追加前に相談する。

各言語の計算単体テストと、5アプリのHTTP・独立更新テストを用意する。選定ライブラリとツールチェーンの正確な版、サイズ、測定・テスト結果は実際の成果物に基づいて追記する。

## 実装・検証結果

5アプリを exsample/ 配下に実装。すべて単一のWASI HTTP 0.2.12コンポーネントを生成できた。各言語の単体テスト、HTTPの全演算・異常系、headless Chromium内の4iframe操作、Rustだけの更新、再起動復元が通過。詳細は ../exsample/docs/verification.md 。

TypeScriptはfetch-eventのexport生成の問題を避け、標準WASI HTTPのincomingHandlerを直接実装。独自ランタイム・独自ABIは追加していない。C++も標準Cバインディングを境界に使い、計算・Mustache描画はC++で実装した。

Pythonの成果物は約18 MiBで初段の16 MiBを超えたため、アップロード上限を設定可能にした。memory/instance/table個数も有限な設定にし、既定値は維持。デモでは余裕を持たせた値で確認し、最小要件とは区別した。

初回コンパイルの長さに対し、Wasmtimeのparallel-compilation、最適化レベル選択、開発プロファイルの主要コンパイラ依存の最適化を導入。本体の単体2件・既存E2E6件は通過した。

ブラウザテストではPythonのクリア後の遷移完了を待たずに旧フォームを読んだため一度失敗。空のvalue属性を持つ新フォームの出現を待つよう修正し、4言語すべてで通過した。

今後：各ランタイムに必要な最小制限値の計測、より長い性能測定、別OSでのツールセットアップ検証。現在のsetup.pyはLinux x86_64向け。
