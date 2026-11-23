//! 名簿の読み込み
//!
//! `写真登録.xlsx` の形式:
//!   B列=名前, C列=部署（1行目に「部署」の見出し）, F列以降=その人が写っている写真（セル内画像）
//! セル内画像（Excel の「セルに配置」画像 / richData）も取り出して、登録済みデータとして使う。
//! CSV の場合は 1 行目が見出し（名前/氏名, 部署/グループ/所属）。

use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Person {
    pub name: String,
    pub group: String,
    /// その人が写っている写真（xlsx のセル内画像の生データ）
    pub images: Vec<Vec<u8>>,
}

pub fn load(path: &Path) -> Result<Vec<Person>> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "xlsx" | "xlsm" => load_xlsx(path),
        "csv" | "txt" => load_csv(path),
        _ => bail!("名簿は .xlsx か .csv を指定してください: {}", path.display()),
    }
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn is_name_header(s: &str) -> bool {
    matches!(s.trim(), "名前" | "氏名" | "なまえ" | "name" | "Name")
}
fn is_group_header(s: &str) -> bool {
    matches!(s.trim(), "部署" | "グループ" | "所属" | "班" | "学年" | "group" | "Group")
}

fn load_csv(path: &Path) -> Result<Vec<Person>> {
    let raw = std::fs::read(path)?;
    let text = String::from_utf8(raw.clone()).map_err(|_| anyhow!("CSV は UTF-8 で保存してください"))?;
    let text = text.trim_start_matches('\u{feff}');
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or("").split(',').collect();
    let ni = header.iter().position(|h| is_name_header(h)).unwrap_or(0);
    let gi = header.iter().position(|h| is_group_header(h));
    let mut out = vec![];
    for l in lines {
        let cols: Vec<&str> = l.split(',').map(|c| c.trim().trim_matches('"')).collect();
        let name = cols.get(ni).copied().unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }
        let group = gi.and_then(|g| cols.get(g)).copied().unwrap_or("").to_string();
        out.push(Person { name: name.into(), group, images: vec![] });
    }
    Ok(out)
}

struct Xlsx {
    zip: zip::ZipArchive<std::fs::File>,
}

impl Xlsx {
    fn read(&mut self, name: &str) -> Option<Vec<u8>> {
        let mut f = self.zip.by_name(name).ok()?;
        let mut v = vec![];
        f.read_to_end(&mut v).ok()?;
        Some(v)
    }
    fn text(&mut self, name: &str) -> Option<String> {
        self.read(name).map(|b| String::from_utf8_lossy(&b).into_owned())
    }
}

