//! 管理画面から編集する設定。`gallery.config.json`（写真を開く鍵を含むので git には入れない）

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "gallery.config.json";
pub const CACHE_FILE: &str = ".gallery-cache.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    /// 写真フォルダー（NAS の UNC パス \\NAS\share\... やドライブ文字のパス）。複数可
    pub photo_dirs: Vec<String>,
    /// 名簿（.xlsx / .csv）。xlsx のセル内画像は「写っている写真」の登録済みデータとして取り込む
    pub roster_path: String,

    /// Google Cloud で作った OAuth クライアント ID（ウェブアプリ）
    pub google_client_id: String,
    /// ログインを許可するドメイン（例: example.ac.jp）。カンマ区切りで複数可
    pub allowed_domain: String,
    /// 管理室に入れるアカウント（メールアドレス）
    pub admin_emails: Vec<String>,
    /// Google Apps Script ウェブアプリの URL（ログイン・回答・ログの保存先）
    pub sheet_url: String,
    /// 写真を暗号化する鍵（自動生成。Apps Script のスクリプトプロパティ VIEW_KEY にも同じ値を設定）
    pub view_key: String,

    pub title: String,
    pub subtitle: String,
    pub period: String,
    pub notice: String,

    /// この件数以上回答が集まった写真は出にくくする
    pub threshold: u32,
    /// 出にくくした写真の重み（通常は 1）
    pub low_weight: f64,
    /// 管理画面の「写っている」判定に使う得票率の既定値
    pub majority: f64,

    /// 公開画像の長辺(px)
    pub max_size: u32,
    pub thumb_size: u32,
    pub jpeg_quality: u8,
    /// 出力先（プロジェクトフォルダーからの相対パス）
    pub out_dir: String,
    /// NAS に常駐させて LAN から開くときのパスワード（"salt$hash"。set-password で設定）
    pub lan_password: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            photo_dirs: vec![],
            roster_path: String::new(),
            google_client_id: String::new(),
            allowed_domain: String::new(),
            admin_emails: vec![],
            sheet_url: String::new(),
            view_key: String::new(),
            title: "あの日の、わたしたち。".into(),
            subtitle: "Portraits of Us — who is in the picture?".into(),
            period: String::new(),
            notice: "写っている方のお名前を教えてください".into(),
            threshold: 10,
            low_weight: 0.03,
            majority: 0.5,
            max_size: 1600,
            thumb_size: 480,
            jpeg_quality: 82,
            out_dir: "web/public/data".into(),
            lan_password: String::new(),
        }
    }
}

pub fn new_view_key() -> String {
    let mut k = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut k);
    B64.encode(k)
}

