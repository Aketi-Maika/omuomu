//! 参加者向けの公開 API（/pub/...）
//! NAS の中（127.0.0.1）で待ち受け、Cloudflare Tunnel / Tailscale Funnel 経由で外から届く。
//! すべての通信でログイン（署名つきトークン）と、今の設定での許可を確かめてから、写真を 1 枚ずつ返す。

use crate::auth::{self, User};
use crate::config::Config;
use crate::store::{self, ResponseRow};
use axum::body::Body;
use axum::extract::{Path as AxPath, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use tower_http::cors::{AllowOrigin, CorsLayer};

#[derive(Clone)]
pub struct Pub {
    pub root: PathBuf,
    /// 確認用ログイン（Google なし）を受け付けるか。外から届く公開用の待ち受けでは false
    pub allow_local: bool,
}

fn err(status: StatusCode, msg: impl ToString) -> Response {
    (status, Json(json!({ "error": msg.to_string() }))).into_response()
}

fn cfg(p: &Pub) -> Result<Config, Response> {
    Config::load(&p.root).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))
}

/// ログインを確かめ、今の設定での役割を返す（許可が外れていればここで止まる）
fn authed(p: &Pub, headers: &HeaderMap) -> Result<(Config, User, &'static str), Response> {
    let c = cfg(p)?;
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "ログインしてください"))?;
    let u = auth::verify(&c, token.trim()).ok_or_else(|| err(StatusCode::UNAUTHORIZED, "もう一度ログインしてください"))?;
    if u.l && !p.allow_local {
        return Err(err(StatusCode::UNAUTHORIZED, "確認用のログインは管理ツールの中でしか使えません"));
    }
    let role = auth::role_of(&c, &u).ok_or_else(|| err(StatusCode::FORBIDDEN, "このアカウントは現在入場できません"))?;
    Ok((c, u, role))
}

fn admin_only(role: &str) -> Result<(), Response> {
    (role == "admin").then_some(()).ok_or_else(|| err(StatusCode::FORBIDDEN, "管理者のみ使えます"))
}

