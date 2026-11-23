//! 管理画面（既定は http://localhost:8787 で、この PC の中だけで開く）
//! NAS のパス設定・構築・GitHub への公開を行う API と、
//! ビルド済みの Web 画面(web/dist)・公開データ(web/public/data)を配信する。
//!
//! `--listen 0.0.0.0` で NAS に常駐させると LAN の他の PC からも開ける。その場合、
//! この機械以外からの接続には管理ツールのパスワード（Basic 認証）を求める。

use crate::builder::{self, Progress, SharedProgress};
use crate::config::Config;
use crate::publish;
use axum::extract::{ConnectInfo, Query, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tower_http::services::ServeDir;

#[derive(Clone)]
struct App {
    root: PathBuf,
    progress: SharedProgress,
}

fn err(status: StatusCode, msg: impl ToString) -> Response {
    (status, Json(json!({ "error": msg.to_string() }))).into_response()
}

/// LAN からの接続の認証（一度通ったヘッダーは覚えておき、毎回 Argon2 を計算しない）
#[derive(Clone)]
struct Guard {
    root: PathBuf,
    ok: Arc<Mutex<HashSet<[u8; 32]>>>,
}

fn unauthorized(msg: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, r#"Basic realm="omuomu-gallery", charset="UTF-8""#)],
        msg.to_string(),
    )
        .into_response()
}

async fn lan_guard(State(g): State<Guard>, ConnectInfo(peer): ConnectInfo<SocketAddr>, req: Request, next: Next) -> Response {
    let ip = match peer.ip() {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(IpAddr::V6(v6)),
        ip => ip,
    };
    if ip.is_loopback() {
        return next.run(req).await;
    }
    let Some(auth) = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).map(str::to_string) else {
        return unauthorized("管理ツールのパスワードが必要です");
    };
    let fp = *blake3::hash(auth.as_bytes()).as_bytes();
    if g.ok.lock().unwrap().contains(&fp) {
        return next.run(req).await;
    }
    let root = g.root.clone();
    let valid = tokio::task::spawn_blocking(move || {
        use base64::Engine;
        let b64 = auth.strip_prefix("Basic ")?;
        let raw = base64::engine::general_purpose::STANDARD.decode(b64.trim()).ok()?;
        let s = String::from_utf8(raw).ok()?;
        let pass = s.split_once(':').map(|(_, p)| p.to_string())?;
        Config::load(&root).ok().map(|c| c.check_lan_password(&pass))
    })
    .await
    .ok()
    .flatten()
    .unwrap_or(false);
    if !valid {
        // 総当たりを遅らせる
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        return unauthorized("パスワードが違います");
    }
    g.ok.lock().unwrap().insert(fp);
    next.run(req).await
}

pub async fn run(root: PathBuf, listen: IpAddr, port: u16, open_browser: bool) -> anyhow::Result<()> {
    let cfg = Config::load(&root)?;
    let app = App { root: root.clone(), progress: Arc::new(Mutex::new(Progress::default())) };
    let dist = root.join("web").join("dist");
    if !dist.join("index.html").exists() {
        eprintln!("注意: web/dist がありません。先に `npm run build`（web フォルダー）を実行してください。");
    }

    let api = Router::new()
        .route("/status", get(status))
        .route("/config", get(get_config).put(put_config))
        .route("/config/new-key", post(new_key))
        .route("/dev-key", get(dev_key))
        .route("/apps-script-setup", get(apps_script_setup))
        .route("/check-path", get(check_path))
        .route("/build", post(start_build))
        .route("/build/status", get(build_status))
        .route("/publish", post(do_publish))
        .route("/export", post(export))
        .with_state(app);

    let lan = !listen.is_loopback();
    if lan && !cfg.has_lan_password() {
        anyhow::bail!("LAN に公開するには、先に `omuomu-gallery set-password` で管理ツールのパスワードを設定してください");
    }
    let guard = Guard { root: root.clone(), ok: Arc::new(Mutex::new(HashSet::new())) };
    let router = Router::new()
        .nest("/api", api)
        .nest_service("/data", ServeDir::new(cfg.out_path(&root)))
        .fallback_service(ServeDir::new(dist))
        .layer(middleware::from_fn_with_state(guard, lan_guard));
    let svc = router.into_make_service_with_connect_info::<SocketAddr>();

    let listener = tokio::net::TcpListener::bind(SocketAddr::from((listen, port))).await?;
    if !lan {
        // localhost が IPv6(::1) に解決される環境のため、IPv6 のループバックでも待ち受ける
        if let Ok(v6) = tokio::net::TcpListener::bind(SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, port))).await {
            let s6 = svc.clone();
            tokio::spawn(async move {
                let _ = axum::serve(v6, s6).await;
            });
        }
    }
    // Google ログインの承認済みオリジンに登録しやすいよう localhost で開く
    let url = format!("http://localhost:{port}/#/admin");
    if lan {
        println!("管理画面: http://<この機械の IP アドレス>:{port}/#/admin （LAN から。パスワードが必要）");
    }
    println!("管理画面: {url}  （終了は Ctrl+C）");
    if open_browser {
        let _ = open::that(&url);
    }
    axum::serve(listener, svc).await?;
    Ok(())
}

async fn status(State(app): State<App>) -> impl IntoResponse {
    let git = publish::git_info(&app.root);
    Json(json!({ "local": true, "root": app.root.display().to_string(), "git": git }))
}

async fn get_config(State(app): State<App>) -> Response {
    match Config::load(&app.root) {
        Ok(mut c) => {
            c.lan_password = if c.has_lan_password() { "設定済み".into() } else { String::new() };
            Json(c).into_response()
        }
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")),
    }
}

