//! Màn 6 — Xuất & gói đăng (spec `06-xuat-goi-dang.md`): danh sách video của bộ (FULL · Phần · Việt/EN), nút Xuất = thêm việc `xuat`
//! vào hàng việc dịch vụ nền (dịch vụ gọi `recap-studio xuat`), tiến độ + tự kiểm + sổ xuất, gói đăng 4 phần (tiêu đề · mô tả có
//! mốc chương máy tính · tag · thumbnail), Chép tiêu đề/mô tả, "Viết lại gói bằng agy" (chỉ tạo việc), đánh dấu Đã đăng vào `bo.json`.
//! Màn hình chỉ ĐỌC sổ xuất + `dang-youtube/`; chỉ GHI việc mới/`lenh/` và khoá `da_dang_video` của `bo.json`.
use egui::{Color32, Rect, RichText, Sense, Ui, Vec2};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::du_lieu::{bay_gio, doc_json};
use crate::giao_dien::*;
use crate::lenh;
use crate::{App, Man};

#[derive(Clone, Default, Debug)]
pub struct SoMuc {
    pub file: String,
    pub nhan: String,
    pub ban: String,
    pub giay: f64,
    pub mb: f64,
    pub luc: String,
    pub kiem: Vec<String>,
    pub dat: bool,
    pub mo_dau: f64,
    pub xuat_giay: f64,
    pub moc: Vec<(String, f64)>,
}

#[derive(Clone, Default, Debug)]
pub struct ViecXuat {
    pub id: String,
    pub trang_thai: String,
    pub tien_do: Option<f64>,
    pub gio_bat_dau: f64,
    pub gio_tao: f64,
    pub ly_do: String,
    pub nhan: String,
    pub ban: String,
}

#[derive(Clone, Default, Debug)]
pub struct DongVideo {
    pub nhan: String,
    pub ban: String,
    pub nn: u32,
    pub tu_den: Option<(u32, u32)>,
    pub so: Option<SoMuc>,
    pub viec: Option<ViecXuat>,
    pub da_dang: Option<(String, String)>,
}

impl DongVideo {
    pub fn khoa(&self) -> String {
        format!("{}|{}", self.ban, self.nhan)
    }
    fn ten(&self) -> String {
        if self.nn == 0 { "FULL".into() } else { format!("Phần {}", self.nn) }
    }
}

#[derive(Clone, Default, Debug)]
pub struct Goi {
    pub co_file: bool,
    pub tieu_de: Vec<String>,
    pub mo_ta: String,
    pub tag: Vec<String>,
    pub thumb_y: Vec<String>,
    pub thumb_anh: Vec<PathBuf>,
}

pub struct ManXuat {
    pub bo: PathBuf,
    pub ten: String,
    pub dong: Vec<DongVideo>,
    pub chon: Option<String>,
    pub ngon: String,
    pub goi: Goi,
    pub moc_ngoai: Vec<(f64, String)>,
    pub ra_dir: PathBuf,
    pub link: String,
    lan_cuoi: Option<Instant>,
    chon_goi: Option<String>,
}

fn nha() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map_or_else(|| PathBuf::from("."), PathBuf::from)
}

pub fn ra_dir() -> PathBuf {
    std::env::var_os("RECAP_RA_DIR").map(PathBuf::from).unwrap_or_else(|| nha().join("recap-thu-moc2"))
}

pub fn hms(giay: f64) -> String {
    let s = giay.max(0.0).round() as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60) } else { format!("{:02}:{:02}", s / 60, s % 60) }
}

fn f(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(0.0)
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_owned()
}

/// Đọc 1 file sổ xuất: hiểu cả sổ của `recap-studio xuat` (`nhan`, `giay`, `mb`, `moc_chuong`) lẫn sổ hệ cũ (`video`, `thoi_luong_s`, `dung_luong`).
pub fn doc_so_file(p: &Path) -> Vec<SoMuc> {
    let Some(Value::Array(a)) = doc_json(p) else { return vec![] };
    a.iter()
        .map(|e| {
            let nhan = if !s(e, "nhan").is_empty() { s(e, "nhan") } else if !s(e, "video").is_empty() { s(e, "video") } else { "FULL".into() };
            let (kiem, dat) = match e.get("kiem") {
                Some(Value::String(t)) => (if t == "đạt" { vec![] } else { vec![t.clone()] }, t == "đạt"),
                Some(Value::Array(l)) => (l.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect(), false),
                _ => (vec![], false),
            };
            let ban = match s(e, "ban").as_str() {
                "en" | "EN" => "en",
                "noi_dung" | "noidung" => "nd",
                _ => "viet",
            };
            SoMuc {
                file: s(e, "file"),
                nhan,
                ban: ban.into(),
                giay: if f(e, "giay") > 0.0 { f(e, "giay") } else { f(e, "thoi_luong_s") },
                mb: if f(e, "mb") > 0.0 { f(e, "mb") } else { f(e, "dung_luong") / 1e6 },
                luc: s(e, "luc"),
                kiem,
                dat,
                mo_dau: f(e, "mo_dau"),
                xuat_giay: f(e, "xuat_giay"),
                moc: e.get("moc_chuong").and_then(Value::as_array).map(|m| m.iter().map(|x| (s(x, "chuong"), f(x, "giay"))).collect()).unwrap_or_default(),
            }
        })
        .collect()
}

