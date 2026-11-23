//! 写真展サイトの共通ロジック（PC の管理ツールとブラウザ(WASM)の両方で使う）
//!
//! - パスワード → 鍵: Argon2id
//! - 暗号化: AES-256-GCM（先頭 12 バイトが nonce）
//! - 重み付き抽選

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};

pub const NONCE_LEN: usize = 12;
pub const KEY_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("鍵の導出に失敗しました: {0}")]
    Kdf(String),
    #[error("復号できませんでした（パスワードが違う可能性があります）")]
    Decrypt,
    #[error("暗号化に失敗しました")]
    Encrypt,
}

/// Argon2id のパラメータ。public.json に平文で保存し、ブラウザ側も同じ値で鍵を作る。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct KdfParams {
    /// メモリ (KiB)
    pub m: u32,
    /// 反復回数
    pub t: u32,
    /// 並列度
    pub p: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        // スマホのブラウザでも 1 秒前後で終わる程度
        Self { m: 19 * 1024, t: 2, p: 1 }
    }
}

pub fn derive_key(password: &str, salt: &[u8], params: KdfParams) -> Result<[u8; KEY_LEN], Error> {
    let p = Params::new(params.m, params.t, params.p, Some(KEY_LEN)).map_err(|e| Error::Kdf(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
    let mut out = [0u8; KEY_LEN];
    argon
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|e| Error::Kdf(e.to_string()))?;
    Ok(out)
}

/// nonce は呼び出し側で乱数生成して渡す（wasm で乱数源を持たないため）。
pub fn encrypt(key: &[u8; KEY_LEN], nonce: [u8; NONCE_LEN], plaintext: &[u8]) -> Result<Vec<u8>, Error> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| Error::Encrypt)?;
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| Error::Encrypt)?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

pub fn decrypt(key: &[u8], data: &[u8]) -> Result<Vec<u8>, Error> {
    if data.len() < NONCE_LEN + 16 || key.len() != KEY_LEN {
        return Err(Error::Decrypt);
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| Error::Decrypt)?;
    let (nonce, ct) = data.split_at(NONCE_LEN);
    cipher.decrypt(Nonce::from_slice(nonce), ct).map_err(|_| Error::Decrypt)
}

/// 重み付き抽選。`r` は [0,1) の乱数。重みがすべて 0 以下なら None。
pub fn weighted_pick(weights: &[f64], r: f64) -> Option<usize> {
    let total: f64 = weights.iter().filter(|w| **w > 0.0).sum();
    if total <= 0.0 {
        return None;
    }
    let mut x = r.clamp(0.0, 0.999_999_999) * total;
    let mut last = None;
    for (i, w) in weights.iter().enumerate() {
        if *w <= 0.0 {
            continue;
        }
        last = Some(i);
        if x < *w {
            return Some(i);
        }
        x -= w;
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fast() -> KdfParams {
        KdfParams { m: 64, t: 1, p: 1 }
    }

    #[test]
    fn roundtrip() {
        let key = derive_key("pass", b"0123456789abcdef", fast()).unwrap();
        let ct = encrypt(&key, [7; 12], "写真".as_bytes()).unwrap();
        assert_eq!(decrypt(&key, &ct).unwrap(), "写真".as_bytes());
    }

    #[test]
    fn wrong_password_fails() {
        let k1 = derive_key("pass", b"0123456789abcdef", fast()).unwrap();
        let k2 = derive_key("pas5", b"0123456789abcdef", fast()).unwrap();
        let ct = encrypt(&k1, [1; 12], b"secret").unwrap();
        assert!(decrypt(&k2, &ct).is_err());
        assert!(decrypt(&k1, &ct[..10]).is_err());
    }

    #[test]
    fn pick_respects_weights() {
        let w = [1.0, 0.0, 3.0];
        assert_eq!(weighted_pick(&w, 0.0), Some(0));
        assert_eq!(weighted_pick(&w, 0.24), Some(0));
        assert_eq!(weighted_pick(&w, 0.26), Some(2));
        assert_eq!(weighted_pick(&w, 0.9999), Some(2));
        assert_eq!(weighted_pick(&[0.0, 0.0], 0.5), None);
        // 低い重みは選ばれにくい
        let w = [1.0, 0.03];
        let hits = (0..1000).filter(|i| weighted_pick(&w, *i as f64 / 1000.0) == Some(1)).count();
        assert!(hits < 40, "{hits}");
    }
}
