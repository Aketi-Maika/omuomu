#!/bin/bash
# 管理ツール omuomu-gallery を NAS に入れて常駐させる（2 回目以降は更新）
#   使い方: このファイルと同じフォルダー（omuomu-nas を展開した場所）で bash 03-gallery.sh
#   変更したいときは環境変数で: BRANCH=main REPO=git@github.com:xxx/yyy.git bash 03-gallery.sh
set -euo pipefail

[[ $EUID -eq 0 ]] || { echo "root で実行してください"; exit 1; }
mountpoint -q /srv/nas || { echo "/srv/nas がマウントされていません。先に 01-storage.sh を実行してください"; exit 1; }

HERE=$(cd "$(dirname "$0")" && pwd)
BRANCH=${BRANCH:-2023}
REPO=${REPO:-git@github.com:Aketi-Maika/omuomu.git}
SVC_USER=omuomu
SVC_HOME=/var/lib/omuomu
CLONE=/srv/nas/omuomu-repo
ROOT="$CLONE/写真"
PORT=8787

[[ -f "$HERE/omuomu-gallery" && -d "$HERE/web-dist" ]] || { echo "omuomu-gallery と web-dist が見つかりません。omuomu-nas を展開したフォルダーで実行してください"; exit 1; }

dnf -y install git openssh-clients

groupadd -f nasusers
if ! id "$SVC_USER" >/dev/null 2>&1; then
  useradd -r -m -d "$SVC_HOME" -s /bin/bash -G nasusers "$SVC_USER"
else
  usermod -aG nasusers "$SVC_USER"
fi

install -D -m 755 "$HERE/omuomu-gallery" /opt/omuomu/bin/omuomu-gallery

# --- GitHub に送るための鍵（このリポジトリ専用の Deploy key）
as_svc() { runuser -u "$SVC_USER" -- env HOME="$SVC_HOME" "$@"; }
if [[ ! -f "$SVC_HOME/.ssh/id_ed25519" ]]; then
  as_svc mkdir -p "$SVC_HOME/.ssh"
  chmod 700 "$SVC_HOME/.ssh"
  as_svc ssh-keygen -q -t ed25519 -N "" -C "omuomu-nas" -f "$SVC_HOME/.ssh/id_ed25519"
fi
as_svc bash -c "ssh-keyscan -t ed25519 github.com 2>/dev/null >> '$SVC_HOME/.ssh/known_hosts'; sort -u -o '$SVC_HOME/.ssh/known_hosts' '$SVC_HOME/.ssh/known_hosts'"

until as_svc ssh -o BatchMode=yes -T git@github.com 2>&1 | grep -q "successfully authenticated"; do
  echo
  echo "=== GitHub に次の公開鍵を登録してください ==="
  cat "$SVC_HOME/.ssh/id_ed25519.pub"
  echo "登録先: GitHub のリポジトリ → Settings → Deploy keys → Add deploy key"
  echo "        Title は omuomu-nas など、「Allow write access」に必ずチェック"
  read -r -p "登録したら Enter（やめるときは Ctrl+C）: " _
done
echo "GitHub に接続できました"

# --- リポジトリ
if [[ ! -d "$CLONE/.git" ]]; then
  install -d -o "$SVC_USER" -g "$SVC_USER" "$CLONE"
  as_svc git clone --branch "$BRANCH" "$REPO" "$CLONE"
else
  as_svc git -C "$CLONE" pull --ff-only || echo "注意: git pull できませんでした（手元に未送信の変更がある可能性）"
fi
as_svc git -C "$CLONE" config user.name "omuomu NAS"
as_svc git -C "$CLONE" config user.email "omuomu-nas@users.noreply.github.com"
[[ -d "$ROOT/crates" ]] || { echo "$ROOT がありません。$BRANCH ブランチに 写真 フォルダーを push してから再実行してください"; exit 1; }

# --- 画面（web/dist）と設定
rm -rf "$ROOT/web/dist"
cp -r "$HERE/web-dist" "$ROOT/web/dist"
if [[ -f "$HERE/gallery.config.json" && ! -f "$ROOT/gallery.config.json" ]]; then
  cp "$HERE/gallery.config.json" "$ROOT/gallery.config.json"
  echo "Windows の設定（写真の鍵・Google ログインの設定を含む）を引き継ぎました"
fi
chown -R "$SVC_USER:$SVC_USER" "$ROOT/web/dist"
[[ -f "$ROOT/gallery.config.json" ]] && chown "$SVC_USER:$SVC_USER" "$ROOT/gallery.config.json" && chmod 600 "$ROOT/gallery.config.json"

# --- LAN から開くときのパスワード
if ! grep -q '"lanPassword": *"[^"]*\$' "$ROOT/gallery.config.json" 2>/dev/null; then
  echo
  while true; do
    read -r -s -p "管理画面（LAN から開く）のパスワードを決めてください（8 文字以上）: " P1; echo
    read -r -s -p "もう一度: " P2; echo
    [[ "$P1" == "$P2" ]] && (( ${#P1} >= 8 )) && break
    echo "一致しないか、8 文字未満です"
  done
  printf '%s\n' "$P1" | as_svc /opt/omuomu/bin/omuomu-gallery --root "$ROOT" set-password
  chmod 600 "$ROOT/gallery.config.json"
fi

# --- 常駐（systemd）
sed -e "s#@ROOT@#$ROOT#g" -e "s#@PORT@#$PORT#g" "$HERE/omuomu-gallery.service" > /etc/systemd/system/omuomu-gallery.service
systemctl daemon-reload
systemctl enable omuomu-gallery
systemctl restart omuomu-gallery

firewall-cmd --permanent --add-port=$PORT/tcp
firewall-cmd --reload

sleep 2
systemctl --no-pager --lines=5 status omuomu-gallery || true
IP=$(hostname -I | awk '{print $1}')
echo
echo "完了しました。LAN の PC のブラウザで次を開いてください（ユーザー名は何でも可、パスワードは今決めたもの）:"
echo "  http://$IP:$PORT/#/admin"
echo "設定画面の写真フォルダーは /srv/nas/photos/（フォルダー名）、名簿は /srv/nas/photos/写真登録.xlsx のように指定します。"
