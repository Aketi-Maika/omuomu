//! 管理画面から編集する設定。`gallery.config.json`（ログインの署名鍵などを含むので git には入れない）

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
    /// 写真フォルダー（NAS の /srv/nas/photos/... や UNC パス \\NAS\share\...）。複数可
    pub photo_dirs: Vec<String>,
    /// 名簿（.xlsx / .csv）。xlsx のセル内画像は「写っている写真」の登録済みデータとして取り込む
    pub roster_path: String,

    /// Google Cloud で作った OAuth クライアント ID（ウェブアプリ）
    pub google_client_id: String,
    /// ログインを許可する組織のドメイン（例: example.ac.jp）。カンマ区切りで複数可
    pub allowed_domain: String,
    /// 管理室に入れるアカウント（メールアドレス）
    pub admin_emails: Vec<String>,
    /// 入場を止めるアカウント（ドメインが許可されていても入れない）
    pub blocked_emails: Vec<String>,
    /// ログインの有効時間
    pub session_hours: u32,
    /// 公開 API の URL（Cloudflare Tunnel / Tailscale Funnel で外から届く https のアドレス）
    pub public_api_url: String,
    /// 公開 API を呼んでよいサイト（GitHub Pages のオリジンなど）
    pub allowed_origins: Vec<String>,

    /// Google スプレッドシートへの写し（Apps Script の URL と合言葉。空なら写さない）
    pub sheet_url: String,
    pub sheet_token: String,

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
    /// GitHub に置く表示用データの出力先（public.json だけ）
    pub out_dir: String,
    /// NAS の中だけに置く写真・名簿・回答の保存先（git には入れない）
    pub store_dir: String,

    /// NAS の管理画面を LAN に直接公開するときのパスワード（"salt$hash"）
    pub lan_password: String,
    /// ログインの署名に使う秘密（自動生成）
    pub session_secret: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            photo_dirs: vec![],
            roster_path: String::new(),
            google_client_id: String::new(),
            allowed_domain: String::new(),
            admin_emails: vec![],
            blocked_emails: vec![],
            session_hours: 8,
            public_api_url: String::new(),
            allowed_origins: vec![],
            sheet_url: String::new(),
            sheet_token: String::new(),
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
            store_dir: ".private".into(),
            lan_password: String::new(),
            session_secret: String::new(),
        }
    }
}

pub fn random_b64(n: usize) -> String {
    let mut k = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut k);
    B64.encode(k)
}

fn norm_list(v: &[String]) -> Vec<String> {
    v.iter().map(|e| e.trim().to_lowercase()).filter(|e| e.contains('@')).collect()
}

impl Config {
    /// 読み込み。署名の秘密が無ければ生成して保存する
    pub fn load(root: &Path) -> Result<Self> {
        let p = root.join(CONFIG_FILE);
        let mut cfg: Self = if p.exists() {
            let s = std::fs::read_to_string(&p).with_context(|| format!("{} を読めません", p.display()))?;
            serde_json::from_str(&s).with_context(|| format!("{} の形式が正しくありません", p.display()))?
        } else {
            Self::default()
        };
        if cfg.session_secret.len() < 32 {
            cfg.session_secret = random_b64(32);
            cfg.save(root)?;
        }
        Ok(cfg)
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        let p = root.join(CONFIG_FILE);
        std::fs::write(&p, serde_json::to_string_pretty(self)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
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

    pub fn domains(&self) -> Vec<String> {
        self.allowed_domain
            .split([',', ' ', '、'])
            .map(|d| d.trim().trim_start_matches('@').to_lowercase())
            .filter(|d| !d.is_empty())
            .collect()
    }

    pub fn admins(&self) -> Vec<String> {
        norm_list(&self.admin_emails)
    }

    pub fn blocked(&self) -> Vec<String> {
        norm_list(&self.blocked_emails)
    }

    pub fn origins(&self) -> Vec<String> {
        let mut o: Vec<String> = self
            .allowed_origins
            .iter()
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| s.starts_with("http"))
            .collect();
        for l in ["http://localhost:8787", "http://127.0.0.1:8787"] {
            if !o.iter().any(|x| x == l) {
                o.push(l.into());
            }
        }
        o
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

    pub fn store_path(&self, root: &Path) -> PathBuf {
        Self::resolve(root, &self.store_dir)
    }

    /// 構築を止めるべき問題
    pub fn validate(&self) -> Vec<String> {
        let mut e = vec![];
        if self.photo_dirs.iter().all(|d| d.trim().is_empty()) {
            e.push("写真フォルダーを 1 つ以上設定してください".into());
        }
        if !(0.0..=1.0).contains(&self.low_weight) {
            e.push("出にくさの重みは 0〜1 の範囲にしてください".into());
        }
        e
    }

    /// 公開前に埋めておくべき項目（この機械での確認だけなら空でもよい）
    pub fn publish_warnings(&self) -> Vec<String> {
        let mut w = vec![];
        if self.google_client_id.trim().is_empty() {
            w.push("Google の OAuth クライアント ID が未設定です（公開サイトでログインできません）".into());
        }
        if !self.public_api_url.trim().starts_with("https://") {
            w.push("公開 API の URL（https://…）が未設定です（公開サイトから写真を取れません）".into());
        }
        if self.domains().is_empty() {
            w.push("ログインを許可するドメインが未設定です（管理者以外は入れません）".into());
        }
        if self.admins().is_empty() {
            w.push("管理者のメールアドレスが未設定です".into());
        }
        if self.allowed_origins.iter().all(|o| o.trim().is_empty()) {
            w.push("公開 API を呼んでよいサイト（GitHub Pages のアドレス）が未設定です".into());
        }
        w
    }

    /// Apps Script のスクリプトプロパティを設定する関数（エディタに貼って 1 回実行する）
    pub fn apps_script_setup(&self) -> String {
        let esc = |s: &str| s.replace('\\', "\\\\").replace('\'', "\\'");
        format!(
            "// Apps Script エディタに貼り付けて、関数 setupProperties を 1 回だけ実行してください。\n\
             // 実行後はこの関数を削除してかまいません。\n\
             function setupProperties() {{\n\
             \x20 PropertiesService.getScriptProperties().setProperties({{ SHEET_TOKEN: '{}' }});\n\
             \x20 Logger.log('設定しました');\n\
             }}\n",
            esc(self.sheet_token.trim()),
        )
    }
}
