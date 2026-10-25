# DockerでYamanamiと多言語デモを試す

ホストに必要なのはGit、Docker Engine、Docker Composeです。Rust・Python・Node.js・WASI SDKはDockerのビルド段階で用意します。実行イメージにはYamanamiと5本のWasmを入れます。

現在の実装は experiment/first-stage ブランチにあります。デモはLinux amd64向けです。Dockerコンテナでの実ビルド・起動確認は、Docker Engine導入後に行います。現時点ではCompose設定検証、エントリーポイントの実サーバーテスト、本体のテストまで確認済みです。

## 1. clone

~~~bash
git clone --branch experiment/first-stage https://github.com/zuntan/yamanami.git
cd yamanami
~~~

すでにclone済みなら、そのディレクトリで実行します。未コミットの変更がある場合は、先に整理してください。

~~~bash
git switch experiment/first-stage
git pull --ff-only
~~~

## 2. UbuntuへDockerを導入する（初回のみ）

DockerとComposeが使える場合はこの節を飛ばします。

Ubuntu向けの導入スクリプトを用意しています。Docker公式aptリポジトリ、Engine、Buildx、Composeプラグインを導入し、サービスを起動してhello-worldで確認します。

~~~bash
# 内容を確認してから実行
cat scripts/install-docker-ubuntu.sh
sudo bash scripts/install-docker-ubuntu.sh
~~~

管理者権限が必要です。このスクリプトは既存の競合パッケージを勝手に削除せず、検出した場合は停止します。dockerグループへの追加やソケット権限の変更は行いません。以降はsudo dockerで操作します。すでにDocker操作権限を設定済みの場合はsudoを省略できます。

[Docker公式のUbuntu導入手順](https://docs.docker.com/engine/install/ubuntu/)

## 3. ビルドして起動する

~~~bash
sudo docker compose up --build -d --wait --wait-timeout 600
~~~

初回は公式イメージ、Rust依存、言語ツールチェーンをダウンロードしてビルドするため時間がかかります。--waitの起動待ち時間はビルド時間とは別です。

準備が完了したら、ブラウザで開きます。

**http://127.0.0.1:8080/**

表示するアプリ：

| パス | 内容 |
| --- | --- |
| / | Rust製トップページ、4つのiframe |
| /rust/ | Rust + MiniJinja |
| /cpp/ | C++ + kainjow/Mustache |
| /python/ | Python + Chevron |
| /typescript/ | TypeScript + Mustache.js |

入力・演算子を選んで「計算する」を押すと、各言語のWasmで計算してHTMLを返します。「クリア」は入力と結果を消します。

8080番が使用中の場合は、たとえば次のように変更します。

~~~bash
sudo env YAMANAMI_HTTP_PORT=8181 docker compose up --build -d --wait --wait-timeout 600
~~~

この場合は http://127.0.0.1:8181/ を開きます。以降のCompose操作でも同じ環境変数を指定してください。

## 4. git pull後、実行側を更新する

~~~bash
git pull --ff-only
sudo docker compose up --build -d --wait --wait-timeout 600
~~~

これでYamanami本体とexsampleを再ビルドし、新しいイメージのコンテナへ置き換えます。データベースと管理トークンは名前付きボリュームに残します。

起動時に、同梱WasmのSHA-256と現在の配備情報を比較します。

- 初回：5アプリを自動配備
- SHA-256とパスが同じ：配備を省略し、Releaseを増やさない
- 変わったアプリ：そのアプリだけ新しいReleaseへ更新

5アプリ全体をまとめる原子的な更新ではありません。また、コンテナを置き換える間は停止時間があります。将来DB形式が変わる場合の移行処理は、別途対応が必要です。

単なる再起動ならビルド不要です。

~~~bash
sudo docker compose restart
~~~

## 5. 状態・ログ・管理操作

~~~bash
sudo docker compose ps
sudo docker compose logs -f --tail 100 demo

# コンテナ内の管理APIを使う（トークンを表示する必要はない）
sudo docker compose exec demo yamanami-entrypoint ctl list
sudo docker compose exec demo yamanami-entrypoint ctl info calculator-python
~~~

最後に「Yamanami demo is ready」が出ると5アプリの配備確認が完了しています。ヘルスチェックも管理APIと準備完了マーカーを確認します。

管理APIの9090番はホストへ公開しません。一般HTTPだけをホストの127.0.0.1へ公開します。管理トークンは初回起動時に生成し、ボリューム内のadmin-tokenへ所有者のみ読める権限で保存します。

コンテナ内のYamanamiは非rootユーザーで動きます。実行コンテナはCPU 2・メモリ4 GiB・PID数256を上限とし、書込み先をデータボリュームと/tmpに限定しています。これらはWasm内部の制限とは別の上限です。

## 6. 停止する

~~~bash
sudo docker compose down
~~~

通常のdownでは名前付きボリュームを残すため、次のupで配備情報を復元します。

**down -vはデータベースと管理トークンを含むボリュームを削除します。** 初期化したい場合だけ使用してください。

## 開発とテスト

ホストのこのディレクトリで開発し、Dockerで実行できます。ソースを変更した後も、同じup --buildで反映します。Dockerfileのマルチステージビルドとキャッシュで、ビルド環境と実行環境を分けています。

Docker Engineなしで確認できる項目：

~~~bash
cargo test --locked
python3 tests/e2e.py
python3 tests/container-entrypoint.py
~~~

エントリーポイントのテストは本物のYamanamiを使い、初回5件配備、同一Wasmでの再起動時にRelease不変、変更した1件だけの更新、トークン維持、停止処理を確認します。

Docker導入後の確認：

~~~bash
sudo docker compose config --quiet
sudo docker compose up --build -d --wait --wait-timeout 600
curl -f http://127.0.0.1:8080/
curl -f -X POST http://127.0.0.1:8080/python/ \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data 'a=6&op=mul&b=7'
~~~

各言語の開発・個別ビルド手順は[exsample/README.md](exsample/README.md)、本体の概要は[README.md](README.md)を参照してください。