fn manifest(p: &Pub, c: &Config) -> Result<Value, Response> {
    let path = c.store_path(&p.root).join("manifest.json");
    let s = std::fs::read_to_string(&path).map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "まだ写真が公開されていません"))?;
    serde_json::from_str(&s).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn valid_id(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

pub fn router(root: PathBuf, allow_local: bool) -> Router {
    let state = Pub { root: root.clone(), allow_local };
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .expose_headers([header::CONTENT_DISPOSITION])
        .max_age(std::time::Duration::from_secs(600))
        .allow_origin(AllowOrigin::predicate(move |origin: &HeaderValue, _| {
            let o = origin.to_str().unwrap_or("");
            Config::load(&root).map(|c| c.origins().iter().any(|x| x == o)).unwrap_or(false)
        }));
    Router::new()
        .route("/pub/health", get(health))
        .route("/pub/login", post(login))
        .route("/pub/manifest", get(get_manifest))
        .route("/pub/photo/:id", get(photo))
        .route("/pub/stats", get(stats))
        .route("/pub/submit", post(submit))
        .route("/pub/log", post(log))
        .route("/pub/audit", get(audit))
        .route("/pub/zip", post(zip))
        .layer(cors)
        .with_state(state)
}

async fn health(State(p): State<Pub>) -> Response {
    match cfg(&p) {
        Ok(c) => Json(json!({ "ok": true, "title": c.title, "ready": c.store_path(&p.root).join("manifest.json").exists() })).into_response(),
        Err(r) => r,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginReq {
    id_token: String,
}

async fn login(State(p): State<Pub>, Json(req): Json<LoginReq>) -> Response {
    let c = match cfg(&p) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let c2 = c.clone();
    let res = tokio::task::spawn_blocking(move || auth::verify_google(&c2, &req.id_token)).await;
    let u = match res {
        Ok(Ok(u)) => u,
        Ok(Err(e)) => return err(StatusCode::UNAUTHORIZED, format!("{e:#}")),
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    let Some(role) = auth::role_of(&c, &u) else {
        let _ = store::add_audit(&p.root, &c, &u.e, &u.n, "", "入場できず", json!({ "hd": u.hd }));
        let hint = if u.hd.is_empty() { "（組織の Google アカウントでログインしてください）" } else { "" };
        return err(StatusCode::FORBIDDEN, format!("このアカウント（{}）では入場できません{hint}", u.e));
    };
    let _ = store::add_audit(&p.root, &c, &u.e, &u.n, role, if role == "admin" { "入場（管理者）" } else { "入場" }, json!({}));
    Json(json!({ "session": auth::sign(&c, &u), "email": u.e, "name": u.n, "role": role, "exp": u.x })).into_response()
}

async fn get_manifest(State(p): State<Pub>, headers: HeaderMap) -> Response {
    let (c, _, role) = match authed(&p, &headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let mut m = match manifest(&p, &c) {
        Ok(m) => m,
        Err(r) => return r,
    };
    // 名簿(Excel)の登録済みデータは管理者だけに見せる（回答の先入観にならないように）
    if role != "admin" {
        if let Some(arr) = m.get_mut("photos").and_then(|v| v.as_array_mut()) {
            for ph in arr {
                if let Some(o) = ph.as_object_mut() {
                    o.remove("seed");
                }
            }
        }
    }
    Json(m).into_response()
}

#[derive(Deserialize)]
struct PhotoQ {
    t: Option<u8>,
}

async fn photo(State(p): State<Pub>, AxPath(id): AxPath<String>, Query(q): Query<PhotoQ>, headers: HeaderMap) -> Response {
    let (c, _, _) = match authed(&p, &headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    if !valid_id(&id) {
        return err(StatusCode::BAD_REQUEST, "bad id");
    }
    let name = if q.t == Some(1) { format!("{id}_t.jpg") } else { format!("{id}.jpg") };
    match tokio::fs::read(c.store_path(&p.root).join("img").join(name)).await {
        Ok(b) => (
            [
                (header::CONTENT_TYPE, "image/jpeg"),
                // 共有キャッシュには残さない
                (header::CACHE_CONTROL, "private, max-age=600"),
            ],
            Body::from(b),
        )
            .into_response(),
        Err(_) => err(StatusCode::NOT_FOUND, "写真がありません"),
    }
}

async fn stats(State(p): State<Pub>, headers: HeaderMap) -> Response {
    let (c, _, _) = match authed(&p, &headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let root = p.root.clone();
    let s = tokio::task::spawn_blocking(move || store::stats(&root, &c)).await.unwrap_or_default();
    Json(json!({ "photos": s })).into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitReq {
    photo_id: String,
    #[serde(default)]
    people: Vec<String>,
    #[serde(default)]
    unknown: u32,
    #[serde(default)]
    left: u32,
    #[serde(default)]
    staff: u32,
    #[serde(default)]
    session_id: String,
}

async fn submit(State(p): State<Pub>, headers: HeaderMap, Json(req): Json<SubmitReq>) -> Response {
    let (c, u, _) = match authed(&p, &headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let m = match manifest(&p, &c) {
        Ok(m) => m,
        Err(r) => return r,
    };
    // 写真と名簿は NAS の manifest と照らし合わせ、ブラウザから送られた名前は信用しない
    let photo = m["photos"].as_array().and_then(|a| a.iter().find(|x| x["id"] == req.photo_id.as_str())).cloned();
    let Some(photo) = photo else { return err(StatusCode::BAD_REQUEST, "写真がありません") };
    let roster = m["roster"].as_array().cloned().unwrap_or_default();
    let mut people = vec![];
    let mut names = vec![];
    for id in req.people.iter().take(100) {
        if let Some(r) = roster.iter().find(|r| r["id"] == id.as_str()) {
            if !people.contains(id) {
                people.push(id.clone());
                names.push(r["name"].as_str().unwrap_or("").to_string());
            }
        }
    }
    let row = ResponseRow {
        t: store::now_ms(),
        photo_id: req.photo_id.clone(),
        photo_title: photo["title"].as_str().unwrap_or("").into(),
        people,
        names,
        unknown: req.unknown.min(99),
        left: req.left.min(99),
        staff: req.staff.min(99),
        email: u.e.clone(),
        name: u.n.clone(),
        session: clip(&req.session_id, 40),
    };
    match store::add_response(&p.root, &c, &row) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")),
    }
}

#[derive(Deserialize)]
struct LogReq {
    event: String,
    #[serde(default)]
    detail: Value,
}

async fn log(State(p): State<Pub>, headers: HeaderMap, Json(req): Json<LogReq>) -> Response {
    let (c, u, role) = match authed(&p, &headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let _ = store::add_audit(&p.root, &c, &u.e, &u.n, role, &clip(&req.event, 40), &req.detail);
    Json(json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
struct AuditQ {
    limit: Option<usize>,
}

async fn audit(State(p): State<Pub>, headers: HeaderMap, Query(q): Query<AuditQ>) -> Response {
    let (c, _, role) = match authed(&p, &headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    if let Err(r) = admin_only(role) {
        return r;
    }
    let root = p.root.clone();
    let limit = q.limit.unwrap_or(300).clamp(10, 2000);
    let rows = tokio::task::spawn_blocking(move || store::audit(&root, &c, limit)).await.unwrap_or_default();
    Json(json!({ "rows": rows })).into_response()
}

#[derive(Deserialize)]
struct ZipReq {
    ids: Vec<String>,
    #[serde(default)]
    label: String,
    #[serde(default)]
    people: Vec<String>,
}

/// 管理者: 選んだ写真を ZIP で渡す（誰が・どの写真を持ち出したかを NAS 側で記録）
async fn zip(State(p): State<Pub>, headers: HeaderMap, Json(req): Json<ZipReq>) -> Response {
    let (c, u, role) = match authed(&p, &headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    if let Err(r) = admin_only(role) {
        return r;
    }
    let m = match manifest(&p, &c) {
        Ok(m) => m,
        Err(r) => return r,
    };
    let root = p.root.clone();
    let res = tokio::task::spawn_blocking(move || -> anyhow::Result<(Vec<u8>, Vec<String>)> {
        let photos = m["photos"].as_array().cloned().unwrap_or_default();
        let img = c.store_path(&root).join("img");
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let mut titles = vec![];
        for id in req.ids.iter().take(1000) {
            let Some(i) = photos.iter().position(|x| x["id"] == id.as_str()) else { continue };
            if !valid_id(id) {
                continue;
            }
            let title = photos[i]["title"].as_str().unwrap_or(id).to_string();
            let bytes = std::fs::read(img.join(format!("{id}.jpg")))?;
            w.start_file(format!("{:03}_{}.jpg", i + 1, title.replace(['/', '\\'], "_")), opts)?;
            w.write_all(&bytes)?;
            titles.push(title);
        }
        let out = w.finish()?.into_inner();
        store::add_audit(
            &root,
            &c,
            &u.e,
            &u.n,
            "admin",
            "ZIPダウンロード",
            json!({ "people": req.people, "count": titles.len(), "photos": titles }),
        )?;
        Ok((out, titles))
    })
    .await;
    match res {
        Ok(Ok((bytes, _))) => {
            let label: String = req.label.chars().filter(|c| !"\\/:*?\"<>|\r\n".contains(*c)).take(40).collect();
            let fname = format!("写真_{}.zip", if label.is_empty() { "選択".into() } else { label });
            let cd = format!("attachment; filename*=UTF-8''{}", urlencode(&fname));
            ([(header::CONTENT_TYPE, "application/zip".to_string()), (header::CONTENT_DISPOSITION, cd)], Body::from(bytes)).into_response()
        }
        Ok(Err(e)) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// この機械の管理ツールから表示を確かめるためのログイン（Google ログインなし）
pub fn local_session(c: &Config) -> String {
    auth::sign(
        c,
        &User { e: "local@管理ツール".into(), n: "管理ツール（確認用）".into(), hd: String::new(), x: store::now_ms() + 12 * 3600 * 1000, l: true },
    )
}
