//! Dữ liệu cho Màn 5 (spec `05-timeline.md`): quét `timeline/` của bộ, đọc nhanh `.fcproj` (JSON), dấu "máy dựng lúc …",
//! và luật cập nhật không phá công sửa tay (chương đã sửa tay → `chNNN.v2.fcproj`, bản cũ nguyên vẹn).
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::chuong::{doc_json, ten_file};

/// 1 giây trong đơn vị thời gian của FilmCraft.
const TICK: f64 = 254_016_000_000.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Nhan {
    Moi,
    DaSuaTay,
    CuHonKichBan,
}

#[derive(Clone)]
pub struct Clip {
    pub bat_dau: f64,
    pub dai: f64,
    pub ten: String,
}

#[derive(Clone, Default)]
pub struct Lane {
    pub ten: String,
    pub clip: Vec<Clip>,
}

#[derive(Clone)]
pub struct MucTl {
    pub nhom: String,
    pub ma: String,
    pub file: PathBuf,
    pub giay: f64,
    pub so_clip: usize,
    pub so_seq: usize,
    pub nhan: Vec<Nhan>,
    pub gio_dung: f64,
    pub v2: bool,
    pub lane: Arc<Vec<Lane>>,
    /// khoá chọn trong cây: đường dẫn file (kiểu cũ) hoặc `<dự án>#<nhóm>/<chNNN>` (kiểu B, nhiều chương chung 1 file)
    pub id: String,
    /// kiểu B: tên phần ("Phần 2"), số phần, giây bắt đầu của chương trên dải phần
    pub phan: String,
    pub so_phan: u64,
    pub t0: f64,
}

pub fn fnv(b: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in b {
        h ^= u64::from(*x);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn mtime(p: &Path) -> f64 {
    std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0.0, |d| d.as_secs_f64())
}

fn so_tick(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0) / TICK
}

