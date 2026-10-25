# 多言語サンプル開発記録

## 2026-10-25 — 着手

利用者の指示により、サンプルは exsample/ に配置する（綴りを維持）。既存の examples/hello は本体の最小検証用として残す。

対象はRust製indexと、Rust・C++・Python・TypeScriptで実装した独立した電卓の計5アプリ。単一のWASI HTTP 0.2コンポーネントとして配備する。四則演算は各言語のサーバー側で行い、HTMLフォームで操作する。

選定案：Rust/MiniJinja、C++/kainjow Mustache + WASI SDK、Python/Chevron + componentize-py、TypeScript/Mustache.js + ComponentizeJS。

先にPython・TypeScriptの最小HTTPを確認する。現在のホスト制限（アップロード16MiB、memory1個、instance32個、メモリ64MiB、fuel1,000万）が必要なランタイムと適合するかを測定し、必要な上限を有限の設定値として扱う。独自ランタイムや複雑な互換層が必要なら追加前に相談する。

各言語の計算単体テストと、5アプリのHTTP・独立更新テストを用意する。選定ライブラリとツールチェーンの正確な版、サイズ、測定・テスト結果は実際の成果物に基づいて追記する。
