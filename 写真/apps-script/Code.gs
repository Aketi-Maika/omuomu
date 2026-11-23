/**
 * 写真展「写っているのは、だれ？」 — Google ログインの確認・回答の保存・操作ログ
 *
 * 準備（README の「Google ログインとスプレッドシートの準備」も参照）:
 *  1. Google スプレッドシートを新規作成 →［拡張機能］→［Apps Script］にこのファイルを貼り付けて保存
 *  2. PC の管理画面「設定」→「Apps Script に貼る設定」の関数 setupProperties を貼り付け、1 回実行
 *     （CLIENT_ID / ALLOWED_DOMAIN / ADMIN_EMAILS / VIEW_KEY がスクリプトプロパティに入ります）
 *  3. ［デプロイ］→［新しいデプロイ］→ ウェブアプリ
 *       次のユーザーとして実行: 自分 ／ アクセスできるユーザー: 全員
 *     → URL（…/exec）を管理画面の「Apps Script ウェブアプリの URL」に貼る
 *
 * シート:
 *   responses … 写っている人の回答（1 回 = 1 行、上書きしない）
 *   audit     … 入場・管理室の操作のログ
 *
 * 誰が操作したかは、ブラウザから送られた名前ではなく、Google が発行したログイン証明
 * （ID トークン）をこのスクリプトで検証して得たメールアドレスで記録します。
 */

const SESSION_HOURS = 12;
const RESPONSES = 'responses';
const AUDIT = 'audit';
const HEADERS = {
  responses: ['日時', 'photo_id', '写真', '人物ID', '名前', '不明', '退会', '教職員', 'メール', '氏名', 'セッション'],
  audit: ['日時', 'メール', '氏名', '役割', '操作', '内容'],
};

// ---------- 共通 ----------

function props_() {
  return PropertiesService.getScriptProperties();
}
function prop_(k) {
  return (props_().getProperty(k) || '').trim();
}
function list_(k) {
  return prop_(k)
    .toLowerCase()
    .split(/[,\s、]+/)
    .map(function (s) { return s.replace(/^@/, ''); })
    .filter(String);
}
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
function append_(name, row) {
  const lock = LockService.getScriptLock();
  lock.waitLock(10000);
  try {
    sheet_(name).appendRow(row);
  } finally {
    lock.releaseLock();
  }
}
function clip_(v, n) {
  return String(v == null ? '' : v).slice(0, n);
}
function count_(v) {
  return Math.max(0, Math.min(99, Math.floor(Number(v) || 0)));
}
function audit_(user, action, detail) {
  append_(AUDIT, [new Date(), user.e, user.n, user.r, action, clip_(typeof detail === 'string' ? detail : JSON.stringify(detail || {}), 2000)]);
}

// ---------- セッション（HMAC 署名つき） ----------

function secret_() {
  let s = prop_('SESSION_SECRET');
  if (!s) {
    s = Utilities.getUuid() + Utilities.getUuid();
    props_().setProperty('SESSION_SECRET', s);
  }
  return s;
}
function b64u_(bytes) {
  return Utilities.base64EncodeWebSafe(bytes).replace(/=+$/, '');
}
function sign_(payload) {
  return b64u_(Utilities.computeHmacSha256Signature(payload, secret_()));
}
function issue_(user) {
  const body = b64u_(Utilities.newBlob(JSON.stringify(user)).getBytes());
  return body + '.' + sign_(body);
}
function verify_(token) {
  const parts = String(token || '').split('.');
  if (parts.length !== 2 || sign_(parts[0]) !== parts[1]) return null;
  let u;
  try {
    u = JSON.parse(Utilities.newBlob(Utilities.base64DecodeWebSafe(parts[0])).getDataAsString());
  } catch (e) {
    return null;
  }
  if (!u.x || u.x < Date.now()) return null;
  return u;
}

// ---------- Google ログインの検証 ----------

function checkIdToken_(idToken) {
  const res = UrlFetchApp.fetch('https://oauth2.googleapis.com/tokeninfo?id_token=' + encodeURIComponent(idToken), {
    muteHttpExceptions: true,
  });
  if (res.getResponseCode() !== 200) throw new Error('ログイン情報を確認できませんでした');
  const t = JSON.parse(res.getContentText());
  if (t.aud !== prop_('CLIENT_ID')) throw new Error('このサイト用のログインではありません');
  if (t.iss !== 'accounts.google.com' && t.iss !== 'https://accounts.google.com') throw new Error('発行元が正しくありません');
  if (String(t.email_verified) !== 'true') throw new Error('メールアドレスが確認されていないアカウントです');
  if (Number(t.exp) * 1000 < Date.now()) throw new Error('ログインの有効期限が切れています');

  const email = String(t.email).toLowerCase();
  const domain = email.split('@')[1] || '';
  const admins = list_('ADMIN_EMAILS');
  const domains = list_('ALLOWED_DOMAIN');
  const isAdmin = admins.indexOf(email) >= 0;
  if (!isAdmin && domains.indexOf(domain) < 0) {
    throw new Error('このアカウント（' + email + '）では入場できません');
  }
  return { e: email, n: clip_(t.name || email, 60), r: isAdmin ? 'admin' : 'viewer', x: Date.now() + SESSION_HOURS * 3600 * 1000 };
}

