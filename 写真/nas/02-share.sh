#!/bin/bash
# Windows から写真を置ける共有フォルダー \\<NAS の IP>\photos を作る
#   使い方: bash 02-share.sh [共有にログインするユーザー名（既定 photo）]
set -euo pipefail

[[ $EUID -eq 0 ]] || { echo "root で実行してください"; exit 1; }
mountpoint -q /srv/nas || { echo "/srv/nas がマウントされていません。先に 01-storage.sh を実行してください"; exit 1; }

SMBUSER=${1:-photo}

dnf -y install samba samba-client policycoreutils-python-utils

groupadd -f nasusers
if ! id "$SMBUSER" >/dev/null 2>&1; then
  useradd -M -s /sbin/nologin -G nasusers "$SMBUSER"
else
  usermod -aG nasusers "$SMBUSER"
fi

mkdir -p /srv/nas/photos
chgrp nasusers /srv/nas/photos
chmod 2775 /srv/nas/photos

# SELinux: Samba がこのフォルダーを読み書きできるようにする
semanage fcontext -a -t samba_share_t "/srv/nas/photos(/.*)?" 2>/dev/null || semanage fcontext -m -t samba_share_t "/srv/nas/photos(/.*)?"
restorecon -R /srv/nas/photos

if ! grep -q '^\[photos\]' /etc/samba/smb.conf; then
  cp -a /etc/samba/smb.conf "/etc/samba/smb.conf.bak.$(date +%Y%m%d%H%M%S)"
  cat >> /etc/samba/smb.conf <<'CONF'

[photos]
	comment = 写真（omuomu 写真展）
	path = /srv/nas/photos
	valid users = @nasusers
	writable = yes
	force group = nasusers
	create mask = 0664
	directory mask = 2775
	server min protocol = SMB2_10
CONF
fi
testparm -s >/dev/null

echo
echo "Windows から共有にログインするときのパスワードを設定します（ユーザー名: $SMBUSER）"
smbpasswd -a "$SMBUSER"

firewall-cmd --permanent --add-service=samba
firewall-cmd --reload
systemctl enable --now smb

IP=$(hostname -I | awk '{print $1}')
echo
echo "完了しました。Windows のエクスプローラーのアドレス欄に次を入力すると開けます:"
echo "  \\\\$IP\\photos   （ユーザー名 $SMBUSER）"
echo "ここに写真フォルダー（例: 2023写真）と 写真登録.xlsx を置いてください。"
