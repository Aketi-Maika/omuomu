//! NAS の写真 → 縮小（EXIF 削除）→ NAS の中の保存先へ
//!
//! 出力:
//!   <store>/img/<id>.jpg, <id>_t.jpg … 公開用に縮小した写真（NAS の中だけ。公開 API がログインを確かめて 1 枚ずつ返す）
//!   <store>/manifest.json            … 名簿・写真一覧・設定（NAS の中だけ）
//!   web/public/data/public.json      … GitHub に置く表示用の文言と接続先（写真や名前は含まない）

use crate::config::{Config, CACHE_FILE};
use crate::roster::{self, Person};
use anyhow::{bail, Context, Result};
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageReader};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const EXTS: &[&str] = &["jpg", "jpeg", "png"];
/// 16x16 グレースケールの平均差がこれ未満なら同じ写真とみなす
const SAME_PHOTO: f64 = 6.0;

#[derive(Default)]
pub struct Progress {
    pub running: bool,
    pub done: usize,
    pub total: usize,
    pub phase: String,
    pub log: Vec<String>,
    pub report: Option<Report>,
    pub error: Option<String>,
}

pub type SharedProgress = Arc<Mutex<Progress>>;

fn log(p: &SharedProgress, msg: impl Into<String>) {
    let m = msg.into();
    println!("{m}");
    p.lock().unwrap().log.push(m);
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub photos: usize,
    pub encoded: usize,
    pub reused: usize,
    pub removed: usize,
    pub people: usize,
    pub seed_links: usize,
    pub unmatched_refs: usize,
    pub seconds: f64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicInfo {
    pub v: u32,
    pub title: String,
    pub subtitle: String,
    pub period: String,
    pub notice: String,
    pub count: usize,
    pub google_client_id: String,
    pub allowed_domains: Vec<String>,
    /// 公開 API（NAS）の URL
    pub api_base: String,
    pub built_at: u64,
}

/// GitHub に置く public.json を書く（設定を変えただけのときも使う）
pub fn write_public(root: &Path, cfg: &Config, count: usize) -> Result<PathBuf> {
    let out = cfg.out_path(root);
    std::fs::create_dir_all(&out)?;
    // 旧方式の暗号化データが残っていれば消す（公開用フォルダーには public.json だけを置く）
    std::fs::remove_dir_all(out.join("img")).ok();
    for f in ["manifest.enc", "admin.enc"] {
        std::fs::remove_file(out.join(f)).ok();
    }
    let public = PublicInfo {
        v: 3,
        title: cfg.title.clone(),
        subtitle: cfg.subtitle.clone(),
        period: cfg.period.clone(),
        notice: cfg.notice.clone(),
        count,
        google_client_id: cfg.google_client_id.trim().into(),
        allowed_domains: cfg.domains(),
        api_base: cfg.public_api_url.trim().trim_end_matches('/').into(),
        built_at: now(),
    };
    let p = out.join("public.json");
    std::fs::write(&p, serde_json::to_string_pretty(&public)?)?;
    Ok(p)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RosterEntry {
    id: String,
    name: String,
    group: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PhotoEntry {
    id: String,
    w: u32,
    h: u32,
    title: String,
    folder: String,
    /// 名簿(xlsx)に登録済みの「写っている人」
    seed: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    title: String,
    groups: Vec<String>,
    roster: Vec<RosterEntry>,
    photos: Vec<PhotoEntry>,
    threshold: u32,
    low_weight: f64,
    majority: f64,
}

/// ローカル専用キャッシュ（元ファイルの場所を含むので git には入れない）
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Cache {
    settings: String,
    pub photos: HashMap<String, CachedPhoto>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CachedPhoto {
    w: u32,
    h: u32,
    hash: Vec<u8>,
    pub src: String,
}

pub fn load_cache(root: &Path) -> Cache {
    std::fs::read_to_string(root.join(CACHE_FILE))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn person_id(name: &str) -> String {
    format!("p{}", &blake3::hash(name.as_bytes()).to_hex()[..10])
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn open_image(bytes: &[u8]) -> Result<DynamicImage> {
    let mut dec = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?.into_decoder()?;
    let orientation = dec.orientation()?;
    let mut img = DynamicImage::from_decoder(dec)?;
    img.apply_orientation(orientation);
    Ok(img)
}

fn shrink(img: &DynamicImage, max: u32) -> DynamicImage {
    if img.width().max(img.height()) <= max {
        img.clone()
    } else {
        img.resize(max, max, FilterType::Lanczos3)
    }
}

/// 同一写真の判定用の小さな指紋
fn fingerprint(thumb: &DynamicImage) -> Vec<u8> {
    thumb.resize_exact(16, 16, FilterType::Triangle).to_luma8().into_raw()
}

fn diff(a: &[u8], b: &[u8]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return f64::MAX;
    }
    a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).abs() as f64).sum::<f64>() / a.len() as f64
}

fn to_jpeg(img: &DynamicImage, q: u8) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let rgb = img.to_rgb8(); // 再エンコードで EXIF(位置情報など)は落ちる
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, q).encode_image(&rgb)?;
    Ok(buf)
}

fn scan(dirs: &[PathBuf], warnings: &mut Vec<String>) -> Vec<(PathBuf, String)> {
    let mut out = vec![];
    for d in dirs {
        if !d.is_dir() {
            warnings.push(format!("フォルダーが見つかりません: {}", d.display()));
            continue;
        }
        let folder_name = d.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        for e in walkdir::WalkDir::new(d).follow_links(true).into_iter().filter_map(|e| e.ok()) {
            if !e.file_type().is_file() {
                continue;
            }
            let name = e.file_name().to_string_lossy();
            if name.starts_with('.') || name.starts_with("~$") {
                continue;
            }
            let ext = e.path().extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
            if EXTS.contains(&ext.as_str()) {
                let sub = e.path().parent().and_then(|p| p.strip_prefix(d).ok()).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                let folder = if sub.is_empty() { folder_name.clone() } else { format!("{folder_name}/{}", sub.replace('\\', "/")) };
                out.push((e.path().to_path_buf(), folder));
            } else if ext == "heic" {
                warnings.push(format!("HEIC は未対応のためスキップ: {}", e.path().display()));
            }
        }
    }
    out.sort();
    out
}

pub fn build(root: &Path, cfg: &Config, progress: &SharedProgress) -> Result<Report> {
    let t0 = Instant::now();
    let errs = cfg.validate();
    if !errs.is_empty() {
        bail!(errs.join(" / "));
    }
    let store = cfg.store_path(root);
    let img_dir = store.join("img");
    std::fs::create_dir_all(&img_dir).with_context(|| format!("保存先を作れません: {}", store.display()))?;
    let mut warnings = vec![];

    let settings_fp = format!("{}:{}:{}", cfg.max_size, cfg.thumb_size, cfg.jpeg_quality);

    let mut cache = load_cache(root);
    let cache_valid = cache.settings == settings_fp;
    if !cache_valid && !cache.photos.is_empty() {
        log(progress, "画質の設定が変わったため、すべての写真を作り直します");
    }

    // --- 名簿
    let roster: Vec<Person> = if cfg.roster_path.trim().is_empty() {
        warnings.push("名簿が設定されていません（固定の選択肢だけになります）".into());
        vec![]
    } else {
        let p = Config::resolve(root, &cfg.roster_path);
        log(progress, format!("名簿を読み込んでいます: {}", p.display()));
        roster::load(&p)?
    };
    log(progress, format!("名簿: {} 名", roster.len()));

    // --- 写真
    let dirs: Vec<PathBuf> = cfg.photo_dirs.iter().filter(|d| !d.trim().is_empty()).map(|d| Config::resolve(root, d)).collect();
    log(progress, "写真フォルダーを走査しています…");
    let files = scan(&dirs, &mut warnings);
    if files.is_empty() {
        bail!("写真が 1 枚も見つかりませんでした（{}）", dirs.iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join(", "));
    }
    {
        let mut p = progress.lock().unwrap();
        p.total = files.len();
        p.done = 0;
        p.phase = "写真を縮小しています".into();
    }
    log(progress, format!("写真: {} 枚", files.len()));

    let done = AtomicUsize::new(0);
    let encoded = AtomicUsize::new(0);
    let results: Vec<Result<(PhotoEntry, CachedPhoto)>> = files
        .par_iter()
        .map(|(path, folder)| {
            let r = (|| -> Result<(PhotoEntry, CachedPhoto)> {
                let bytes = std::fs::read(path).with_context(|| format!("読めません: {}", path.display()))?;
                let id = blake3::hash(&bytes).to_hex()[..16].to_string();
                let title = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let full_p = img_dir.join(format!("{id}.jpg"));
                let thumb_p = img_dir.join(format!("{id}_t.jpg"));
                if cache_valid {
                    if let Some(c) = cache.photos.get(&id) {
                        if full_p.exists() && thumb_p.exists() {
                            let mut c = c.clone();
                            c.src = path.to_string_lossy().into_owned();
                            return Ok((PhotoEntry { id, w: c.w, h: c.h, title, folder: folder.clone(), seed: vec![] }, c));
                        }
                    }
                }
                let img = open_image(&bytes).with_context(|| format!("画像として開けません: {}", path.display()))?;
                let full = shrink(&img, cfg.max_size);
                let thumb = shrink(&full, cfg.thumb_size);
                std::fs::write(&full_p, to_jpeg(&full, cfg.jpeg_quality)?)?;
                std::fs::write(&thumb_p, to_jpeg(&thumb, 78)?)?;
                encoded.fetch_add(1, Ordering::Relaxed);
                let c = CachedPhoto { w: full.width(), h: full.height(), hash: fingerprint(&thumb), src: path.to_string_lossy().into_owned() };
                Ok((PhotoEntry { id, w: c.w, h: c.h, title, folder: folder.clone(), seed: vec![] }, c))
            })();
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            progress.lock().unwrap().done = n;
            r
        })
        .collect();

    let mut photos: Vec<PhotoEntry> = vec![];
    let mut new_cache: HashMap<String, CachedPhoto> = HashMap::new();
    for r in results {
        match r {
            Ok((p, c)) => {
                if new_cache.contains_key(&p.id) {
                    warnings.push(format!("同じ写真が重複しています（1 枚として扱います）: {}", c.src));
                    continue;
                }
                new_cache.insert(p.id.clone(), c);
                photos.push(p);
            }
            Err(e) => warnings.push(format!("{e:#}")),
        }
    }
    if photos.is_empty() {
        bail!("写真を 1 枚も処理できませんでした");
    }

    // --- 名簿のセル内画像 → どの写真か照合して「登録済み」として付ける
    progress.lock().unwrap().phase = "名簿の登録済み写真を照合しています".into();
    let mut seed_links = 0;
    let mut unmatched_refs = 0;
    let mut seeds: HashMap<String, Vec<String>> = HashMap::new();
    let ref_hashes: Vec<(String, Vec<u8>)> = roster
        .par_iter()
        .flat_map(|p| {
            let pid = person_id(&p.name);
            p.images
                .par_iter()
                .filter_map(move |b| open_image(b).ok().map(|img| (pid.clone(), fingerprint(&shrink(&img, cfg.thumb_size)))))
        })
        .collect();
    for (pid, h) in &ref_hashes {
        let best = new_cache.iter().map(|(id, c)| (diff(h, &c.hash), id)).min_by(|a, b| a.0.total_cmp(&b.0));
        match best {
            Some((d, id)) if d < SAME_PHOTO => {
                let v = seeds.entry(id.clone()).or_default();
                if !v.contains(pid) {
                    v.push(pid.clone());
                    seed_links += 1;
                }
            }
            _ => unmatched_refs += 1,
        }
    }
    if unmatched_refs > 0 {
        warnings.push(format!("名簿に貼られた写真のうち {unmatched_refs} 枚は、写真フォルダー内に同じ写真が見つかりませんでした"));
    }
    for p in photos.iter_mut() {
        if let Some(s) = seeds.remove(&p.id) {
            p.seed = s;
        }
    }
    photos.sort_by(|a, b| (&a.folder, &a.title).cmp(&(&b.folder, &b.title)));

    // --- 使われなくなった画像を削除
    let keep: HashSet<String> = photos.iter().flat_map(|p| [format!("{}.jpg", p.id), format!("{}_t.jpg", p.id)]).collect();
    let mut removed = 0;
    for e in std::fs::read_dir(&img_dir)?.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if n.ends_with(".jpg") && !keep.contains(&n) {
            std::fs::remove_file(e.path()).ok();
            if !n.ends_with("_t.jpg") {
                removed += 1;
            }
        }
    }

    // --- マニフェスト
    let mut groups: Vec<String> = vec![];
    for p in &roster {
        if !p.group.is_empty() && !groups.contains(&p.group) {
            groups.push(p.group.clone());
        }
    }
    let mut seen = HashSet::new();
    let roster_entries: Vec<RosterEntry> = roster
        .iter()
        .filter(|p| seen.insert(p.name.clone()))
        .map(|p| RosterEntry { id: person_id(&p.name), name: p.name.clone(), group: p.group.clone() })
        .collect();
    let manifest = Manifest {
        title: cfg.title.clone(),
        groups,
        roster: roster_entries,
        photos: photos.clone(),
        threshold: cfg.threshold.max(1),
        low_weight: cfg.low_weight,
        majority: cfg.majority,
    };
    std::fs::write(store.join("manifest.json"), serde_json::to_vec(&manifest)?)?;
    write_public(root, cfg, photos.len())?;

    cache.settings = settings_fp;
    cache.photos = new_cache;
    std::fs::write(root.join(CACHE_FILE), serde_json::to_string(&cache)?)?;

    let enc = encoded.load(Ordering::Relaxed);
    let report = Report {
        photos: photos.len(),
        encoded: enc,
        reused: photos.len().saturating_sub(enc),
        removed,
        people: manifest.roster.len(),
        seed_links,
        unmatched_refs,
        seconds: t0.elapsed().as_secs_f64(),
        warnings,
    };
    log(
        progress,
        format!(
            "完了: 写真 {} 枚（新規 {} / 再利用 {} / 削除 {}）、名簿 {} 名、登録済みの紐付け {} 件、{:.1} 秒",
            report.photos, report.encoded, report.reused, report.removed, report.people, report.seed_links, report.seconds
        ),
    );
    for w in cfg.publish_warnings() {
        log(progress, format!("公開前に: {w}"));
    }
    for w in &report.warnings {
        log(progress, format!("注意: {w}"));
    }
    Ok(report)
}

/// 原寸の元ファイルをフォルダーへコピー（管理画面の「原寸を書き出す」）
pub fn export_originals(root: &Path, ids: &[String], dest: &Path) -> Result<BTreeMap<String, String>> {
    let cache = load_cache(root);
    std::fs::create_dir_all(dest)?;
    let mut out = BTreeMap::new();
    for id in ids {
        let Some(c) = cache.photos.get(id) else {
            out.insert(id.clone(), "元ファイルの場所が不明です（先に構築してください）".into());
            continue;
        };
        let src = PathBuf::from(&c.src);
        let name = src.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| format!("{id}.jpg"));
        let mut target = dest.join(&name);
        let mut k = 2;
        while target.exists() {
            target = dest.join(format!("{k}_{name}"));
            k += 1;
        }
        match std::fs::copy(&src, &target) {
            Ok(_) => out.insert(id.clone(), target.to_string_lossy().into_owned()),
            Err(e) => out.insert(id.clone(), format!("コピーできません: {e}")),
        };
    }
    Ok(out)
}
