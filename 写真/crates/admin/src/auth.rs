//! Google ログインの確認と、NAS が発行するログイン状態（署名つきトークン）

use crate::config::Config;
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64U, Engine};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// メールアドレス
    pub e: String,
    /// 表示名
    pub n: String,
    /// Google Workspace の組織のドメイン（個人アカウントは空）
    #[serde(default)]
    pub hd: String,
    /// 有効期限（ミリ秒）
    pub x: u64,
    /// この機械の管理ツールから発行した確認用（Google ログインなし）
    #[serde(default)]
    pub l: bool,
}

fn mac_key(cfg: &Config) -> [u8; 32] {
    blake3::derive_key("omuomu-gallery session v1", cfg.session_secret.as_bytes())
}

pub fn sign(cfg: &Config, u: &User) -> String {
    let body = B64U.encode(serde_json::to_vec(u).unwrap_or_default());
    let mac = blake3::keyed_hash(&mac_key(cfg), body.as_bytes());
    format!("{body}.{}", B64U.encode(mac.as_bytes()))
}

pub fn verify(cfg: &Config, token: &str) -> Option<User> {
    let (body, mac) = token.split_once('.')?;
    let want = blake3::keyed_hash(&mac_key(cfg), body.as_bytes());
    let got = B64U.decode(mac).ok()?;
    // blake3::Hash の比較は定数時間
    if got.len() != 32 || blake3::Hash::from_bytes(got.try_into().ok()?) != want {
        return None;
    }
    let u: User = serde_json::from_slice(&B64U.decode(body).ok()?).ok()?;
    (u.x > crate::store::now_ms()).then_some(u)
}

/// 今の設定でこのアカウントに何を許すか（毎回の通信で確認するので、設定を変えるとすぐ効く）
pub fn role_of(cfg: &Config, u: &User) -> Option<&'static str> {
    if u.l {
        return Some("admin");
    }
    let email = u.e.to_lowercase();
    if cfg.blocked().contains(&email) {
        return None;
    }
    if cfg.admins().contains(&email) {
        return Some("admin");
    }
    let domain = email.rsplit_once('@').map(|(_, d)| d).unwrap_or("");
    let hd = u.hd.to_lowercase();
    // メールアドレスの末尾だけでは判定せず、Google Workspace の組織（hd）が一致するかを見る
    let ok = cfg.domains().iter().any(|d| if d == "gmail.com" { domain == "gmail.com" && hd.is_empty() } else { hd == *d && domain == d });
    ok.then_some("viewer")
}

#[derive(Deserialize)]
struct TokenInfo {
    aud: String,
    iss: String,
    email: String,
    #[serde(default)]
    email_verified: serde_json::Value,
    exp: String,
    #[serde(default)]
    hd: String,
    #[serde(default)]
    name: String,
}

/// Google が発行した ID トークンを Google に問い合わせて確かめる
pub fn verify_google(cfg: &Config, id_token: &str) -> Result<User> {
    if cfg.google_client_id.trim().is_empty() {
        bail!("Google ログインがまだ設定されていません");
    }
    if id_token.len() > 4096 || id_token.is_empty() {
        bail!("ログイン情報が正しくありません");
    }
    let res = ureq::get("https://oauth2.googleapis.com/tokeninfo")
        .query("id_token", id_token)
        .timeout(std::time::Duration::from_secs(15))
        .call();
    let t: TokenInfo = match res {
        Ok(r) => r.into_json().context("Google の応答を読めません")?,
        Err(ureq::Error::Status(_, _)) => bail!("ログイン情報を確認できませんでした"),
        Err(e) => bail!("Google に接続できません: {e}"),
    };
    if t.aud != cfg.google_client_id.trim() {
        bail!("このサイト用のログインではありません");
    }
    if t.iss != "accounts.google.com" && t.iss != "https://accounts.google.com" {
        bail!("発行元が正しくありません");
    }
    let verified = t.email_verified.as_bool().unwrap_or(false) || t.email_verified.as_str() == Some("true");
    if !verified {
        bail!("メールアドレスが確認されていないアカウントです");
    }
    let exp: u64 = t.exp.parse().unwrap_or(0);
    if exp * 1000 < crate::store::now_ms() {
        bail!("ログインの有効期限が切れています");
    }
    let hours = cfg.session_hours.clamp(1, 72) as u64;
    Ok(User {
        e: t.email.to_lowercase(),
        n: if t.name.is_empty() { t.email.clone() } else { t.name.chars().take(60).collect() },
        hd: t.hd.to_lowercase(),
        x: crate::store::now_ms() + hours * 3600 * 1000,
        l: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config {
            session_secret: "x".repeat(40),
            allowed_domain: "example.ac.jp".into(),
            admin_emails: vec!["boss@gmail.com".into()],
            blocked_emails: vec!["bad@example.ac.jp".into()],
            ..Default::default()
        }
    }
    fn user(e: &str, hd: &str) -> User {
        User { e: e.into(), n: "n".into(), hd: hd.into(), x: crate::store::now_ms() + 60_000, l: false }
    }

    #[test]
    fn token_roundtrip_and_tamper() {
        let c = cfg();
        let t = sign(&c, &user("a@example.ac.jp", "example.ac.jp"));
        assert_eq!(verify(&c, &t).unwrap().e, "a@example.ac.jp");
        let mut bad = t.clone();
        bad.insert(3, 'A');
        assert!(verify(&c, &bad).is_none());
        let mut other = cfg();
        other.session_secret = "y".repeat(40);
        assert!(verify(&other, &t).is_none());
        let mut old = user("a@example.ac.jp", "example.ac.jp");
        old.x = 1;
        assert!(verify(&c, &sign(&c, &old)).is_none());
    }

    #[test]
    fn roles() {
        let c = cfg();
        assert_eq!(role_of(&c, &user("a@example.ac.jp", "example.ac.jp")), Some("viewer"));
        // 組織に属さない個人アカウント（hd なし）は、同じドメインのアドレスでも入れない
        assert_eq!(role_of(&c, &user("a@example.ac.jp", "")), None);
        assert_eq!(role_of(&c, &user("a@other.jp", "other.jp")), None);
        assert_eq!(role_of(&c, &user("bad@example.ac.jp", "example.ac.jp")), None);
        assert_eq!(role_of(&c, &user("boss@gmail.com", "")), Some("admin"));
    }
}