/// Mọi sổ xuất của bộ: `<ra-dir>/<tên>/so-xuat.json` (đích của `recap-studio xuat`) + `<bộ>/xuat/{so-xuat,_so-xuat}.json`.
pub fn doc_so(bo: &Path, ten: &str, ra: &Path) -> Vec<SoMuc> {
    let mut v = vec![];
    for p in [ra.join(ten).join("so-xuat.json"), bo.join("xuat").join("so-xuat.json"), bo.join("xuat").join("_so-xuat.json")] {
        for m in doc_so_file(&p) {
            if !v.iter().any(|x: &SoMuc| x.file == m.file && x.ban == m.ban) {
                v.push(m);
            }
        }
    }
    v
}

/// Việc `xuat` của bộ trong hàng việc (đọc thô vì `Viec` chung không giữ `tham_so.nhan`).
pub fn doc_viec_xuat(goc_dv: &Path, bo: &Path) -> Vec<ViecXuat> {
    let ten = bo.file_name().map(|n| n.to_os_string());
    let mut v = vec![];
    for e in std::fs::read_dir(goc_dv.join("hang-viec")).into_iter().flatten().flatten() {
        let p = e.path();
        let t = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !t.ends_with(".json") || t.ends_with(".ket-thuc.json") || t.starts_with('.') {
            continue;
        }
        let Some(j) = doc_json(&p) else { continue };
        if s(&j, "loai") != "xuat" || Path::new(&s(&j, "bo")).file_name().map(|n| n.to_os_string()) != ten {
            continue;
        }
        v.push(ViecXuat {
            id: s(&j, "id"),
            trang_thai: s(&j, "trang_thai"),
            tien_do: j.get("tien_do").and_then(Value::as_f64),
            gio_bat_dau: f(&j, "gio_bat_dau"),
            gio_tao: f(&j, "gio_tao"),
            ly_do: s(&j, "ly_do"),
            nhan: j.pointer("/tham_so/nhan").and_then(Value::as_str).unwrap_or("").to_owned(),
            ban: j.pointer("/tham_so/ban").and_then(Value::as_str).unwrap_or("viet").to_owned(),
        });
    }
    v.sort_by(|a, b| a.gio_tao.total_cmp(&b.gio_tao));
    v
}

/// Chia của bộ: `xuat.chia` trong `_du-an.json` hoặc `bo.json`. Chỉ chia "arc" cho biết trước các phần; còn lại chỉ thấy phần đã xuất + "phần tiếp".
fn chia_cua_bo(bo: &Path) -> String {
    ["_du-an.json", "bo.json"].iter().find_map(|t| doc_json(&bo.join(t)).and_then(|j| j.pointer("/xuat/chia").and_then(Value::as_str).map(str::to_owned))).unwrap_or_else(|| "full".into())
}

fn arcs(bo: &Path) -> Vec<(u32, u32, u32)> {
    let mut v: Vec<(u32, u32, u32)> = ["kich-ban", "Script"]
        .iter()
        .flat_map(|d| std::fs::read_dir(bo.join(d)).into_iter().flatten().flatten())
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            let p: Vec<&str> = n.strip_prefix("arc")?.split('-').collect();
            let [nn, a, b] = p[..] else { return None };
            Some((nn.parse().ok()?, a.strip_prefix("ch")?.parse().ok()?, b.parse().ok()?))
        })
        .collect();
    v.sort_by_key(|x| x.1);
    v.dedup();
    v
}

fn so_phan(nhan: &str) -> u32 {
    nhan.strip_prefix("phan").and_then(|r| r.get(..2)).and_then(|n| n.parse().ok()).unwrap_or(0)
}

fn khoang_chuong(nhan: &str) -> Option<(u32, u32)> {
    let r = nhan.split_once(" ch")?.1;
    let (a, b) = r.split_once('-')?;
    Some((a.parse().ok()?, b.trim_start_matches("ch").parse().ok()?))
}

fn da_dang_map(bo: &Path) -> Vec<(String, String, String)> {
    let Some(j) = doc_json(&bo.join("bo.json")) else { return vec![] };
    j.get("da_dang_video").and_then(Value::as_object).map(|o| o.iter().map(|(k, v)| (k.clone(), s(v, "ngay"), s(v, "link"))).collect()).unwrap_or_default()
}

