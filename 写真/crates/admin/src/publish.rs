//! 公開: 暗号化済みデータだけを git commit / push する（元の写真や名簿は含めない）

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::json;
use std::path::Path;
use std::process::Command;

#[derive(Serialize)]
pub struct PublishResult {
    pub committed: bool,
    pub pushed: bool,
    pub branch: String,
    pub log: Vec<String>,
}

fn git(dir: &Path, args: &[&str]) -> Result<(bool, String)> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .context("git が見つかりません（Git for Windows をインストールしてください）")?;
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok((out.status.success(), s.trim().to_string()))
}

pub fn git_info(root: &Path) -> serde_json::Value {
    let branch = git(root, &["rev-parse", "--abbrev-ref", "HEAD"]).ok().filter(|r| r.0).map(|r| r.1);
    let remote = git(root, &["remote", "get-url", "origin"]).ok().filter(|r| r.0).map(|r| r.1);
    json!({ "branch": branch, "remote": remote })
}

pub fn publish(root: &Path, out_dir: &Path, message: Option<&str>) -> Result<PublishResult> {
    let mut log = vec![];
    let (ok, top) = git(root, &["rev-parse", "--show-toplevel"])?;
    if !ok {
        bail!("このフォルダーは git リポジトリではありません");
    }
    let (_, branch) = git(root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if !out_dir.join("public.json").exists() {
        bail!("公開データがありません。先に「構築」を実行してください");
    }
    let rel = out_dir.strip_prefix(root).unwrap_or(out_dir);
    let rel = rel.to_string_lossy().replace('\\', "/");
    log.push(format!("リポジトリ: {top}（ブランチ {branch}）"));

    // 暗号化済みデータのみをステージ（-A で削除された画像も反映）
    let (ok, o) = git(root, &["add", "-A", "--", &rel])?;
    if !o.is_empty() {
        log.push(o.clone());
    }
    if !ok {
        bail!("git add に失敗しました: {o}");
    }
    let (_, staged) = git(root, &["diff", "--cached", "--name-only", "--", &rel])?;
    let mut committed = false;
    if staged.trim().is_empty() {
        log.push("変更はありません（コミットは作りません）".into());
    } else {
        let n = staged.lines().count();
        let msg = message.filter(|m| !m.trim().is_empty()).unwrap_or("写真展データを更新");
        // 他にステージ済みのファイルがあっても巻き込まないよう、パスを指定してコミット
        let (ok, o) = git(root, &["commit", "-m", msg, "--", &rel])?;
        log.push(o.clone());
        if !ok {
            bail!("git commit に失敗しました: {o}");
        }
        committed = true;
        log.push(format!("{n} ファイルをコミットしました"));
    }

    let (ok, o) = git(root, &["push", "origin", &branch])?;
    log.push(o.clone());
    if !ok {
        bail!("git push に失敗しました（GitHub へのログインが必要かもしれません）: {o}");
    }
    log.push("GitHub に送信しました。数分後に GitHub Pages に反映されます".into());
    Ok(PublishResult { committed, pushed: true, branch, log })
}