/// Đọc `.fcproj`: sequence chính (trùng tên mã, hoặc sequence đầu), tổng clip, lane để vẽ.
pub fn doc_fcproj(raw: &[u8], ma: &str) -> Option<(f64, usize, usize, Vec<Lane>)> {
    let j: Value = serde_json::from_slice(raw).ok()?;
    let items = j.pointer("/project/items")?.as_object()?;
    let seqs: Vec<(&String, &Value)> = items.iter().filter(|(_, v)| v.pointer("/kind/Sequence").is_some()).collect();
    let dem = |s: &Value| -> usize { ["video_tracks", "audio_tracks"].iter().filter_map(|k| s.get(k)?.as_array()).flatten().map(|t| t.get("items").and_then(Value::as_array).map_or(0, Vec::len)).sum() };
    let so_clip: usize = seqs.iter().filter_map(|(_, v)| v.pointer("/kind/Sequence")).map(dem).sum();
    let chinh = seqs.iter().find(|(_, v)| v.get("name").and_then(Value::as_str).is_some_and(|n| n.starts_with(ma))).or_else(|| seqs.first())?;
    let s = chinh.1.pointer("/kind/Sequence")?;
    let mut lane = vec![];
    let mut giay = 0.0f64;
    for (loai, k) in [("Hình", "video_tracks"), ("Tiếng", "audio_tracks")] {
        for (i, t) in s.get(k).and_then(Value::as_array).into_iter().flatten().enumerate() {
            let clip: Vec<Clip> = t
                .get("items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|c| Clip { bat_dau: so_tick(&c["start"]), dai: so_tick(&c["duration"]), ten: c.get("name").and_then(Value::as_str).unwrap_or("").to_owned() })
                .collect();
            giay = clip.iter().map(|c| c.bat_dau + c.dai).fold(giay, f64::max);
            if !clip.is_empty() {
                lane.push(Lane { ten: format!("{loai} {}", i + 1), clip });
            }
        }
    }
    Some((giay, so_clip, seqs.len(), lane))
}

fn ma_chuong(ten: &str) -> Option<&str> {
    let (ma, _) = ten.split_once('.')?;
    (ma.starts_with("ch") || ma.starts_with("phan") || ma == "full").then_some(ma)
}

/// Quét `timeline/` (và `timeline/viet|en|noi-dung/`). `bo_nho`: đường dẫn → (mtime, mục) để không đọc lại file không đổi.
pub fn quet(bo: &Path, bo_nho: &mut HashMap<PathBuf, (f64, MucTl)>) -> Vec<MucTl> {
    let goc = bo.join("timeline");
    if let Some(v) = quet_du_an(bo) {
        return v;
    }
    let mut kq = vec![];
    let thu_muc: Vec<(String, PathBuf)> = vec![("viet".into(), goc.clone()), ("viet".into(), goc.join("viet")), ("en".into(), goc.join("en")), ("noi-dung".into(), goc.join("noi-dung"))];
    for (nhom, dir) in thu_muc {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "fcproj")).collect();
        files.sort();
        for f in files {
            let t = ten_file(&f);
            let Some(ma) = ma_chuong(&t).map(str::to_owned).or_else(|| (nhom == "noi-dung").then(|| t.trim_end_matches(".fcproj").to_owned())) else { continue };
            let m = mtime(&f);
            if let Some((c, muc)) = bo_nho.get(&f) {
                if (*c - m).abs() < 1e-6 {
                    let mut muc = muc.clone();
                    gan_nhan(bo, &mut muc);
                    kq.push(muc);
                    continue;
                }
            }
            let Ok(raw) = std::fs::read(&f) else { continue };
            let Some((giay, so_clip, so_seq, lane)) = doc_fcproj(&raw, &ma) else { continue };
            let mut muc = MucTl { nhom: nhom.clone(), ma: ma.clone(), file: f.clone(), giay, so_clip, so_seq, nhan: vec![], gio_dung: 0.0, v2: t.contains(".v"), lane: Arc::new(lane), id: f.to_string_lossy().into_owned(), phan: String::new(), so_phan: 0, t0: 0.0 };
            muc.v2 = t.trim_end_matches(".fcproj").rsplit('.').next().is_some_and(|x| x.starts_with('v') && x[1..].chars().all(|c| c.is_ascii_digit()));
            bo_nho.insert(f.clone(), (m, muc.clone()));
            gan_nhan(bo, &mut muc);
            kq.push(muc);
        }
    }
    kq
}

static CACHE_DU_AN: Mutex<Option<(PathBuf, f64, Vec<MucTl>)>> = Mutex::new(None);