impl Config {
    /// 読み込み。鍵が無ければ生成して保存する
    pub fn load(root: &Path) -> Result<Self> {
        let p = root.join(CONFIG_FILE);
        let mut cfg: Self = if p.exists() {
            let s = std::fs::read_to_string(&p).with_context(|| format!("{} を読めません", p.display()))?;
            serde_json::from_str(&s).with_context(|| format!("{} の形式が正しくありません", p.display()))?
        } else {
            Self::default()
        };
        if cfg.view_key_bytes().is_none() {
            cfg.view_key = new_view_key();
            cfg.save(root)?;
        }
        Ok(cfg)
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        let p = root.join(CONFIG_FILE);
        std::fs::write(&p, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn has_lan_password(&self) -> bool {
        self.lan_password.contains('$')
    }

    pub fn set_lan_password(&mut self, pass: &str) -> Result<()> {
        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);
        let h = gallery_core::derive_key(pass, &salt, gallery_core::KdfParams::default())?;
        self.lan_password = format!("{}${}", B64.encode(salt), B64.encode(h));
        Ok(())
    }

    pub fn check_lan_password(&self, pass: &str) -> bool {
        let Some((s, h)) = self.lan_password.split_once('$') else { return false };
        let (Ok(salt), Ok(hash)) = (B64.decode(s), B64.decode(h)) else { return false };
        match gallery_core::derive_key(pass, &salt, gallery_core::KdfParams::default()) {
            // 比較時間で中身が推測されないよう、全バイトを比べる
            Ok(k) => k.len() == hash.len() && k.iter().zip(&hash).fold(0u8, |a, (x, y)| a | (x ^ y)) == 0,
            Err(_) => false,
        }
    }

    pub fn view_key_bytes(&self) -> Option<[u8; 32]> {
        B64.decode(self.view_key.trim()).ok().and_then(|v| v.try_into().ok())
    }

    pub fn domains(&self) -> Vec<String> {
        self.allowed_domain
            .split([',', ' ', '、'])
            .map(|d| d.trim().trim_start_matches('@').to_lowercase())
            .filter(|d| !d.is_empty())
            .collect()
    }

    pub fn admins(&self) -> Vec<String> {
        self.admin_emails.iter().map(|e| e.trim().to_lowercase()).filter(|e| e.contains('@')).collect()
    }

    /// 相対パスはプロジェクトフォルダー基準で解決する
    pub fn resolve(root: &Path, p: &str) -> PathBuf {
        let pb = PathBuf::from(p.trim());
        if pb.is_absolute() || p.starts_with("\\\\") {
            pb
        } else {
            root.join(pb)
        }
    }

    pub fn out_path(&self, root: &Path) -> PathBuf {
        Self::resolve(root, &self.out_dir)
    }

    /// 構築を止めるべき問題
    pub fn validate(&self) -> Vec<String> {
        let mut e = vec![];
        if self.photo_dirs.iter().all(|d| d.trim().is_empty()) {
            e.push("写真フォルダーを 1 つ以上設定してください".into());
        }
        if self.view_key_bytes().is_none() {
            e.push("写真の鍵が正しくありません（「鍵を作り直す」を押してください）".into());
        }
        if !(0.0..=1.0).contains(&self.low_weight) {
            e.push("出にくさの重みは 0〜1 の範囲にしてください".into());
        }
        e
    }

    /// 公開前に埋めておくべき項目（ローカル確認だけなら空でもよい）
    pub fn publish_warnings(&self) -> Vec<String> {
        let mut w = vec![];
        if self.google_client_id.trim().is_empty() {
            w.push("Google の OAuth クライアント ID が未設定です（公開サイトでログインできません）".into());
        }
        if self.sheet_url.trim().is_empty() {
            w.push("Apps Script の URL が未設定です（公開サイトでログインできません）".into());
        }
        if self.domains().is_empty() {
            w.push("ログインを許可するドメインが未設定です（管理者以外は入れません）".into());
        }
        if self.admins().is_empty() {
            w.push("管理者のメールアドレスが未設定です".into());
        }
        w
    }

    /// Apps Script のスクリプトプロパティを一括設定する関数（エディタに貼って 1 回実行する）
    pub fn apps_script_setup(&self) -> String {
        let esc = |s: &str| s.replace('\\', "\\\\").replace('\'', "\\'");
        format!(
            "// Apps Script エディタに貼り付けて、関数 setupProperties を 1 回だけ実行してください。\n\
             // 実行後はこの関数を削除してかまいません（鍵がコードに残らないように）。\n\
             function setupProperties() {{\n\
             \x20 PropertiesService.getScriptProperties().setProperties({{\n\
             \x20   CLIENT_ID: '{}',\n\
             \x20   ALLOWED_DOMAIN: '{}',\n\
             \x20   ADMIN_EMAILS: '{}',\n\
             \x20   VIEW_KEY: '{}',\n\
             \x20 }});\n\
             \x20 Logger.log('設定しました');\n\
             }}\n",
            esc(self.google_client_id.trim()),
            esc(&self.domains().join(",")),
            esc(&self.admins().join(",")),
            esc(self.view_key.trim()),
        )
    }
}