async fn put_config(State(app): State<App>, Json(mut cfg): Json<Config>) -> Response {
    // 鍵と LAN 用パスワードは画面から書き換えさせない
    if let Ok(cur) = Config::load(&app.root) {
        cfg.view_key = cur.view_key;
        cfg.lan_password = cur.lan_password;
    }
    match cfg.save(&app.root) {
        Ok(()) => Json(json!({ "ok": true, "problems": cfg.validate(), "warnings": cfg.publish_warnings() })).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")),
    }
}

/// 鍵を作り直す（以前に配った鍵では開けなくなる。Apps Script の VIEW_KEY も更新が必要）
async fn new_key(State(app): State<App>) -> Response {
    let r = Config::load(&app.root).and_then(|mut c| {
        c.view_key = crate::config::new_view_key();
        c.save(&app.root)?;
        Ok(c)
    });
    match r {
        Ok(_) => Json(json!({ "ok": true })).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")),
    }
}

/// この PC での表示確認用: Google ログインなしで写真の鍵を渡す（127.0.0.1 でしか待ち受けない）
async fn dev_key(State(app): State<App>) -> Response {
    match Config::load(&app.root) {
        Ok(c) => Json(json!({ "viewKey": c.view_key })).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")),
    }
}

async fn apps_script_setup(State(app): State<App>) -> Response {
    match Config::load(&app.root) {
        Ok(c) => Json(json!({ "code": c.apps_script_setup() })).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")),
    }
}

#[derive(Deserialize)]
struct PathQ {
    p: String,
}

/// 管理画面で入力したパスが存在するか、写真が何枚あるかを確認
async fn check_path(State(app): State<App>, Query(q): Query<PathQ>) -> impl IntoResponse {
    let root = app.root.clone();
    let res = tokio::task::spawn_blocking(move || {
        let p = Config::resolve(&root, &q.p);
        if p.is_file() {
            return json!({ "exists": true, "kind": "file", "path": p.display().to_string() });
        }
        if !p.is_dir() {
            return json!({ "exists": false, "path": p.display().to_string() });
        }
        let n = walkdir::WalkDir::new(&p)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file()
                    && matches!(
                        e.path().extension().and_then(|x| x.to_str()).map(|s| s.to_lowercase()).as_deref(),
                        Some("jpg" | "jpeg" | "png")
                    )
            })
            .count();
        json!({ "exists": true, "kind": "dir", "images": n, "path": p.display().to_string() })
    })
    .await
    .unwrap_or_else(|e| json!({ "exists": false, "error": e.to_string() }));
    Json(res)
}

async fn start_build(State(app): State<App>) -> Response {
    {
        let mut p = app.progress.lock().unwrap();
        if p.running {
            return err(StatusCode::CONFLICT, "構築中です");
        }
        *p = Progress { running: true, phase: "準備中".into(), ..Default::default() };
    }
    let cfg = match Config::load(&app.root) {
        Ok(c) => c,
        Err(e) => {
            app.progress.lock().unwrap().running = false;
            return err(StatusCode::BAD_REQUEST, format!("{e:#}"));
        }
    };
    let root = app.root.clone();
    let prog = app.progress.clone();
    std::thread::spawn(move || {
        let r = builder::build(&root, &cfg, &prog);
        let mut p = prog.lock().unwrap();
        p.running = false;
        match r {
            Ok(rep) => {
                p.phase = "完了".into();
                p.report = Some(rep);
            }
            Err(e) => {
                p.phase = "失敗".into();
                p.error = Some(format!("{e:#}"));
                p.log.push(format!("エラー: {e:#}"));
            }
        }
    });
    Json(json!({ "ok": true })).into_response()
}

async fn build_status(State(app): State<App>) -> impl IntoResponse {
    let p = app.progress.lock().unwrap();
    Json(json!({
        "running": p.running, "done": p.done, "total": p.total, "phase": p.phase,
        "log": p.log, "report": p.report, "error": p.error,
    }))
}

#[derive(Deserialize)]
struct PublishReq {
    message: Option<String>,
}

async fn do_publish(State(app): State<App>, Json(req): Json<PublishReq>) -> Response {
    let root = app.root.clone();
    let r = tokio::task::spawn_blocking(move || {
        let cfg = Config::load(&root)?;
        // Google ログインの設定が無いまま公開すると、誰も入場できないサイトになる
        let w: Vec<String> = cfg
            .publish_warnings()
            .into_iter()
            .filter(|w| w.contains("クライアント ID") || w.contains("Apps Script"))
            .collect();
        if !w.is_empty() {
            anyhow::bail!("公開する前に設定してください: {}", w.join(" / "));
        }
        publish::publish(&root, &cfg.out_path(&root), req.message.as_deref())
    })
    .await;
    match r {
        Ok(Ok(out)) => Json(out).into_response(),
        Ok(Err(e)) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

#[derive(Deserialize)]
struct ExportReq {
    ids: Vec<String>,
    dest: String,
}

async fn export(State(app): State<App>, Json(req): Json<ExportReq>) -> Response {
    if req.dest.trim().is_empty() {
        return err(StatusCode::BAD_REQUEST, "書き出し先フォルダーを指定してください");
    }
    let root = app.root.clone();
    let r = tokio::task::spawn_blocking(move || {
        let dest = Config::resolve(&root, &req.dest);
        builder::export_originals(&root, &req.ids, &dest).map(|m| (dest, m))
    })
    .await;
    match r {
        Ok(Ok((dest, m))) => Json(json!({ "dest": dest.display().to_string(), "results": m })).into_response(),
        Ok(Err(e)) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}
