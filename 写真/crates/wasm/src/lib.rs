//! ブラウザ向け: 重み付き抽選（回答が集まった写真ほど出にくくする）

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn weighted_pick(weights: Vec<f64>, r: f64) -> i32 {
    gallery_core::weighted_pick(&weights, r).map(|i| i as i32).unwrap_or(-1)
}
