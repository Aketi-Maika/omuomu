//! 回答と操作ログの保存（NAS の中。1 行 1 件の JSON）と、Google スプレッドシートへの写し

use crate::config::Config;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseRow {
    pub t: u64,
    pub photo_id: String,
    pub photo_title: String,
    pub people: Vec<String>,
    pub names: Vec<String>,
    pub unknown: u32,
    pub left: u32,
    pub staff: u32,
    pub email: String,
    pub name: String,
    pub session: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRow {
    pub t: u64,
    pub email: String,
    pub name: String,
    pub role: String,
    pub action: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PhotoStat {
    pub n: u32,
    pub people: HashMap<String, u32>,
    pub unknown: u32,
    pub left: u32,
    pub staff: u32,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn data_dir(root: &Path, cfg: &Config) -> PathBuf {
    cfg.store_path(root).join("data")
}

fn append<T: Serialize>(path: &Path, row: &T) -> Result<()> {
    let _g = WRITE_LOCK.lock().unwrap();
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{}", serde_json::to_string(row)?)?;
    Ok(())
}

fn read_lines<T: for<'de> Deserialize<'de>>(path: &Path) -> Vec<T> {
    let Ok(f) = std::fs::File::open(path) else { return vec![] };
    BufReader::new(f).lines().map_while(Result::ok).filter_map(|l| serde_json::from_str(&l).ok()).collect()
}

pub fn add_response(root: &Path, cfg: &Config, r: &ResponseRow) -> Result<()> {
    append(&data_dir(root, cfg).join("responses.jsonl"), r)
}

pub fn add_audit(root: &Path, cfg: &Config, email: &str, name: &str, role: &str, action: &str, detail: impl Serialize) -> Result<()> {
    let mut d = serde_json::to_string(&detail).unwrap_or_default();
    if d.len() > 2000 {
        d.truncate(d.char_indices().take_while(|(i, _)| *i < 2000).last().map(|(i, _)| i).unwrap_or(0));
    }
    let row = AuditRow { t: now_ms(), email: email.into(), name: name.into(), role: role.into(), action: action.into(), detail: d };
    append(&data_dir(root, cfg).join("audit.jsonl"), &row)
}

pub fn responses(root: &Path, cfg: &Config) -> Vec<ResponseRow> {
    read_lines(&data_dir(root, cfg).join("responses.jsonl"))
}

pub fn stats(root: &Path, cfg: &Config) -> HashMap<String, PhotoStat> {
    let mut s: HashMap<String, PhotoStat> = HashMap::new();
    for r in responses(root, cfg) {
        let st = s.entry(r.photo_id.clone()).or_default();
        st.n += 1;
        let mut seen = std::collections::HashSet::new();
        for p in &r.people {
            if seen.insert(p) {
                *st.people.entry(p.clone()).or_default() += 1;
            }
        }
        st.unknown += (r.unknown > 0) as u32;
        st.left += (r.left > 0) as u32;
        st.staff += (r.staff > 0) as u32;
    }
    s
}

/// 管理室の「ログ」: 操作ログと回答を新しい順に
pub fn audit(root: &Path, cfg: &Config, limit: usize) -> Vec<AuditRow> {
    let mut out: Vec<AuditRow> = read_lines(&data_dir(root, cfg).join("audit.jsonl"));
    out.extend(responses(root, cfg).into_iter().map(|r| AuditRow {
        t: r.t,
        email: r.email,
        name: r.name,
        role: String::new(),
        action: "回答".into(),
        detail: json!({ "photoId": r.photo_id, "photo": r.photo_title, "names": r.names.join("、"),
                        "unknown": r.unknown, "left": r.left, "staff": r.staff })
        .to_string(),
    }));
    out.sort_by(|a, b| b.t.cmp(&a.t));
    out.truncate(limit);
    out
}

// ---------- スプレッドシートへの写し ----------

#[derive(Default, Serialize, Deserialize)]
struct MirrorState {
    responses: usize,
    audit: usize,
}

/// まだ送っていない行を Apps Script に送る（失敗したら次回に持ち越し）
pub fn mirror_once(root: &Path, cfg: &Config) -> Result<usize> {
    if cfg.sheet_url.trim().is_empty() || cfg.sheet_token.trim().is_empty() {
        return Ok(0);
    }
    let dir = data_dir(root, cfg);
    let state_path = dir.join("mirror-state.json");
    let mut st: MirrorState = std::fs::read_to_string(&state_path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let mut sent = 0;
    for kind in ["responses", "audit"] {
        let rows: Vec<serde_json::Value> = read_lines(&dir.join(format!("{kind}.jsonl")));
        let from = if kind == "responses" { st.responses } else { st.audit };
        if from >= rows.len() {
            continue;
        }
        for chunk in rows[from..].chunks(200) {
            let res = ureq::post(cfg.sheet_url.trim())
                .timeout(std::time::Duration::from_secs(30))
                .send_string(&json!({ "token": cfg.sheet_token.trim(), "kind": kind, "rows": chunk }).to_string());
            let body = match res {
                Ok(r) => r.into_string().unwrap_or_default(),
                Err(e) => bail!("スプレッドシートに送れません: {e}"),
            };
            let v: serde_json::Value = serde_json::from_str(&body).unwrap_or(json!({ "error": body.chars().take(200).collect::<String>() }));
            if v.get("ok").and_then(|x| x.as_bool()) != Some(true) {
                bail!("スプレッドシートが受け付けませんでした: {v}");
            }
            if kind == "responses" {
                st.responses += chunk.len();
            } else {
                st.audit += chunk.len();
            }
            sent += chunk.len();
            std::fs::write(&state_path, serde_json::to_string(&st)?)?;
        }
    }
    Ok(sent)
}