/// Dựng danh sách dòng: sổ (đã xuất) + video chưa xuất biết trước (FULL, các arc nếu chia theo arc, "phần tiếp") + việc xuất trong hàng.
pub fn dung_dong(bo: &Path, ten: &str, ra: &Path, goc_dv: &Path) -> Vec<DongVideo> {
    let so = doc_so(bo, ten, ra);
    let viec = doc_viec_xuat(goc_dv, bo);
    let dd = da_dang_map(bo);
    let mut dong: Vec<DongVideo> = so.into_iter().map(|m| DongVideo { nhan: m.nhan.clone(), ban: m.ban.clone(), nn: so_phan(&m.nhan), tu_den: khoang_chuong(&m.nhan), so: Some(m), ..DongVideo::default() }).collect();
    fn them(dong: &mut Vec<DongVideo>, nhan: String, nn: u32, td: Option<(u32, u32)>) {
        if !dong.iter().any(|d| d.nhan == nhan && d.ban == "viet") {
            dong.push(DongVideo { nhan, ban: "viet".into(), nn, tu_den: td, ..DongVideo::default() });
        }
    }
    them(&mut dong, "FULL".into(), 0, None);
    if chia_cua_bo(bo) == "arc" {
        for a in arcs(bo) {
            them(&mut dong, format!("phan{:02} ch{:03}-{:03}", a.0, a.1, a.2), a.0, Some((a.1, a.2)));
        }
    } else {
        let cao = dong.iter().map(|d| d.nn).max().unwrap_or(0);
        if cao > 0 || chia_cua_bo(bo) != "full" {
            them(&mut dong, format!("phan{:02}", cao + 1), cao + 1, None);
        }
    }
    for d in &mut dong {
        d.viec = viec.iter().rev().find(|v| v.nhan == d.nhan && v.ban == d.ban && v.trang_thai != "huy").cloned().or_else(|| viec.iter().rev().find(|v| d.nn > 0 && v.nhan.starts_with(&format!("phan{:02}", d.nn)) && v.ban == d.ban && v.trang_thai != "huy").cloned());
        d.da_dang = dd.iter().find(|(k, ..)| *k == d.khoa()).map(|(_, n, l)| (n.clone(), l.clone()));
    }
    dong.sort_by(|a, b| (a.ban != "viet", a.nn, &a.nhan).cmp(&(b.ban != "viet", b.nn, &b.nhan)));
    dong
}

fn tim_file(bo: &Path, ten: &str, ra: &Path, file: &str) -> Option<PathBuf> {
    [ra.join(ten).join(file), bo.join("xuat").join(file)].into_iter().find(|p| p.is_file())
}

// ---------------------------------------------------------------- gói đăng

fn la_moc(d: &str) -> bool {
    let t = d.trim_start();
    let so_dau: String = t.chars().take_while(|c| c.is_ascii_digit() || *c == ':').collect();
    so_dau.contains(':') && so_dau.len() >= 4 && t[so_dau.len()..].starts_with([' ', '-', '–']) 
}

/// `video-NN[-en].md` → Gói (3 mục `## Tên`, `## Mô tả`, `## Thumbnail`); tag = hashtag trong mô tả; ảnh = `thumb-video-NN-*.jpg`.
pub fn doc_goi(bo: &Path, nn: u32, ban: &str) -> Goi {
    let d = bo.join("dang-youtube");
    let pre = format!("video-{nn:02}{}", if ban == "en" { "-en" } else { "" });
    let Ok(md) = std::fs::read_to_string(d.join(format!("{pre}.md"))) else { return Goi::default() };
    let mut g = Goi { co_file: true, ..Goi::default() };
    let (mut muc, mut mo_ta) = (String::new(), String::new());
    for dong in md.lines() {
        if let Some(t) = dong.strip_prefix("## ") {
            muc = t.trim().to_lowercase();
            continue;
        }
        match muc.as_str() {
            "tên" | "ten" | "title" => {
                if let Some(t) = dong.trim().strip_prefix("- ") {
                    g.tieu_de.push(t.trim().to_owned());
                }
            }
            "mô tả" | "mo ta" | "description" => {
                mo_ta.push_str(dong);
                mo_ta.push('\n');
            }
            "thumbnail" => {
                if let Some(t) = dong.trim().strip_prefix("- ") {
                    g.thumb_y.push(t.trim().to_owned());
                }
            }
            _ => {}
        }
    }
    g.mo_ta = mo_ta.trim().to_owned();
    g.tag = g.mo_ta.split_whitespace().filter(|w| w.starts_with('#') && w.len() > 1).map(str::to_owned).collect();
    let mut anh: Vec<PathBuf> = std::fs::read_dir(&d).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&format!("thumb-{pre}-"))) && p.extension().is_some_and(|e| e == "jpg" || e == "png")).collect();
    anh.sort();
    g.thumb_anh = anh;
    g
}

