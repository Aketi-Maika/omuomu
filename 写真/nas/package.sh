#!/bin/bash
# Windows（Git Bash）で NAS 用の一式 dist-nas/omuomu-nas.tar.gz を作る
#   必要: rustup target add x86_64-unknown-linux-musl / cargo install cargo-zigbuild / pip install ziglang
#   使い方: 写真 フォルダーで  bash nas/package.sh
set -euo pipefail
cd "$(dirname "$0")/.."

ZIG=$(python -c "import ziglang,os;print(os.path.join(os.path.dirname(ziglang.__file__),'zig.exe'))")
export CARGO_ZIGBUILD_ZIG_PATH="$ZIG"

echo "== Linux 用の実行ファイル（Atom など古い CPU でも動く汎用 x86-64）"
cargo zigbuild --release -p omuomu-gallery --target x86_64-unknown-linux-musl

echo "== 画面"
( cd web && npm run wasm && npm run build )

OUT=dist-nas/omuomu-nas
rm -rf dist-nas
mkdir -p "$OUT"
cp target/x86_64-unknown-linux-musl/release/omuomu-gallery "$OUT/"
cp -r web/dist "$OUT/web-dist"
rm -rf "$OUT/web-dist/data"   # 表示用データは NAS 側で構築する
cp nas/01-storage.sh nas/02-share.sh nas/03-gallery.sh nas/04-tunnel.sh nas/omuomu-gallery.service nas/README.md "$OUT/"
if [[ -f gallery.config.json ]]; then
  # 写真の鍵・Google ログインの設定を NAS に引き継ぐ（パスは NAS 側で設定し直す）
  python -I - "$OUT/gallery.config.json" <<'PY'
import json, sys
c = json.load(open('gallery.config.json', encoding='utf-8'))
c['photoDirs'] = []
c['rosterPath'] = ''
c['lanPassword'] = ''
json.dump(c, open(sys.argv[1], 'w', encoding='utf-8'), ensure_ascii=False, indent=2)
PY
fi
# 改行コードを LF に（Windows で編集しても動くように）
sed -i 's/\r$//' "$OUT"/*.sh "$OUT"/omuomu-gallery.service
tar -C dist-nas -czf dist-nas/omuomu-nas.tar.gz omuomu-nas
echo
echo "できました: $(pwd)/dist-nas/omuomu-nas.tar.gz"
echo "NAS へ送る: scp dist-nas/omuomu-nas.tar.gz root@<NAS の IP>:/root/"