/// kiểu B (T621): `<bộ>/timeline/<bộ>.fcproj`. Vị trí/nhãn "đã sửa tay" hỏi `recap-studio trang-thai` (1 nguồn tính dấu);
/// lane vẽ lấy thẳng từ dải phần trong file. Đọc lại chỉ khi file đổi.
fn quet_du_an(bo: &Path) -> Option<Vec<MucTl>> {
    let ten = ten_file(bo);
    let f = bo.join("timeline").join(format!("{ten}.fcproj"));
    let meta = f.with_extension("may-dung.json");
    let m = mtime(&f).max(mtime(&meta));
    if m == 0.0 {
        return None;
    }
    let mut c = CACHE_DU_AN.lock().ok()?;
    let mut v = match c.as_ref() {
        Some((p, t, v)) if *p == f && (*t - m).abs() < 1e-6 => v.clone(),
        _ => {
            let tt: Value = {
                let o = std::process::Command::new(tim_lenh()?).arg("trang-thai").arg(goc_dung(bo)).env("RECAP_TIMELINE_DIR", bo.join("timeline")).output().ok()?;
                serde_json::from_slice(&o.stdout).ok()?
            };
            let j: Value = serde_json::from_slice(&std::fs::read(&f).ok()?).ok()?;
            let mut lane_phan: HashMap<i64, Arc<Vec<Lane>>> = HashMap::new();
            let mut ds = vec![];
            for e in tt["chuong"].as_array()? {
                let (seq, nhom, ma) = (e["seq"].as_i64()?, e["nhom"].as_str()?.to_owned(), e["ma"].as_str()?.to_owned());
                let lane = lane_phan.entry(seq).or_insert_with(|| Arc::new(lane_cua_seq(&j, seq))).clone();
                let so_clip = lane.iter().map(|l| l.clip.len()).sum();
                let (giay, t0) = (e["dai"].as_f64().unwrap_or(0.0), e["bat_dau"].as_f64().unwrap_or(0.0));
                let so_phan = e["phan"].as_u64().unwrap_or(0);
                let mut nhan = vec![];
                if e["da_sua_tay"].as_bool() == Some(true) {
                    nhan.push(Nhan::DaSuaTay);
                }
                ds.push(MucTl {
                    id: format!("{}#{nhom}/{ma}", f.display()),
                    phan: format!("Phần {so_phan}"),
                    nhom: if nhom == "viet" { "viet".into() } else { nhom },
                    ma, file: f.clone(), giay, so_clip, so_seq: 1, nhan, gio_dung: e["luc_s"].as_f64().unwrap_or(m), v2: false, lane, so_phan, t0,
                });
            }
            ds.sort_by(|a, b| (&a.nhom, a.so_phan, &a.ma).cmp(&(&b.nhom, b.so_phan, &b.ma)));
            *c = Some((f.clone(), m, ds.clone()));
            ds
        }
    };
    drop(c);
    for x in &mut v {
        let kb = crate::chuong::tim_kich_ban(bo, &x.ma).map_or(0.0, |p| mtime(&p));
        let moi = kb.max(mtime(&bo.join("khung").join(format!("{}.json", x.ma)))).max(mtime(&bo.join("che-chu").join(format!("{}.json", x.ma))));
        x.nhan.retain(|n| *n != Nhan::CuHonKichBan && *n != Nhan::Moi);
        if moi > x.gio_dung + 1.0 {
            x.nhan.push(Nhan::CuHonKichBan);
        }
        if x.nhan.is_empty() {
            x.nhan.push(Nhan::Moi);
        }
    }
    Some(v)
}

/// lane (track không rỗng) của 1 sequence theo id — thời gian TUYỆT ĐỐI trên dải
fn lane_cua_seq(j: &Value, seq: i64) -> Vec<Lane> {
    let mut lane = vec![];
    let Some(s) = j.pointer(&format!("/project/items/{seq}/kind/Sequence")) else { return lane };
    for (loai, k) in [("Hình", "video_tracks"), ("Tiếng", "audio_tracks")] {
        for (i, t) in s.get(k).and_then(Value::as_array).into_iter().flatten().enumerate() {
            let clip: Vec<Clip> = t["items"].as_array().into_iter().flatten().map(|c| Clip { bat_dau: so_tick(&c["start"]), dai: so_tick(&c["duration"]), ten: c["name"].as_str().unwrap_or("").to_owned() }).collect();
            if !clip.is_empty() {
                lane.push(Lane { ten: format!("{loai} {}", i + 1), clip });
            }
        }
    }
    lane
}

/// Chọn dự án bộ cho lệnh `recap-studio mo-phan` rồi mở FilmCraft đúng phần.
pub fn mo_phan_filmcraft(bo: &Path, nhom: &str, so_phan: u64, f: &Path) -> Result<(), String> {
    let o = std::process::Command::new(tim_lenh().ok_or("không thấy lệnh recap-studio")?)
        .args(["mo-phan"]).arg(goc_dung(bo)).args(["--nhom", nhom, "--phan", &so_phan.to_string()])
        .env("RECAP_TIMELINE_DIR", bo.join("timeline")).output().map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("mo-phan lỗi").to_owned());
    }
    mo_filmcraft(f).map_err(|e| e.to_string())
}

