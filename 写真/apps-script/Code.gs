/**
 * 写真展「写っているのは、だれ？」 — NAS から送られた記録をスプレッドシートに書き写す
 *
 * ログインの確認・写真の配信・回答の保存は NAS（omuomu-gallery）が行います。
 * このスクリプトは NAS が 30 秒ごとに送ってくる「回答」と「操作ログ」をシートに追記するだけです。
 * ブラウザ（参加者）からは呼ばれません。
 *
 * 準備:
 *  1. Google スプレッドシートを新規作成 →［拡張機能］→［Apps Script］にこのファイルを貼り付けて保存
 *  2. NAS の管理画面「設定」→「Apps Script に貼る設定を表示」の関数 setupProperties を貼り付け、1 回実行
 *     （NAS と共有する合言葉 SHEET_TOKEN がスクリプトプロパティに入ります）
 *  3. ［デプロイ］→［新しいデプロイ］→ ウェブアプリ
 *       次のユーザーとして実行: 自分 ／ アクセスできるユーザー: 全員
 *     → URL（…/exec）を NAS の管理画面「スプレッドシートの URL」に貼る
 *
 * シート:
 *   responses … 写っている人の回答（1 回 = 1 行）
 *   audit     … 入場・入場できず・管理室の操作・ZIP ダウンロードのログ
 */

const HEADERS = {
  responses: ['日時', 'photo_id', '写真', '人物ID', '名前', '不明', '退会', '教職員', 'メール', '氏名', 'セッション'],
  audit: ['日時', 'メール', '氏名', '役割', '操作', '内容'],
};

function json_(obj) {
  return ContentService.createTextOutput(JSON.stringify(obj)).setMimeType(ContentService.MimeType.JSON);
}

function sheet_(name) {
  const ss = SpreadsheetApp.getActiveSpreadsheet();
  let sh = ss.getSheetByName(name);
  if (!sh) {
    sh = ss.insertSheet(name);
    sh.appendRow(HEADERS[name]);
    sh.setFrozenRows(1);
  }
  return sh;
}

function clip_(v, n) {
  return String(v == null ? '' : v).slice(0, n);
}

/** 定数時間での文字列比較（合言葉の照合用） */
function same_(a, b) {
  a = String(a || '');
  b = String(b || '');
  let d = a.length ^ b.length;
  for (let i = 0; i < Math.max(a.length, b.length); i++) d |= (a.charCodeAt(i) || 0) ^ (b.charCodeAt(i) || 0);
  return d === 0;
}

function doGet() {
  return json_({ ok: true, service: 'omuomu-photo-mirror' });
}

function doPost(e) {
  let b;
  try {
    b = JSON.parse(e.postData.contents);
  } catch (err) {
    return json_({ error: 'bad request' });
  }
  const token = PropertiesService.getScriptProperties().getProperty('SHEET_TOKEN') || '';
  if (!token || !same_(b.token, token)) return json_({ error: 'unauthorized' });
  if (!HEADERS[b.kind] || !Array.isArray(b.rows)) return json_({ error: 'bad kind' });

  const rows = b.rows.slice(0, 500).map(function (r) {
    const t = new Date(Number(r.t) || Date.now());
    if (b.kind === 'responses') {
      return [
        t,
        clip_(r.photoId, 32),
        clip_(r.photoTitle, 120),
        (r.people || []).map(function (x) { return clip_(x, 40); }).join(','),
        (r.names || []).map(function (x) { return clip_(x, 40); }).join('、'),
        Number(r.unknown) || 0,
        Number(r.left) || 0,
        Number(r.staff) || 0,
        clip_(r.email, 120),
        clip_(r.name, 60),
        clip_(r.session, 40),
      ];
    }
    return [t, clip_(r.email, 120), clip_(r.name, 60), clip_(r.role, 10), clip_(r.action, 40), clip_(r.detail, 2000)];
  });
  if (!rows.length) return json_({ ok: true, added: 0 });

  const lock = LockService.getScriptLock();
  lock.waitLock(20000);
  try {
    const sh = sheet_(b.kind);
    sh.getRange(sh.getLastRow() + 1, 1, rows.length, rows[0].length).setValues(rows);
  } finally {
    lock.releaseLock();
  }
  return json_({ ok: true, added: rows.length });
}