/// Mốc chương do MÁY tính từ sổ xuất (đã cộng mở đầu); luật YouTube: mốc đầu 00:00, ≥ 3 mốc, tăng dần, cách ≥ 10 giây.
pub fn tinh_moc(m: &SoMuc, en: bool) -> (Vec<(f64, String)>, Vec<String>) {
    let mut v: Vec<(f64, String)> = vec![];
    if m.moc.first().is_some_and(|c| c.1 > 0.5) {
        v.push((0.0, if en { "Intro".into() } else { "Mở đầu".into() }));
    }
    for (ch, g) in &m.moc {
        let n: String = ch.chars().filter(char::is_ascii_digit).collect();
        v.push((*g, format!("{} {}", if en { "Chapter" } else { "Chương" }, n.trim_start_matches('0'))));
    }
    let mut loi = vec![];
    if v.len() < 3 {
        loi.push(format!("chỉ có {} mốc (YouTube cần ≥ 3)", v.len()));
    }
    if v.first().is_some_and(|x| x.0 > 0.0) {
        loi.push("mốc đầu không phải 00:00".into());
    }
    if v.windows(2).any(|w| w[1].0 - w[0].0 < 10.0) {
        loi.push("có 2 mốc cách nhau < 10 giây".into());
    }
    (v, loi)
}

pub fn moc_text(v: &[(f64, String)]) -> String {
    v.iter().map(|(g, t)| format!("{} {t}", hms(*g))).collect::<Vec<_>>().join("\n")
}

/// Mô tả để dán lên YouTube: thay khối mốc cũ trong mô tả bằng mốc máy tính (không có khối thì chèn sau đoạn đầu).
pub fn mo_ta_day_du(mo_ta: &str, moc: &str) -> String {
    if moc.is_empty() {
        return mo_ta.to_owned();
    }
    let dong: Vec<&str> = mo_ta.lines().collect();
    let dau = dong.iter().position(|d| la_moc(d));
    let mut ra: Vec<String> = vec![];
    match dau {
        Some(i) => {
            ra.extend(dong[..i].iter().map(|x| (*x).to_owned()));
            ra.push(moc.to_owned());
            ra.extend(dong[i..].iter().filter(|d| !la_moc(d)).map(|x| (*x).to_owned()));
        }
        None => {
            let c = dong.iter().position(|d| d.trim().is_empty()).unwrap_or(dong.len());
            ra.extend(dong[..c].iter().map(|x| (*x).to_owned()));
            ra.push(String::new());
            ra.push(moc.to_owned());
            ra.extend(dong[c..].iter().map(|x| (*x).to_owned()));
        }
    }
    ra.join("\n")
}

// ---------------------------------------------------------------- việc

fn them_xuat(goc_dv: &Path, bo: &Path, d: &DongVideo) -> std::io::Result<String> {
    let (co, gt) = if d.nn == 0 { ("--full".to_owned(), "--bitrate-kbps".to_owned()) } else { ("--phan".to_owned(), d.nn.to_string()) };
    lenh::them_viec(goc_dv, "xuat", bo, None, json!({"nhan": d.nhan, "ban": d.ban, "co": co, "gt": gt}))
}

fn prompt_goi(ten: &str, d: &DongVideo, moc: &str) -> String {
    format!(
        "Viết lại gói đăng YouTube cho video \"{}\" của bộ {ten} theo mẫu engine/tien-do/MAU-MO-TA-YOUTUBE.md.\n\
         Ghi dang-youtube/video-{:02}{}.md đúng 3 mục: ## Tên (3 phương án, mỗi dòng bắt đầu \"- \", ≤ 100 ký tự), ## Mô tả (2 dòng đầu có tên bộ + chương X–Y), ## Thumbnail (các dòng \"- trang=… | chu1=… | chu2=…\").\n\
         KHÔNG tự gõ mốc chương: máy đã tính, dán nguyên khối sau vào mô tả.\n{moc}\n",
        d.nhan,
        d.nn,
        if d.ban == "en" { "-en" } else { "" }
    )
}

impl ManXuat {
    pub fn moi(bo: &Path, ten: &str) -> Self {
        Self { bo: bo.to_path_buf(), ten: ten.to_owned(), dong: vec![], chon: None, ngon: "Tất cả".into(), goi: Goi::default(), moc_ngoai: vec![], ra_dir: ra_dir(), link: String::new(), lan_cuoi: None, chon_goi: None }
    }

    fn lam_moi(&mut self, goc_dv: &Path) {
        if self.lan_cuoi.is_some_and(|t| t.elapsed() < Duration::from_millis(1500)) {
            return;
        }
        self.lan_cuoi = Some(Instant::now());
        self.dong = dung_dong(&self.bo, &self.ten, &self.ra_dir, goc_dv);
        if self.chon.is_none() {
            self.chon = self.dong.iter().find(|d| d.so.is_some()).or(self.dong.first()).map(DongVideo::khoa);
        }
        let k = self.chon.clone();
        if self.chon_goi != k {
            self.chon_goi = k.clone();
            if let Some(d) = self.dong.iter().find(|d| Some(d.khoa()) == k) {
                self.goi = doc_goi(&self.bo, d.nn, &d.ban);
                self.link = d.da_dang.as_ref().map(|x| x.1.clone()).unwrap_or_default();
            }
        }
    }
}