fn dau_may_path(f: &Path) -> PathBuf {
    f.with_extension("dau-may.json")
}

/// Gắn nhãn Mới / Đã sửa tay / Cũ hơn kịch bản (đo từ file thật mỗi lần quét: sha file hiện tại ≠ sha lúc máy dựng = đã sửa tay).
fn gan_nhan(bo: &Path, m: &mut MucTl) {
    m.nhan.clear();
    let dau = doc_json(&dau_may_path(&m.file));
    let dung = dau.as_ref().and_then(|d| d.get("gio")?.as_f64());
    m.gio_dung = dung.unwrap_or_else(|| mtime(&m.file));
    if let Some(sha) = dau.as_ref().and_then(|d| d.get("sha_fcproj")?.as_str().map(str::to_owned)) {
        if std::fs::read(&m.file).map(|b| fnv(&b)).ok().as_deref() != Some(sha.as_str()) {
            m.nhan.push(Nhan::DaSuaTay);
        }
    }
    if m.ma.starts_with("ch") {
        let kb = crate::chuong::tim_kich_ban(bo, &m.ma).map_or(0.0, |p| mtime(&p));
        let moi = kb.max(mtime(&bo.join("khung").join(format!("{}.json", m.ma)))).max(mtime(&bo.join("che-chu").join(format!("{}.json", m.ma))));
        if moi > m.gio_dung + 1.0 {
            m.nhan.push(Nhan::CuHonKichBan);
        }
    }
    if m.nhan.is_empty() {
        m.nhan.push(Nhan::Moi);
    }
}

pub fn ten_nhan(n: &Nhan) -> &'static str {
    match n {
        Nhan::Moi => "Mới",
        Nhan::DaSuaTay => "Đã sửa tay",
        Nhan::CuHonKichBan => "Timeline cũ hơn kịch bản",
    }
}

/// Phần (arc) của chương theo tên thư mục `arcNN-chAAA-BBB` trong `kich-ban/`: (tên phần, từ, đến).
pub fn cac_phan(bo: &Path) -> Vec<(String, u32, u32)> {
    let mut v: Vec<(String, u32, u32)> = std::fs::read_dir(bo.join("kich-ban"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let t = ten_file(&e.path());
            let r = t.strip_prefix("arc")?;
            let so: String = r.chars().take_while(char::is_ascii_digit).collect();
            let (a, b) = r.split_once("-ch")?.1.split_once('-')?;
            Some((format!("Phần {}", so.trim_start_matches('0')), a.parse().ok()?, b.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()?))
        })
        .collect();
    v.sort();
    v
}

/// Gom mục timeline của nhóm `viet` theo phần.
pub fn cay_viet(bo: &Path, muc: &[MucTl]) -> BTreeMap<String, Vec<usize>> {
    let phan = cac_phan(bo);
    let mut m: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, x) in muc.iter().enumerate().filter(|(_, x)| x.nhom == "viet" && !x.phan.is_empty()) {
        m.entry(x.phan.clone()).or_default().push(i);
    }
    for (i, x) in muc.iter().enumerate().filter(|(_, x)| x.nhom == "viet" && x.phan.is_empty()) {
        let so: u32 = x.ma.trim_start_matches("ch").parse().unwrap_or(0);
        let ten = if x.ma.starts_with("ch") { phan.iter().find(|p| so >= p.1 && so <= p.2).map_or_else(|| "Chương chưa vào phần".to_owned(), |p| p.0.clone()) } else { "Phần / FULL".to_owned() };
        m.entry(ten).or_default().push(i);
    }
    m
}

// ---------- cập nhật từ dữ liệu ----------

pub fn tim_lenh() -> Option<PathBuf> {
    let ten = if cfg!(windows) { "recap-studio.exe" } else { "recap-studio" };
    if let Some(p) = std::env::var_os("RECAP_STUDIO_BIN").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(p);
    }
    let canh = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(ten))).filter(|p| p.is_file());
    canh.or_else(|| std::env::var_os("PATH").and_then(|v| std::env::split_paths(&v).map(|d| d.join(ten)).find(|p| p.is_file())))
}

