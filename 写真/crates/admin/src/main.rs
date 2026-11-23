//! omuomu-gallery
//!
//!   omuomu-gallery                    管理画面を起動（この PC だけ。ブラウザが開きます）
//!   omuomu-gallery build              設定ファイルの内容で公開データを構築
//!   omuomu-gallery set-password       LAN から開くときのパスワードを設定（標準入力から 1 行）
//!   オプション:
//!     --listen 0.0.0.0   LAN の他の機械からも開けるようにする（NAS に常駐させるとき）
//!     --port N           ポート番号（既定 8787）
//!     --public-port N    参加者向けの公開 API を 127.0.0.1:N で待ち受ける（NAS で常駐させるとき。既定なし）
//!     --root DIR         プロジェクトフォルダー（`写真`）の場所
//!     --no-open          ブラウザを自動で開かない

mod auth;
mod builder;
mod config;
mod public_api;
mod store;
mod publish;
mod roster;
mod server;

use std::io::BufRead;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn is_root(p: &Path) -> bool {
    p.join("web").is_dir() && p.join("crates").is_dir()
}

/// プロジェクトフォルダー（`写真`）を探す: --root → カレント → 実行ファイルの上位 → ビルド時の場所
fn find_root(arg: Option<&String>) -> PathBuf {
    if let Some(r) = arg {
        return PathBuf::from(r);
    }
    if let Ok(cwd) = std::env::current_dir() {
        if is_root(&cwd) {
            return cwd;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        for a in exe.ancestors() {
            if is_root(a) {
                return a.to_path_buf();
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap_or_else(|_| PathBuf::from("."))
}

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1))
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = find_root(opt(&args, "--root"));
    let port = opt(&args, "--port").and_then(|p| p.parse().ok()).unwrap_or(8787);
    let public_port: Option<u16> = opt(&args, "--public-port").and_then(|p| p.parse().ok());
    let listen: IpAddr = opt(&args, "--listen").and_then(|p| p.parse().ok()).unwrap_or(IpAddr::from([127, 0, 0, 1]));

    match args.first().map(|s| s.as_str()) {
        Some("build") => {
            let cfg = config::Config::load(&root)?;
            let progress = Arc::new(Mutex::new(builder::Progress::default()));
            builder::build(&root, &cfg, &progress)?;
            return Ok(());
        }
        Some("set-password") => {
            eprintln!("LAN から管理画面を開くときのパスワードを入力して Enter（8 文字以上）:");
            let mut line = String::new();
            std::io::stdin().lock().read_line(&mut line)?;
            let pass = line.trim_end_matches(['\r', '\n']);
            if pass.chars().count() < 8 {
                anyhow::bail!("8 文字以上にしてください");
            }
            let mut cfg = config::Config::load(&root)?;
            cfg.set_lan_password(pass)?;
            cfg.save(&root)?;
            eprintln!("設定しました（{}）", root.join(config::CONFIG_FILE).display());
            return Ok(());
        }
        _ => {}
    }

    let open_browser = !args.iter().any(|a| a == "--no-open") && listen.is_loopback();
    tokio::runtime::Runtime::new()?.block_on(server::run(root, listen, port, public_port, open_browser))
}
