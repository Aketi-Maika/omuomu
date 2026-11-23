# NAS の作り方（AlmaLinux 8 ＋ 4TB×2）

この機械を次のように使います。

```
Windows PC ──(エクスプローラー \\NAS\photos)──▶ /srv/nas/photos   写真・写真登録.xlsx を置く
                                                │  4TB×2 ミラーリング（RAID1。片方壊れても残る）
LAN の PC ──(ブラウザ http://NAS:8787/#/admin)──▶ omuomu-gallery（常駐）── 構築・暗号化 ──▶ GitHub ──▶ GitHub Pages
```

対象: AlmaLinux 8.10 / Intel Atom D2701 / メモリ 3.6GB。古い CPU（SSE4.2 なし）でも動く実行ファイルを使います。

---

## 0. 準備（Windows 側）

`写真` フォルダーで一式を作ります（手順書と同じフォルダーにある `package.sh`）。

```
cd F:\github\omuomu\写真
bash nas/package.sh
scp dist-nas/omuomu-nas.tar.gz root@<NAS の IP>:/root/
```

> まだなら先に、`写真` フォルダー一式と `.github/workflows/photo-pages.yml` を GitHub の `2023` ブランチに push しておいてください
> （NAS はそのブランチを取り込んで使います）。元の写真や `写真登録.xlsx` は push しないこと。

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
終わると `/srv/nas` に約 3.6TB の領域ができます。2 台の同期は裏で数時間〜半日かかりますが、その間も使えます（`cat /proc/mdstat` で確認）。

## 2. 共有フォルダーを作る（02-share.sh）

```
bash 02-share.sh          # ユーザー名を変えるなら: bash 02-share.sh 名前
```

途中で共有用のパスワードを 2 回聞かれます。終わったら Windows のエクスプローラーで `\\<NAS の IP>\photos` を開き、
ユーザー名 `photo` とそのパスワードでログインします。ここに次を置きます。

```
\\NAS\photos\
  ├─ 2023写真\        … 写真（サブフォルダーも読み込みます）
  └─ 写真登録.xlsx    … 名簿
```

## 3. 管理ツールを常駐させる（03-gallery.sh）

```
bash 03-gallery.sh
```

1. GitHub に送るための公開鍵が表示されます。GitHub のリポジトリ →「Settings」→「Deploy keys」→「Add deploy key」に貼り付け、
   **「Allow write access」にチェック**して保存し、Enter を押します。
2. 管理画面のパスワード（LAN から開くとき用、8 文字以上）を決めます。
3. 終わると表示される `http://<NAS の IP>:8787/#/admin` を LAN の PC で開きます（ユーザー名は何でも可）。

Windows で使っていた設定（写真の鍵・Google ログインの設定）は引き継がれます。写真の場所だけ設定し直してください。

- 写真の登録元: `/srv/nas/photos/2023写真`
- 名簿: `/srv/nas/photos/写真登録.xlsx`

「保存して構築」→「GitHub に公開」で、今までと同じように公開されます。

> LAN のアドレス（`http://192.168…`）では Google ログインが使えません（Google の制限）。
> 管理画面の表示確認は入口の「管理ツールで確認」から行えます。参加者は GitHub Pages の公開サイトから Google ログインで入ります。

## 更新するとき

Windows で `bash nas/package.sh` → NAS に送って展開 → `bash 03-gallery.sh` をもう一度実行します（設定や鍵はそのまま残ります）。

## 困ったとき

| 症状 | 確認すること |
| --- | --- |
| 管理画面が開かない | `systemctl status omuomu-gallery` / `journalctl -u omuomu-gallery -n 50` |
| 共有フォルダーに入れない | `systemctl status smb`、ユーザー名 `photo`、`smbpasswd -a photo` でパスワード再設定 |
| ディスクの状態 | `cat /proc/mdstat`（`[UU]` なら 2 台とも正常、`[U_]` なら 1 台故障）、`mdadm --detail /dev/md/nas` |
| 「GitHub に公開」が失敗する | `runuser -u omuomu -- ssh -T git@github.com` で「successfully authenticated」と出るか（Deploy key の write 権限） |
| 構築が遅い | Atom では大きな写真（6000×4000）1 枚に数秒かかります。2 回目以降は増えた写真だけ処理します |

## 安全のために

- 管理画面は LAN からパスワード付きで開けます。**ルーターでポート 8787 をインターネットに公開しないでください。**
- `gallery.config.json`（写真の鍵を含む）は `/srv/nas/omuomu-repo/写真/` にあり、管理ツールのユーザーだけが読めます。
- RAID1 はディスク 1 台の故障には耐えますが、誤って消したファイルは戻りません。大事な写真は別の場所にもバックアップしてください。
