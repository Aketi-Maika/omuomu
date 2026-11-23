#!/bin/bash
# 4TB×2 をミラーリング（RAID1）して /srv/nas にマウントする
#   使い方: bash 01-storage.sh /dev/sdX /dev/sdY
#   ※ 指定した 2 台のディスクの中身はすべて消えます。OS が入っているディスクは指定しないこと。
set -euo pipefail

[[ $EUID -eq 0 ]] || { echo "root で実行してください"; exit 1; }

if [[ $# -ne 2 ]]; then
  echo "使い方: bash $0 /dev/sdX /dev/sdY"
  echo
  echo "接続されているディスク（4TB の 2 台を探してください。OS のディスクは選ばないこと）:"
  lsblk -d -o NAME,SIZE,MODEL,SERIAL,TYPE,TRAN
  echo
  lsblk -o NAME,SIZE,FSTYPE,MOUNTPOINT
  exit 1
fi

D1=$1
D2=$2
[[ "$D1" != "$D2" ]] || { echo "同じディスクが 2 回指定されています"; exit 1; }

ROOTDISK=$(lsblk -no PKNAME "$(findmnt -no SOURCE /)" 2>/dev/null || true)
for d in "$D1" "$D2"; do
  [[ -b "$d" ]] || { echo "$d が見つかりません"; exit 1; }
  [[ "$(lsblk -dno TYPE "$d")" == "disk" ]] || { echo "$d はディスク全体を指定してください（例: /dev/sdb。sdb1 ではない）"; exit 1; }
  if [[ -n "$ROOTDISK" && "$(basename "$d")" == "$ROOTDISK" ]]; then
    echo "$d は OS が入っているディスクです。中止します"; exit 1
  fi
  if lsblk -no MOUNTPOINT "$d" | grep -q .; then
    echo "$d の中にマウント中の領域があります（使用中のディスクでは？）。中止します"; exit 1
  fi
  if command -v pvs >/dev/null && pvs --noheadings -o pv_name 2>/dev/null | grep -q "^ *$d"; then
    echo "$d は LVM で使用中です。中止します"; exit 1
  fi
  if grep -q "$(basename "$d")" /proc/mdstat 2>/dev/null; then
    echo "$d はすでに RAID で使用中です。中止します"; exit 1
  fi
  SIZE=$(lsblk -bdno SIZE "$d")
  if (( SIZE < 1000000000000 )); then
    echo "$d は 1TB 未満です（$(lsblk -dno SIZE "$d")）。4TB のディスクか確認してください"; exit 1
  fi
done

echo "次の 2 台を RAID1 にします。中身はすべて消去されます:"
lsblk -o NAME,SIZE,MODEL,SERIAL,FSTYPE "$D1" "$D2"
echo
read -r -p "続けるには ERASE と入力してください: " ans
[[ "$ans" == "ERASE" ]] || { echo "中止しました"; exit 1; }

dnf -y install mdadm xfsprogs parted smartmontools

for d in "$D1" "$D2"; do
  wipefs -a "$d"
  parted -s "$d" mklabel gpt mkpart nas 1MiB 100% set 1 raid on
done
partprobe "$D1" "$D2" || true
udevadm settle

P1=$(lsblk -lnpo NAME,TYPE "$D1" | awk '$2=="part"{print $1; exit}')
P2=$(lsblk -lnpo NAME,TYPE "$D2" | awk '$2=="part"{print $1; exit}')
[[ -n "$P1" && -n "$P2" ]] || { echo "パーティションを作れませんでした"; exit 1; }

mdadm --create /dev/md/nas --level=1 --raid-devices=2 --metadata=1.2 --name=nas --run "$P1" "$P2"
mkfs.xfs -f -L nas /dev/md/nas

# 起動時に自動で組み立て・マウント
{
  echo "MAILADDR root"
  mdadm --detail --scan
} > /etc/mdadm.conf
mkdir -p /srv/nas
UUID=$(blkid -s UUID -o value /dev/md/nas)
grep -q "$UUID" /etc/fstab || echo "UUID=$UUID  /srv/nas  xfs  defaults,nofail  0 0" >> /etc/fstab
systemctl daemon-reload
mount /srv/nas

# ディスクの故障を監視（root 宛てのメールに通知。journalctl でも確認できます）
systemctl enable --now mdmonitor smartd || true

echo
echo "完了しました: /srv/nas（RAID1）"
df -h /srv/nas
echo
echo "いまディスク同士の同期（数時間〜半日）が裏で進んでいます。その間も使えます。進み具合:"
echo "  cat /proc/mdstat"