// ---------- 入口 ----------

function doGet() {
  return json_({ ok: true, service: 'omuomu-photo' });
}

function doPost(e) {
  let b;
  try {
    b = JSON.parse(e.postData.contents);
  } catch (err) {
    return json_({ error: 'bad request' });
  }
  try {
    if (b.action === 'login') return json_(login_(b));
    const user = verify_(b.session);
    if (!user) return json_({ error: 'session', message: 'もう一度ログインしてください' });
    switch (b.action) {
      case 'stats':
        return json_(stats_());
      case 'submit':
        return json_(submit_(user, b));
      case 'log':
        audit_(user, clip_(b.event, 40), b.detail);
        return json_({ ok: true });
      case 'audit':
        if (user.r !== 'admin') return json_({ error: 'forbidden' });
        return json_(auditRows_(b.limit));
      default:
        return json_({ error: 'unknown action' });
    }
  } catch (err) {
    return json_({ error: 'failed', message: String(err.message || err) });
  }
}

function login_(b) {
  if (!prop_('CLIENT_ID') || !prop_('VIEW_KEY')) throw new Error('スクリプトプロパティが未設定です（setupProperties を実行してください）');
  const user = checkIdToken_(b.idToken);
  audit_(user, user.r === 'admin' ? '入場（管理者）' : '入場', { ua: clip_(b.ua, 200) });
  return {
    session: issue_(user),
    email: user.e,
    name: user.n,
    role: user.r,
    exp: user.x,
    viewKey: prop_('VIEW_KEY'),
  };
}

function submit_(user, b) {
  if (!b.photoId || !/^[0-9a-f]{8,32}$/.test(String(b.photoId))) throw new Error('bad photoId');
  const people = (Array.isArray(b.people) ? b.people : []).map(function (x) { return clip_(x, 40); }).filter(String).slice(0, 100);
  const names = (Array.isArray(b.names) ? b.names : []).map(function (x) { return clip_(x, 40); }).slice(0, 100);
  append_(RESPONSES, [
    new Date(),
    clip_(b.photoId, 32),
    clip_(b.photoTitle, 120),
    people.join(','),
    names.join('、'),
    count_(b.unknown),
    count_(b.left),
    count_(b.staff),
    user.e,
    user.n,
    clip_(b.sessionId, 40),
  ]);
  return { ok: true };
}

function stats_() {
  const values = sheet_(RESPONSES).getDataRange().getValues();
  const photos = {};
  for (let i = 1; i < values.length; i++) {
    const r = values[i];
    const id = String(r[1]);
    if (!id) continue;
    const st = photos[id] || (photos[id] = { n: 0, people: {}, unknown: 0, left: 0, staff: 0 });
    st.n++;
    const seen = {};
    String(r[3]).split(',').filter(String).forEach(function (pid) {
      if (seen[pid]) return;
      seen[pid] = true;
      st.people[pid] = (st.people[pid] || 0) + 1;
    });
    if (Number(r[5]) > 0) st.unknown++;
    if (Number(r[6]) > 0) st.left++;
    if (Number(r[7]) > 0) st.staff++;
  }
  return { photos: photos };
}

/** 管理室の「ログ」タブ用: 回答と操作ログを新しい順に */
function auditRows_(limit) {
  const n = Math.max(10, Math.min(1000, Number(limit) || 300));
  function tail(name) {
    const sh = sheet_(name);
    const last = sh.getLastRow();
    if (last < 2) return [];
    const start = Math.max(2, last - n + 1);
    return sh.getRange(start, 1, last - start + 1, sh.getLastColumn()).getValues();
  }
  const out = [];
  tail(AUDIT).forEach(function (r) {
    out.push({ t: new Date(r[0]).getTime(), email: r[1], name: r[2], role: r[3], action: r[4], detail: String(r[5]) });
  });
  tail(RESPONSES).forEach(function (r) {
    out.push({
      t: new Date(r[0]).getTime(),
      email: r[8],
      name: r[9],
      role: '',
      action: '回答',
      detail: JSON.stringify({ photoId: r[1], photo: r[2], names: r[4], unknown: r[5], left: r[6], staff: r[7] }),
    });
  });
  out.sort(function (a, b) { return b.t - a.t; });
  return { rows: out.slice(0, n) };
}
