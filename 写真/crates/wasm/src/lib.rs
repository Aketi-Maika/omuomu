//! ブラウザ向け: パスワード→鍵、写真の復号、ZIP 作成、重み付き抽選

use base64::Engine;
use gallery_core::KdfParams;
use std::io::{Cursor, Write};
use wasm_bindgen::prelude::*;

fn js_err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// salt は base64。戻り値は 32 バイトの鍵。
#[wasm_bindgen]
pub fn derive_key(password: &str, salt_b64: &str, m: u32, t: u32, p: u32) -> Result<Vec<u8>, JsError> {
    let salt = base64::engine::general_purpose::STANDARD.decode(salt_b64).map_err(js_err)?;
    let key = gallery_core::derive_key(password, &salt, KdfParams { m, t, p }).map_err(js_err)?;
    Ok(key.to_vec())
}

#[wasm_bindgen]
pub fn decrypt(key: &[u8], data: &[u8]) -> Result<Vec<u8>, JsError> {
    gallery_core::decrypt(key, data).map_err(js_err)
}

#[wasm_bindgen]
pub fn weighted_pick(weights: Vec<f64>, r: f64) -> i32 {
    gallery_core::weighted_pick(&weights, r).map(|i| i as i32).unwrap_or(-1)
}

/// 無圧縮 ZIP（写真は JPEG なので圧縮しても縮まない）。names[i] と files[i] が対応。
#[wasm_bindgen]
pub fn make_zip(names: Vec<String>, files: js_sys::Array) -> Result<Vec<u8>, JsError> {
    if names.len() != files.length() as usize {
        return Err(JsError::new("names と files の数が一致しません"));
    }
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o644);
    let mut used = std::collections::HashSet::new();
    for (i, name) in names.iter().enumerate() {
        let mut n = name.clone();
        let mut k = 2;
        while !used.insert(n.clone()) {
            n = format!("{k}_{name}");
            k += 1;
        }
        let bytes = js_sys::Uint8Array::new(&files.get(i as u32)).to_vec();
        w.start_file(n, opts).map_err(js_err)?;
        w.write_all(&bytes).map_err(js_err)?;
    }
    Ok(w.finish().map_err(js_err)?.into_inner())
}
