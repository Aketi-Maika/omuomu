# NAS の作り方（AlmaLinux 8 ＋ 4TB×2）

この機械が写真展の「中身」をすべて持ちます。GitHub Pages には画面のプログラムと表示用の文言しか置きません。

```
Windows PC ──(エクスプローラー \\NAS\photos)──▶ /srv/nas/photos      写真・写真登録.xlsx を置く
                                                │  4TB×2 ミラーリング（RAID1）
                                                ▼
                              omuomu-gallery（NAS の中だけで常駐）
                                ├─ 管理画面  127.0.0.1:8787 ◀── PC から SSH トンネル（暗号化）
                                └─ 公開 API  127.0.0.1:8788 ◀── Tailscale Funnel / Cloudflare Tunnel（https）
                                                                      ▲
参加者のブラウザ ── GitHub Pages の画面 ── Google ログイン ───────────┘
                    写真は NAS がログインを確かめて 1 枚ずつ返す。回答・ログは NAS に保存
                    （任意で Google スプレッドシートにも写す）
```

- 写真・名簿・回答・ログは NAS の外に出ません（`/srv/nas/omuomu-repo/写真/.private/`）。
- 誰が入れるかは NAS が毎回の通信で確かめます。管理画面で入場を止めたアカウントは、次の操作から入れなくなります。
- ルーターのポートは開けません（トンネルは NAS から外へつなぐ方式です）。

対象: AlmaLinux 8.10 / Intel Atom D2701 / メモリ 3.6GB。古い CPU（SSE4.2 なし）でも動く実行ファイルを使います。

---

## 0. 準備（Windows 側）

`写真` フォルダーで一式を作ります。

```
cd F:\github\omuomu\写真
bash nas/package.sh
scp dist-nas/omuomu-nas.tar.gz root@<NAS の IP>:/root/
```

NAS 側で展開します。

```
cd /root
tar xzf omuomu-nas.tar.gz
cd omuomu-nas
```

## 1. ディスクをミラーリングする（01-storage.sh）

**指定した 2 台の中身はすべて消えます。** まず引数なしで実行して、4TB のディスク名を確認します。

```
bash 01-storage.sh
```

`SIZE` が `3.6T` の 2 台（例: `sdb` と `sdc`）を確認してから:

```
bash 01-storage.sh /dev/sdb /dev/sdc
```

`ERASE` と入力すると始まります。OS のディスク・使用中のディスク・1TB 未満のディスクは指定できないようにしてあります。
2 台の同期は裏で数時間〜半日かかりますが、その間も使えます（`cat /proc/mdstat` で確認）。

## 2. 共有フォルダーを作る（02-share.sh）

```
bash 02-share.sh          # ユーザー名を変えるなら: bash 02-share.sh 名前
```

Windows のエクスプローラーで `\\<NAS の IP>\photos` を開き（ユーザー名 `photo`）、次を置きます。

```
\\NAS\photos\
  ├─ 2023写真\        … 写真（サブフォルダーも読み込みます）
  └─ 写真登録.xlsx    … 名簿（Excel で編集。構築のたびに読み込み直します）
```

## 3. 管理ツールを常駐させる（03-gallery.sh）

```
bash 03-gallery.sh
```

1. GitHub に送るための公開鍵が表示されます。GitHub のリポジトリ →「Settings」→「Deploy keys」→「Add deploy key」に貼り付け、
   **「Allow write access」にチェック**して保存し、Enter を押します（GitHub に送るのは表示用の `public.json` だけです）。
2. 管理画面は **NAS の中だけ** で動きます。PC から開くときは、PC の PowerShell で SSH トンネルを張ります。

   ```
   ssh -N -L 8787:localhost:8787 root@<NAS の IP>
   ```

   この窓を開いたまま、ブラウザで `http://localhost:8787/#/admin` を開きます。通信は SSH で暗号化され、
   アドレスが localhost なので Google ログインも使えます。使い終わったら Ctrl+C。
   （同じ PC で Windows 版の管理ツールを動かしている場合は、先に止めてください）
3. 「設定」で写真の場所を指定し、「保存して構築」を押します。
   - 写真の登録元: `/srv/nas/photos/2023写真`
   - 名簿: `/srv/nas/photos/写真登録.xlsx`

## 4. 公開 API を外に出す（04-tunnel.sh）

参加者のブラウザが写真を受け取れるよう、NAS の公開 API（127.0.0.1:8788）だけを https で外に出します。

### Tailscale Funnel を使う場合（独自ドメイン不要・無料）

```
bash 04-tunnel.sh
```

Tailscale のアカウント作成・ログインと、Funnel の利用許可を求められます。終わると
`https://<NAS の名前>.<tailnet 名>.ts.net` のような URL が表示されます。

### Cloudflare Tunnel を使う場合（独自ドメインを Cloudflare で管理している場合）

Cloudflare のダッシュボード →「Zero Trust」→「Networks」→「Tunnels」でトンネルを作り、表示されるコマンドで
NAS に `cloudflared` を入れます。「Public Hostname」に `photos.example.com` → `http://127.0.0.1:8788` を登録します。
**8787（管理画面）は登録しないでください。**

### 最後に

管理画面の「設定」→「写真の配信」で次を入れて保存し、「GitHub に公開」を押します。

- 公開 API の URL: 上で表示された `https://…`（「接続を確認」で ✓ が出ること）
- 公開 API を呼んでよいサイト: `https://aketi-maika.github.io`

## 更新するとき

Windows で `bash nas/package.sh` → NAS に送って展開 → `bash 03-gallery.sh` をもう一度実行します（設定や写真はそのまま残ります）。

## 困ったとき

| 症状 | 確認すること |
| --- | --- |
| 管理画面が開かない | SSH トンネルの窓が開いているか。NAS で `systemctl status omuomu-gallery` / `journalctl -u omuomu-gallery -n 50` |
| 公開サイトで写真が出ない | NAS で `curl http://127.0.0.1:8788/pub/health`、外から `https://<公開 API>/pub/health`。`tailscale funnel status` |
| 共有フォルダーに入れない | `systemctl status smb`、ユーザー名 `photo`、`smbpasswd -a photo` でパスワード再設定 |
| ディスクの状態 | `cat /proc/mdstat`（`[UU]` なら 2 台とも正常、`[U_]` なら 1 台故障）、`mdadm --detail /dev/md/nas` |
| 「GitHub に公開」が失敗する | `runuser -u omuomu -- ssh -T git@github.com` で「successfully authenticated」と出るか（Deploy key の write 権限） |
| 構築が遅い | Atom では大きな写真（6000×4000）1 枚に数秒かかります。2 回目以降は増えた写真だけ処理します |

## 安全のために

- インターネットに出るのは公開 API（8788）だけです。公開 API はログイン（NAS が Google に確かめたもの）がない通信には何も返さず、
  管理用の操作（設定・構築・公開）は持っていません。
- 管理画面（8787）は NAS の中だけで待ち受け、SSH トンネル経由でのみ開けます。
- `gallery.config.json`（ログインの署名に使う秘密を含む）と `.private/`（写真・名簿・回答）は
  `/srv/nas/omuomu-repo/写真/` にあり、git には入りません。
- NAS が止まると公開サイトで写真が見られなくなります。
- RAID1 はディスク 1 台の故障には耐えますが、誤って消したファイルは戻りません。大事な写真は別の場所にもバックアップしてください。