fn trang_thai(d: &DongVideo) -> (String, Color32) {
    if let Some(v) = &d.viec {
        match v.trang_thai.as_str() {
            "cho" => return (if v.ly_do.is_empty() { "Đang chờ máy".into() } else { format!("Đang chờ · {}", v.ly_do) }, VANG),
            "cat_quota" => return ("Chờ quota".into(), VANG),
            "chay" => {
                let dang = if v.gio_bat_dau > 0.0 { khoang(bay_gio() - v.gio_bat_dau) } else { String::new() };
                return match v.tien_do {
                    Some(p) if p > 0.01 && v.gio_bat_dau > 0.0 => {
                        let con = (bay_gio() - v.gio_bat_dau) * (1.0 - p) / p;
                        (format!("Đang xuất {:.0}% · còn ~{}", p * 100.0, khoang(con)), XANH_NHAT)
                    }
                    _ => (format!("Đang xuất · {dang}"), XANH_NHAT),
                };
            }
            "loi" => return (format!("Lỗi: {}", v.ly_do), DO),
            "dung" => return ("Đã dừng".into(), VANG),
            _ => {}
        }
    }
    match &d.so {
        Some(m) if m.dat => ("Đã xuất · tự kiểm Đạt".into(), XANH),
        Some(m) => (format!("Đã xuất · tự kiểm Lỗi: {}", m.kiem.join("; ")), DO),
        None => ("Chưa xuất".into(), CHU_MO),
    }
}

fn chip_ban(ban: &str) -> &'static str {
    match ban {
        "en" => "EN",
        "nd" => "Nội dung",
        _ => "Việt",
    }
}

