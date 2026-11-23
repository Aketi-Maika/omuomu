#!/bin/bash
# 公開 API（NAS の 127.0.0.1:8788）を Tailscale Funnel でインターネットに https で出す
#   ルーターのポートを開ける必要はありません（NAS から外へつなぐ方式）。
#   使い方: bash 04-tunnel.sh
#   独自ドメインがあり Cloudflare を使う場合は README の「Cloudflare Tunnel を使う場合」を参照。
set -euo pipefail

[[ $EUID -eq 0 ]] || { echo "root で実行してください"; exit 1; }
PORT=8788

curl -fsS "http://127.0.0.1:$PORT/pub/health" >/dev/null || {
  echo "公開 API（127.0.0.1:$PORT）が応答しません。先に 03-gallery.sh を実行してください"; exit 1;
}

if ! command -v tailscale >/dev/null; then
  dnf config-manager --add-repo https://pkgs.tailscale.com/stable/rhel/8/tailscale.repo
  dnf -y install tailscale
fi
systemctl enable --now tailscaled

if ! tailscale status >/dev/null 2>&1; then
  echo
  echo "Tailscale にログインします。表示される URL をブラウザで開いて承認してください。"
  tailscale up
fi

echo
echo "Funnel（インターネットへの公開）を有効にします。"
echo "初めてのときは、表示される URL で Funnel の利用を許可してください（Tailscale の管理画面）。"
tailscale funnel --bg "$PORT"

echo
tailscale funnel status
HOST=$(tailscale status --json | python3 -c "import json,sys;print(json.load(sys.stdin)['Self']['DNSName'].rstrip('.'))" 2>/dev/null || true)
echo
if [[ -n "$HOST" ]]; then
  echo "公開 API の URL: https://$HOST"
  echo "確認: curl https://$HOST/pub/health"
  echo "この URL を管理画面「写真の配信」→「公開 API の URL」に入れて保存し、「GitHub に公開」を押してください。"
fi