/// rels ファイル (Id → Target)
fn parse_rels(xml: &str) -> HashMap<String, String> {
    let re = Regex::new(r#"<Relationship\b[^>]*>"#).unwrap();
    let id = Regex::new(r#"\bId="([^"]+)""#).unwrap();
    let tg = Regex::new(r#"\bTarget="([^"]+)""#).unwrap();
    re.find_iter(xml)
        .filter_map(|m| {
            let s = m.as_str();
            Some((id.captures(s)?[1].to_string(), tg.captures(s)?[1].to_string()))
        })
        .collect()
}

/// base ディレクトリ(例 "xl/richData")からの相対パスを解決
fn join_zip(base: &str, target: &str) -> String {
    if let Some(t) = target.strip_prefix('/') {
        return t.to_string();
    }
    let mut parts: Vec<&str> = base.split('/').filter(|s| !s.is_empty()).collect();
    for seg in target.split('/') {
        match seg {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn col_index(col: &str) -> usize {
    col.bytes().fold(0, |acc, b| acc * 26 + (b - b'A' + 1) as usize) - 1
}

fn load_xlsx(path: &Path) -> Result<Vec<Person>> {
    let f = std::fs::File::open(path).with_context(|| format!("名簿を開けません: {}", path.display()))?;
    let mut x = Xlsx { zip: zip::ZipArchive::new(f).context("xlsx を読めません（Excel で開いたままでも読めますが、壊れていないか確認してください）")? };

    // 共有文字列（ふりがな <rPh> は除く）
    let mut shared = vec![];
    if let Some(ss) = x.text("xl/sharedStrings.xml") {
        let si = Regex::new(r"(?s)<si>(.*?)</si>").unwrap();
        let rph = Regex::new(r"(?s)<rPh\b.*?</rPh>").unwrap();
        let t = Regex::new(r"(?s)<t(?:\s[^>]*)?>(.*?)</t>").unwrap();
        for c in si.captures_iter(&ss) {
            let body = rph.replace_all(&c[1], "");
            shared.push(unescape(&t.captures_iter(&body).map(|m| m[1].to_string()).collect::<String>()));
        }
    }

    // 最初のシート
    let wb = x.text("xl/workbook.xml").ok_or_else(|| anyhow!("workbook.xml がありません"))?;
    let wb_rels = parse_rels(&x.text("xl/_rels/workbook.xml.rels").unwrap_or_default());
    let first_rid = Regex::new(r#"<sheet\b[^>]*\br:id="([^"]+)""#).unwrap()
        .captures(&wb)
        .map(|c| c[1].to_string());
    let sheet_path = first_rid
        .and_then(|r| wb_rels.get(&r).cloned())
        .map(|t| join_zip("xl", &t))
        .unwrap_or_else(|| "xl/worksheets/sheet1.xml".into());
    let sheet = x.text(&sheet_path).ok_or_else(|| anyhow!("{sheet_path} がありません"))?;

    // セル内画像: vm(1始まり) → valueMetadata → futureMetadata → rdrichvalue → richValueRel → 画像ファイル
    let images_by_vm = rich_images(&mut x).unwrap_or_default();

    // セルを読む
    let row_re = Regex::new(r#"(?s)<row\b[^>]*\br="(\d+)"[^>]*>(.*?)</row>"#).unwrap();
    let cell_re = Regex::new(r#"(?s)<c\b([^>]*?)(?:/>|>(.*?)</c>)"#).unwrap();
    let r_re = Regex::new(r#"\br="([A-Z]+)(\d+)""#).unwrap();
    let t_re = Regex::new(r#"\bt="([^"]+)""#).unwrap();
    let vm_re = Regex::new(r#"\bvm="(\d+)""#).unwrap();
    let v_re = Regex::new(r"(?s)<v>(.*?)</v>").unwrap();
    let is_re = Regex::new(r"(?s)<t(?:\s[^>]*)?>(.*?)</t>").unwrap();

    struct Cell {
        text: Option<String>,
        vm: Option<usize>,
    }
    let mut rows: Vec<(usize, HashMap<usize, Cell>)> = vec![];
    for rc in row_re.captures_iter(&sheet) {
        let rnum: usize = rc[1].parse().unwrap_or(0);
        let mut cells = HashMap::new();
        for cc in cell_re.captures_iter(&rc[2]) {
            let attrs = &cc[1];
            let body = cc.get(2).map(|m| m.as_str()).unwrap_or("");
            let Some(r) = r_re.captures(attrs) else { continue };
            let col = col_index(&r[1]);
            let t = t_re.captures(attrs).map(|c| c[1].to_string());
            let vm = vm_re.captures(attrs).and_then(|c| c[1].parse().ok());
            let v = v_re.captures(body).map(|c| unescape(&c[1]));
            let text = match t.as_deref() {
                Some("s") => v.and_then(|i| i.parse::<usize>().ok()).and_then(|i| shared.get(i).cloned()),
                Some("inlineStr") => Some(unescape(&is_re.captures_iter(body).map(|m| m[1].to_string()).collect::<String>())),
                Some("e") => None,
                _ => v,
            };
            cells.insert(col, Cell { text, vm });
        }
        rows.push((rnum, cells));
    }

    // 見出し行と列の決定
    let mut name_col = None;
    let mut group_col = None;
    let mut header_row = 0;
    for (rnum, cells) in rows.iter().take(5) {
        for (c, cell) in cells {
            if let Some(t) = &cell.text {
                if is_name_header(t) {
                    name_col = Some(*c);
                    header_row = *rnum;
                }
                if is_group_header(t) {
                    group_col = Some(*c);
                    header_row = *rnum;
                }
            }
        }
        if group_col.is_some() || name_col.is_some() {
            break;
        }
    }
    let name_col = match (name_col, group_col) {
        (Some(n), _) => n,
        (None, Some(g)) if g > 0 => g - 1, // 写真登録.xlsx: 名前列に見出しが無く、部署の左隣
        _ => 1,                            // 見出しが無ければ B 列
    };

    let mut people = vec![];
    for (rnum, cells) in &rows {
        if *rnum <= header_row {
            continue;
        }
        let Some(name) = cells.get(&name_col).and_then(|c| c.text.clone()) else { continue };
        let name = name.trim().to_string();
        if name.is_empty() || name.parse::<f64>().is_ok() {
            continue;
        }
        let group = group_col
            .and_then(|g| cells.get(&g))
            .and_then(|c| c.text.clone())
            .unwrap_or_default()
            .trim()
            .to_string();
        let mut vms: Vec<(usize, usize)> = cells.iter().filter_map(|(c, cell)| cell.vm.map(|v| (*c, v))).collect();
        vms.sort();
        let mut images = vec![];
        for (_, vm) in vms {
            if let Some(path) = images_by_vm.get(&vm) {
                if let Some(bytes) = x.read(path) {
                    images.push(bytes);
                }
            }
        }
        people.push(Person { name, group, images });
    }
    if people.is_empty() {
        bail!("名簿から名前を読み取れませんでした（1 行目に「部署」または「名前」の見出しがあるか確認してください）");
    }
    Ok(people)
}

fn rich_images(x: &mut Xlsx) -> Option<HashMap<usize, String>> {
    let meta = x.text("xl/metadata.xml")?;
    let rv = x.text("xl/richData/rdrichvalue.xml")?;
    let rel = x.text("xl/richData/richValueRel.xml")?;
    let rel_rels = parse_rels(&x.text("xl/richData/_rels/richValueRel.xml.rels")?);

    let bk = Regex::new(r"(?s)<bk>(.*?)</bk>").unwrap();
    let section = |tag: &str| -> String {
        let re = Regex::new(&format!(r"(?s)<{tag}\b[^>]*>(.*?)</{tag}>")).unwrap();
        re.captures(&meta).map(|c| c[1].to_string()).unwrap_or_default()
    };
    let rc_v = Regex::new(r#"<rc\b[^>]*\bv="(\d+)""#).unwrap();
    let rvb_i = Regex::new(r#"<xlrd:rvb\b[^>]*\bi="(\d+)""#).unwrap();

    let value_meta: Vec<usize> = bk
        .captures_iter(&section("valueMetadata"))
        .map(|c| rc_v.captures(&c[1]).and_then(|m| m[1].parse().ok()).unwrap_or(usize::MAX))
        .collect();
    let future: Vec<usize> = bk
        .captures_iter(&section("futureMetadata"))
        .map(|c| rvb_i.captures(&c[1]).and_then(|m| m[1].parse().ok()).unwrap_or(usize::MAX))
        .collect();
    let rv_re = Regex::new(r"(?s)<rv\b[^>]*>(.*?)</rv>").unwrap();
    let v_re = Regex::new(r"<v>(\d+)</v>").unwrap();
    let rich: Vec<usize> = rv_re
        .captures_iter(&rv)
        .map(|c| v_re.captures(&c[1]).and_then(|m| m[1].parse().ok()).unwrap_or(usize::MAX))
        .collect();
    let rel_ids: Vec<String> = Regex::new(r#"<rel\b[^>]*\br:id="([^"]+)""#)
        .unwrap()
        .captures_iter(&rel)
        .map(|c| c[1].to_string())
        .collect();

    let mut out = HashMap::new();
    for (i, fut) in value_meta.iter().enumerate() {
        let vm = i + 1;
        let target = future
            .get(*fut)
            .and_then(|ri| rich.get(*ri))
            .and_then(|li| rel_ids.get(*li))
            .and_then(|rid| rel_rels.get(rid));
        if let Some(t) = target {
            out.insert(vm, join_zip("xl/richData", t));
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths() {
        assert_eq!(join_zip("xl/richData", "../media/image1.jpeg"), "xl/media/image1.jpeg");
        assert_eq!(join_zip("xl", "worksheets/sheet1.xml"), "xl/worksheets/sheet1.xml");
        assert_eq!(join_zip("xl", "/xl/worksheets/sheet1.xml"), "xl/worksheets/sheet1.xml");
        assert_eq!(col_index("A"), 0);
        assert_eq!(col_index("F"), 5);
        assert_eq!(col_index("AA"), 26);
    }
}