impl App {
    pub fn mo_xuat(&mut self, bo: &Path) {
        let ten = self.snap.bo.iter().find(|b| b.thu_muc == bo).map_or_else(|| bo.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()), |b| b.ten.clone());
        self.xuat = Some(Box::new(ManXuat::moi(bo, &ten)));
        self.man = Man::Xuat(bo.to_path_buf());
    }

    pub fn man_xuat(&mut self, ui: &mut Ui) {
        let Some(mut m) = self.xuat.take() else {
            self.man = Man::Thu;
            return;
        };
        let goc_dv = self.duong.goc_dv.clone();
        m.lam_moi(&goc_dv);
        let chon = m.chon.clone().and_then(|k| m.dong.iter().find(|d| d.khoa() == k).cloned());
        let mut di = false;
        let mut doi_chon: Option<String> = None;
        egui::Panel::top("x_tren").show_separator_line(false).frame(egui::Frame::new().fill(MAT_1).inner_margin(egui::Margin::symmetric(16, 8))).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if nut(ui, "← Bảng điều khiển", false, true).clicked() {
                    di = true;
                }
                ui.label(RichText::new(format!("{} — Xuất & gói đăng", m.ten)).size(15.0).strong());
                ui.separator();
                for t in ["Tất cả", "Việt", "EN", "Nội dung"] {
                    if ui.selectable_label(m.ngon == t, t).clicked() {
                        m.ngon = t.into();
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if nut(ui, "Mở thư mục xuất", false, true).clicked() {
                        lenh::mo_thu_muc(&m.ra_dir.join(&m.ten));
                    }
                });
            });
        });
        egui::Panel::right("x_goi").exact_size(470.0).resizable(false).frame(egui::Frame::new().fill(MAT_1).inner_margin(egui::Margin::same(14))).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("x_goi_cuon").auto_shrink([false, false]).show(ui, |ui| self.cot_goi(ui, &mut m, chon.as_ref()));
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(NEN).inner_margin(egui::Margin::same(14))).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("x_ds").auto_shrink([false, false]).show(ui, |ui| {
                let hien: Vec<DongVideo> = m.dong.iter().filter(|d| m.ngon == "Tất cả" || chip_ban(&d.ban) == m.ngon).cloned().collect();
                if hien.is_empty() {
                    ui.label(RichText::new("Chưa có video nào ở nhóm này.").color(CHU_PHU));
                }
                for d in &hien {
                    if self.dong_video(ui, &m, d, chon.as_ref().is_some_and(|c| c.khoa() == d.khoa())) {
                        doi_chon = Some(d.khoa());
                    }
                }
            });
        });
        if let Some(k) = doi_chon {
            m.chon = Some(k);
            m.lan_cuoi = None;
        }
        if di {
            self.man = Man::Bo(m.bo.clone());
        }
        if matches!(self.man, Man::Xuat(_)) {
            self.xuat = Some(m);
            ui.ctx().request_repaint_after(Duration::from_millis(500));
        }
    }

    /// 1 dòng video: nhãn · thời lượng · trạng thái · nút. Trả true khi được chọn.
    fn dong_video(&mut self, ui: &mut Ui, m: &ManXuat, d: &DongVideo, chon: bool) -> bool {
        let (chu, mau) = trang_thai(d);
        let rong = ui.available_width();
        let (rect, r) = ui.allocate_exact_size(Vec2::new(rong, 86.0), Sense::click());
        let p = ui.painter();
        p.rect_filled(rect, 8.0, if chon { MAT_3 } else { MAT_2 });
        if chon {
            p.rect_stroke(rect, 8.0, egui::Stroke::new(1.0, NHAN), egui::StrokeKind::Inside);
        }
        let x = rect.min.x + 14.0;
        let w = ve_chu(p, egui::pos2(x, rect.min.y + 10.0), &d.ten(), 15.0, CHU, 200.0);
        nhan(p, egui::pos2(x + w + 8.0, rect.min.y + 11.0), chip_ban(&d.ban), CHU_PHU);
        let phu = match (&d.so, d.tu_den) {
            (Some(s), _) => format!("{} · {:.0} MB · {}", hms(s.giay), s.mb, s.luc.get(..16).unwrap_or(&s.luc).replace('T', " ")),
            (None, Some((a, b))) => format!("chương {a}–{b}"),
            _ => "chưa biết khoảng chương".into(),
        };
        ve_chu(p, egui::pos2(x, rect.min.y + 34.0), &phu, 12.0, CHU_PHU, 330.0);
        ve_chu(p, egui::pos2(x, rect.min.y + 56.0), &chu, 12.0, mau, rong - 28.0 - 300.0);
        if let Some(v) = d.viec.as_ref().filter(|v| v.trang_thai == "chay") {
            if let Some(t) = v.tien_do {
                let b = Rect::from_min_size(egui::pos2(rect.min.x + 14.0, rect.max.y - 6.0), Vec2::new((rong - 28.0).max(10.0), 3.0));
                p.rect_filled(b, 1.5, MAT_1);
                p.rect_filled(Rect::from_min_size(b.min, Vec2::new(b.width() * t.clamp(0.0, 1.0) as f32, 3.0)), 1.5, XANH_NHAT);
            }
        }
        let (mut xuat, mut dung, mut lai, mut mo_file, mut mo_dir) = (false, false, false, false, false);
        let dang_cho = d.viec.as_ref().is_some_and(|v| matches!(v.trang_thai.as_str(), "cho" | "chay" | "cat_quota"));
        let da_dang = d.da_dang.is_some();
        let file = d.so.as_ref().and_then(|s| tim_file(&m.bo, &m.ten, &m.ra_dir, &s.file));
        let vung = Rect::from_min_size(egui::pos2(rect.max.x - 296.0, rect.min.y + 26.0), Vec2::new(284.0, 34.0));
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(vung).layout(egui::Layout::right_to_left(egui::Align::Center)));
        (|ui: &mut Ui| {
            if dang_cho {
                dung = nut(ui, "Dừng", false, d.viec.as_ref().is_some_and(|v| v.trang_thai == "chay")).on_hover_text("Chỉ nút này mới dừng được một lần xuất đang chạy").clicked();
            } else if d.viec.as_ref().is_some_and(|v| matches!(v.trang_thai.as_str(), "loi" | "dung")) {
                lai = nut(ui, "Thử lại", true, true).clicked();
            } else {
                let on = d.ban == "viet" && !da_dang;
                let t = if d.so.is_some() { "Xuất lại" } else { "Xuất" };
                xuat = nut(ui, t, d.so.is_none(), on).on_disabled_hover_text(if da_dang { "Video đã đăng — máy không xuất lại" } else { "Bản EN/Nội dung chưa có lệnh xuất trong lõi" }).clicked();
            }
            mo_dir = nut(ui, "Thư mục", false, file.is_some()).clicked();
            mo_file = nut(ui, "Mở file", false, file.is_some()).clicked();
        })(&mut c);
        if xuat {
            let r = if d.viec.as_ref().is_some_and(|v| matches!(v.trang_thai.as_str(), "cho" | "chay" | "cat_quota")) { Ok(String::new()) } else { them_xuat(&self.duong.goc_dv, &m.bo, d) };
            self.gui(r.map(|_| ()), &format!("Đã thêm việc xuất {} vào hàng việc", d.ten()));
        }
        if lai {
            if let Some(v) = &d.viec {
                let r = lenh::gui_lenh(&self.duong.goc_dv, "thu_lai", &v.id);
                self.gui(r, "Đã gửi lệnh thử lại");
            }
        }
        if dung {
            if let Some(v) = &d.viec {
                let r = lenh::gui_lenh(&self.duong.goc_dv, "dung", &v.id);
                self.gui(r, "Đã gửi lệnh dừng — đang dừng sạch");
            }
        }
        if let (true, Some(p)) = (mo_file, &file) {
            lenh::mo_thu_muc(p);
        }
        if let (true, Some(p)) = (mo_dir, file.as_ref().and_then(|p| p.parent())) {
            lenh::mo_thu_muc(p);
        }
        r.clicked()
    }

    fn cot_goi(&mut self, ui: &mut Ui, m: &mut ManXuat, chon: Option<&DongVideo>) {
        let Some(d) = chon else {
            ui.label(RichText::new("Chọn 1 video bên trái để xem gói đăng.").color(CHU_PHU));
            return;
        };
        let en = d.ban == "en";
        ui.label(RichText::new(format!("Gói đăng — {}", d.ten())).size(16.0).strong());
        let (moc_v, moc_loi) = d.so.as_ref().map_or((vec![], vec!["chưa xuất nên chưa có mốc".to_owned()]), |s| tinh_moc(s, en));
        let moc_s = moc_text(&moc_v);
        let g = m.goi.clone();
        let du = [!g.tieu_de.is_empty(), !g.mo_ta.is_empty() && moc_loi.is_empty(), !g.tag.is_empty(), !g.thumb_y.is_empty() || !g.thumb_anh.is_empty()];
        let so_du = du.iter().filter(|x| **x).count();
        ui.label(RichText::new(format!("{so_du}/4 phần đủ: tiêu đề · mô tả + mốc · tag · thumbnail")).size(12.0).color(if so_du == 4 { XANH } else { VANG }));
        ui.add_space(6.0);

        if let Some(s) = &d.so {
            muc(ui, "Sổ xuất");
            ui.label(RichText::new(format!("{} · {} · {:.0} MB ({:.1} MB/phút)", s.file, hms(s.giay), s.mb, if s.giay > 0.0 { s.mb / (s.giay / 60.0) } else { 0.0 })).size(12.0).color(CHU_PHU));
            ui.label(RichText::new(if s.dat { "Tự kiểm: Đạt".to_owned() } else { format!("Tự kiểm: Lỗi — {}", s.kiem.join("; ")) }).size(12.0).color(if s.dat { XANH } else { DO }));
            if s.mo_dau > 0.0 {
                ui.label(RichText::new(format!("Mở đầu {} + nội dung {} = {} (mốc chương đã cộng mở đầu)", hms(s.mo_dau), hms(s.giay - s.mo_dau), hms(s.giay))).size(12.0).color(CHU_PHU));
            }
            if s.xuat_giay > 0.0 {
                ui.label(RichText::new(format!("Xuất mất {}", khoang(s.xuat_giay))).size(12.0).color(CHU_MO));
            }
        }

        muc(ui, "Tiêu đề");
        if g.tieu_de.is_empty() {
            thieu(ui, if g.co_file { "File gói chưa có mục ## Tên" } else { "Chưa có gói đăng cho video này (dang-youtube/video-NN.md)" });
        }
        for t in &g.tieu_de {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} ký tự", t.chars().count())).size(11.0).color(if t.chars().count() > 100 { DO } else { CHU_MO }));
                if nut(ui, "Chép tiêu đề", false, true).clicked() {
                    ui.ctx().copy_text(t.clone());
                    self.tb("Đã chép tiêu đề — dán vào YouTube");
                }
            });
            ui.label(t.as_str());
            ui.add_space(4.0);
        }

        muc(ui, "Mốc chương (máy tính từ timeline)");
        if moc_loi.is_empty() {
            ui.label(RichText::new(format!("{} mốc · hợp lệ cho YouTube (00:00, tăng dần, cách ≥ 10 giây)", moc_v.len())).size(12.0).color(XANH));
        } else {
            thieu(ui, &moc_loi.join("; "));
        }
        if !moc_v.is_empty() {
            let t = moc_s.clone();
            egui::ScrollArea::vertical().id_salt("x_moc").max_height(120.0).show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut t.as_str()).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
            });
        }

        muc(ui, "Mô tả");
        if g.mo_ta.is_empty() {
            thieu(ui, "Chưa có mục ## Mô tả");
        } else {
            let day_du = mo_ta_day_du(&g.mo_ta, &moc_s);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} ký tự", day_du.chars().count())).size(11.0).color(if day_du.chars().count() > 5000 { DO } else { CHU_MO }));
                if nut(ui, "Chép mô tả", false, true).on_hover_text("Mô tả đã thay khối mốc bằng mốc máy tính").clicked() {
                    ui.ctx().copy_text(day_du.clone());
                    self.tb("Đã chép mô tả (đã có mốc chương) — dán vào YouTube");
                }
            });
            egui::ScrollArea::vertical().id_salt("x_mota").max_height(170.0).show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut day_du.as_str()).desired_width(f32::INFINITY).desired_rows(6));
            });
        }

        muc(ui, "Tag");
        if g.tag.is_empty() {
            thieu(ui, "Mô tả chưa có hashtag");
        } else {
            ui.label(RichText::new(g.tag.join("  ")).size(12.0).color(CHU_PHU));
        }

        muc(ui, "Thumbnail");
        if g.thumb_anh.is_empty() && g.thumb_y.is_empty() {
            thieu(ui, "Chưa có ý thumbnail");
        }
        for p in g.thumb_anh.iter().take(3) {
            if let Some(t) = self.lay_bia(p) {
                ui.add(egui::Image::new((t, Vec2::new(160.0, 90.0))));
            }
        }
        for y in &g.thumb_y {
            ui.label(RichText::new(y.as_str()).size(11.0).color(CHU_PHU));
        }

        ui.add_space(10.0);
        let r = nut(ui, "Viết lại gói bằng agy", false, true).on_hover_text("Chỉ tạo việc agy kiểu `goi` trong hàng việc; agy chạy theo quota ở Màn 7").clicked();
        if r {
            let pr = prompt_goi(&m.ten, d, &moc_s);
            let res = lenh::them_viec(&self.duong.goc_dv, "agy", &m.bo, None, json!({"kieu": "goi", "prompt": pr, "ban": if en { "en" } else { "vi" }}));
            self.gui(res.map(|_| ()), "Đã tạo việc viết lại gói (agy) trong hàng việc");
        }

        muc(ui, "Đã đăng");
        if let Some((ngay, link)) = &d.da_dang {
            ui.label(RichText::new(format!("Đã đăng {ngay}{}", if link.is_empty() { String::new() } else { format!(" · {link}") })).color(XANH));
            if nut(ui, "Bỏ đánh dấu", false, true).clicked() {
                let k = d.khoa();
                let res = lenh::sua_bo_json(&m.bo, |v| {
                    if let Some(o) = v.get_mut("da_dang_video").and_then(Value::as_object_mut) {
                        o.remove(&k);
                    }
                });
                self.gui(res, "Đã bỏ đánh dấu Đã đăng");
            }
        } else {
            ui.add(egui::TextEdit::singleline(&mut m.link).hint_text("Link video YouTube (có thể để trống)").desired_width(f32::INFINITY));
            if nut(ui, "Đánh dấu Đã đăng", true, d.so.is_some()).on_disabled_hover_text("Cần xuất xong trước").clicked() {
                let (k, ngay, link) = (d.khoa(), chrono::Local::now().format("%d/%m/%Y").to_string(), m.link.trim().to_owned());
                let res = lenh::sua_bo_json(&m.bo, |v| {
                    if !v.get("da_dang_video").is_some_and(Value::is_object) {
                        v["da_dang_video"] = json!({});
                    }
                    v["da_dang_video"][&k] = json!({"ngay": ngay, "link": link});
                });
                self.gui(res, "Đã ghi Đã đăng vào bo.json");
            }
        }
    }
}

