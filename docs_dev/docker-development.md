# Docker実行環境の準備

2026-10-25。開発はホスト、実行はDockerとする利用者の指示。ルートDOCKER.mdを利用者向け原本とする。

公式ベースイメージのタグ・digestの存在をレジストリで確認。Compose v5.6.1の公式バイナリをSHA-256検証して取得し、config --quietが通過。シェル構文、Rustの待受境界テスト、実サーバーでのbootstrap/同一ハッシュ省略/差分更新/停止のテストも通過。

このUbuntu 24.04にはDocker CLI・デーモンがなく、uidmapもない。セッションの書込権限はworkspaceと/tmpに限定されるため、管理者による導入スクリプト実行が必要。Dockerイメージの実ビルド・コンテナ起動・ポート公開・volume上のUID権限は導入後の未検証事項。

管理APIは127.0.0.1のまま、アプリHTTPだけを明示オプションで0.0.0.0へbind。Composeはホスト127.0.0.1のHTTPのみをpublishする。トークンはvolume内へ生成し、標準出力へ表示しない。dockerグループへの追加とソケット権限変更は行わない。

最終確認：本体単体3件・既存E2E6件・Clippy・エントリーポイントの実サーバーテストが通過。WASI SDKとwit-bindgen配布バイナリのglibc要求もDebian bookwormの範囲内であることを確認。Docker実行は管理者による導入待ち。