/// Thư mục bộ mà lõi dựng đọc: lõi mốc này đọc ĐỊNH DẠNG CŨ, nên bộ đã chuyển dùng đường `chuyen_tu_cu.nguon` (chỉ đọc).
fn goc_dung(bo: &Path) -> PathBuf {
    doc_json(&bo.join("bo.json")).and_then(|j| j.pointer("/chuyen_tu_cu/nguon")?.as_str().map(PathBuf::from)).filter(|p| p.is_dir()).unwrap_or_else(|| bo.to_path_buf())
}

/// Cập nhật 1 chương vào dự án bộ bằng `recap-studio dung-bo` (máy tự giữ chương đã sửa tay trừ khi `thay`). Trả lời ngắn cho thanh báo.
pub fn cap_nhat_bo(bo: &Path, ma: &str, thay: bool) -> Result<String, String> {
    let lenh = tim_lenh().ok_or("Không thấy lệnh recap-studio (đặt RECAP_STUDIO_BIN hoặc để cạnh app)")?;
    let n = ma.trim_start_matches("ch");
    let cache = std::env::temp_dir().join("recap-studio").join("cache");
    let mut c = std::process::Command::new(lenh);
    c.arg("dung-bo").arg(goc_dung(bo)).args(["--chuong", &format!("{n}-{n}"), "--ep", "--cache"]).arg(&cache).env("RECAP_TIMELINE_DIR", bo.join("timeline"));
    if thay {
        c.arg("--thay-sua-tay");
    }
    let r = c.output().map_err(|e| e.to_string())?;
    if !r.status.success() {
        let e = String::from_utf8_lossy(&r.stderr);
        return Err(e.lines().last().unwrap_or("lõi dựng báo lỗi").trim_start_matches("recap-studio: ").to_owned());
    }
    let j: Value = serde_json::from_slice(&r.stdout).map_err(|e| e.to_string())?;
    let co = |k: &str| j[k].as_array().is_some_and(|a| !a.is_empty());
    Ok(if co("giu_sua_tay") { format!("{ma} đã sửa tay — giữ nguyên bản của bạn (muốn thay bằng bản mới: nút \"Thay bằng bản mới\")") } else if co("thieu_moc") { format!("{ma} chưa có mốc câu — chưa dựng được") } else { format!("Đã cập nhật {ma} trên dải phần") })
}

pub fn mo_filmcraft(f: &Path) -> std::io::Result<()> {
    let ten = if cfg!(windows) { "filmcraft.exe" } else { "filmcraft" };
    let canh = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(ten))).filter(|p| p.is_file());
    let p = canh.or_else(|| std::env::var_os("PATH").and_then(|v| std::env::split_paths(&v).map(|d| d.join(ten)).find(|p| p.is_file()))).ok_or_else(|| std::io::Error::other("không thấy FilmCraft (filmcraft) cạnh app hoặc trong PATH"))?;
    std::process::Command::new(p).arg(f).spawn().map(|_| ())
}

#[cfg(test)]
mod thu {
    use super::*;
    use serde_json::json;

    #[test]
    fn fcproj_toi_thieu() {
        let j = json!({"project": {"items": {"1": {"name": "ch001", "kind": {"Sequence": {"video_tracks": [{"items": [{"start": 0, "duration": 254016000000u64 * 3, "name": "a"}]}], "audio_tracks": []}}}}}});
        let (giay, clip, seq, lane) = doc_fcproj(j.to_string().as_bytes(), "ch001").unwrap();
        assert!((giay - 3.0).abs() < 1e-9 && clip == 1 && seq == 1 && lane.len() == 1);
        assert_eq!(lane_cua_seq(&j, 1).len(), 1);
    }
}