fn muc(ui: &mut Ui, t: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(t).size(13.0).strong().color(NHAN));
}

fn thieu(ui: &mut Ui, t: &str) {
    ui.label(RichText::new(format!("Thiếu: {t}")).size(12.0).color(DO));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chạy tay với dịch vụ thử: `RECAP_DV_GOC=… RECAP_TEST_BO=… RECAP_TEST_NN=0|N cargo test -p recap-gui -- --ignored them_xuat_that`.
    #[test]
    #[ignore]
    fn them_xuat_that() {
        let nn: u32 = std::env::var("RECAP_TEST_NN").unwrap_or_default().parse().unwrap_or(0);
        let d = DongVideo { nhan: if nn == 0 { "FULL".into() } else { format!("phan{nn:02}") }, ban: "viet".into(), nn, ..DongVideo::default() };
        let (goc, bo) = (PathBuf::from(std::env::var("RECAP_DV_GOC").unwrap()), PathBuf::from(std::env::var("RECAP_TEST_BO").unwrap()));
        them_xuat(&goc, &bo, &d).unwrap();
    }

    #[test]
    fn doc_so_that_va_moc_hop_le() {
        let p = nha().join("recap-thu-moc2/Nisekoi/so-xuat.json");
        let Some(m) = doc_so_file(&p).into_iter().next() else { return };
        assert!(m.dat);
        let (v, loi) = tinh_moc(&m, false);
        assert!(loi.is_empty(), "{loi:?}");
        assert_eq!(v[0].0, 0.0);
        assert!(v.len() >= 26);
    }

    #[test]
    fn mo_ta_thay_khoi_moc() {
        let t = "Dòng 1\n\n00:00 A\n01:00 B\n\nCuối";
        let r = mo_ta_day_du(t, "00:00 X\n00:20 Y\n01:00 Z");
        assert!(r.contains("00:20 Y") && !r.contains("01:00 B") && r.contains("Cuối"));
        assert!(!mo_ta_day_du("Dòng 1\n\nCuối", "00:00 X").is_empty());
    }
}
