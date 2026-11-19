//! Đọc dữ liệu thật, CHỈ ĐỌC: thư mục bộ (`bo.json` + đếm file, spec `00` mục C) và file trạng thái của
//! dịch vụ nền (`engine/dich-vu-nen/DINH-DANG.md`). Không có hàm ghi nào ở đây (ghi: `lenh.rs`).
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SO_BUOC: usize = 14;
pub const TEN_NGAN: [&str; SO_BUOC] = ["Nhập", "OCR", "Xoá chữ", "Dò khung", "Chia arc", "Kịch bản", "Tên riêng", "Kiểm KB", "Giọng", "Timeline", "Dịch Anh", "Mở đầu", "Xuất", "Gói đăng"];
pub const TEN_DAI: [&str; SO_BUOC] = [
    "Nhập bộ", "Đọc chữ (OCR)", "Xoá chữ", "Dò khung", "Chia arc + chất giọng", "Viết kịch bản", "Tên riêng + phát âm", "Kiểm kịch bản", "Giọng", "Lên timeline", "Dịch bản Anh", "Mở đầu", "Xuất", "Gói đăng",
];
/// Đơn vị đếm của từng bước (chương / arc / file…).
pub const DON_VI: [&str; SO_BUOC] = ["chương", "chương", "chương", "chương", "mục", "chương", "arc", "chương", "chương", "phần", "chương", "mục", "video", "file"];
/// Loại việc của dịch vụ chạy được từng bước; `None` = chưa có loại việc máy cho bước này (bước kiểm = máy đo, không chạy).
pub const LOAI_BUOC: [Option<&str>; SO_BUOC] =
    [Some("tai"), Some("ocr"), Some("xoa_chu"), Some("do_khung"), None, Some("agy"), Some("agy"), None, Some("giong"), Some("dung_timeline"), Some("agy"), Some("agy"), Some("xuat"), Some("agy")];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tt {
    Chua,
    Chay,
    Xong,
    Loi,
    /// chờ Thịnh / chờ quota / đã dừng
    Cho,
    BoQua,
}

#[derive(Clone, Debug)]
pub struct Dong {
    pub ten: String,
    pub xong: bool,
    pub tt: Tt,
    pub ly_do: String,
    pub viec: Option<String>,
    pub gio: f64,
}
impl Dong {
    fn moi(ten: String, xong: bool) -> Self {
        Self { ten, xong, tt: if xong { Tt::Xong } else { Tt::Chua }, ly_do: String::new(), viec: None, gio: 0.0 }
    }
}

#[derive(Clone, Debug)]
pub struct Buoc {
    pub dong: Vec<Dong>,
    pub xong: usize,
    pub tong: usize,
    pub tt: Tt,
    pub cau: String,
    /// id các việc đang chạy của bước (để nút Dừng)
    pub dang_chay: Vec<String>,
    /// id các việc lỗi của bước (để nút Chạy lại chương lỗi)
    pub viec_loi: Vec<String>,
}
impl Buoc {
    fn tu_dong(dong: Vec<Dong>) -> Self {
        let xong = dong.iter().filter(|d| d.xong).count();
        let tt = if dong.is_empty() { Tt::Chua } else if xong == dong.len() { Tt::Xong } else { Tt::Chua };
        Self { tong: dong.len(), xong, dong, tt, cau: String::new(), dang_chay: vec![], viec_loi: vec![] }
    }
    fn bo_qua() -> Self {
        Self { dong: vec![], xong: 0, tong: 0, tt: Tt::BoQua, cau: "Bỏ qua — bộ này không nối tiếp anime".into(), dang_chay: vec![], viec_loi: vec![] }
    }
}

#[derive(Clone, Debug)]
pub struct Bo {
    pub thu_muc: PathBuf,
    pub ten: String,
    pub loai: String,
    pub may: String,
    pub da_dang: bool,
    pub tam_dung: bool,
    pub tu_dong_ca_bo: bool,
    pub tu_dong_buoc: [bool; SO_BUOC],
    pub bia: Option<PathBuf>,
    pub noi_tiep_anime: bool,
    pub buoc: Vec<Buoc>,
    /// epoch giây: lần cuối có gì thay đổi (đo từ mtime thư mục + việc của dịch vụ)
    pub cap_nhat: f64,
    pub tt: TtBo,
    pub cau: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum TtBo {
    Ket,
    DangLam,
    GanXuat,
    XongChuaDang,
    DaDang,
    Roi,
}

pub fn bay_gio() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

fn mtime(p: &Path) -> f64 {
    std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0.0, |d| d.as_secs_f64())
}

pub fn doc_json(p: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

fn ten_file(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Tên mục con (file hoặc thư mục) không bắt đầu bằng `.` hoặc `_`, sắp xếp.
fn muc(dir: &Path, chi_thu_muc: bool) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let t = ten_file(p);
            !t.starts_with('.') && !t.starts_with('_') && p.is_dir() == chi_thu_muc
        })
        .collect();
    v.sort();
    v
}

fn la_anh(p: &Path) -> bool {
    matches!(p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref(), Some("jpg" | "jpeg" | "png" | "webp"))
}

fn dem_anh(dir: &Path) -> usize {
    muc(dir, false).iter().filter(|p| la_anh(p)).count()
}

/// Tên (không đuôi) của các file `.json` trong `dir` (ocr/, che-chu/, khung/).
fn cac_json(dir: &Path) -> BTreeSet<String> {
    muc(dir, false).iter().filter_map(|p| ten_file(p).strip_suffix(".json").map(str::to_owned)).collect()
}

struct KichBan {
    md: BTreeMap<String, PathBuf>,
    doi_chieu: BTreeSet<String>,
    en: BTreeSet<String>,
    arc: Vec<(String, bool, bool)>, // tên arc, có md, có _ten-rieng.json
    chat_giong: bool,
}

fn quet_kich_ban(bo: &Path) -> KichBan {
    let dir = bo.join("kich-ban");
    let mut kb = KichBan { md: BTreeMap::new(), doi_chieu: BTreeSet::new(), en: BTreeSet::new(), arc: vec![], chat_giong: !muc(&dir.join("_chuan"), false).is_empty() };
    for a in muc(&dir, true).into_iter().filter(|p| ten_file(p).starts_with("arc")) {
        let mut co_md = false;
        for f in muc(&a, false) {
            let t = ten_file(&f);
            if let Some(ch) = t.strip_suffix(".doi-chieu.md") {
                kb.doi_chieu.insert(ch.to_owned());
            } else if let Some(ch) = t.strip_suffix(".md") {
                co_md = true;
                kb.md.insert(ch.to_owned(), f);
            }
        }
        for f in muc(&a.join("en"), false) {
            if let Some(ch) = ten_file(&f).strip_suffix(".md") {
                kb.en.insert(ch.to_owned());
            }
        }
        kb.arc.push((ten_file(&a), co_md, a.join("_ten-rieng.json").is_file()));
    }
    kb
}

/// Số đoạn lời kể (cách đếm của `kiem.py`: phần sau `## SCRIPT`, tách bằng dòng trống).
pub fn dem_doan(raw: &str) -> usize {
    raw.rsplit("## SCRIPT").next().unwrap_or("").trim().split("\n\n").filter(|s| !s.trim().is_empty()).count()
}

fn dong_chuong(chuong: &[String], xong: impl Fn(&str) -> bool) -> Vec<Dong> {
    chuong.iter().map(|c| Dong::moi(c.clone(), xong(c))).collect()
}

fn buoc_kiem(chuong: &[String], so_anh: &BTreeMap<String, usize>, kb: &KichBan) -> Buoc {
    let dong = chuong
        .iter()
        .map(|c| {
            let Some(p) = kb.md.get(c) else { return Dong::moi(c.clone(), false) };
            let d = dem_doan(&std::fs::read_to_string(p).unwrap_or_default());
            let a = so_anh.get(c).copied().unwrap_or(0);
            if a > 0 && d != a {
                let mut x = Dong::moi(c.clone(), false);
                x.tt = Tt::Loi;
                x.ly_do = format!("{d} đoạn cho {a} trang → bấm Viết lại chương (agy viết lại đúng chương này, giữ bản cũ)");
                x
            } else {
                Dong::moi(c.clone(), true)
            }
        })
        .collect();
    Buoc::tu_dong(dong)
}

fn ten_giong(bj: &Value) -> Option<String> {
    let g = bj.get("giong")?.as_array()?.first()?;
    g.as_str().map(str::to_owned).or_else(|| g.get("ten").and_then(Value::as_str).map(str::to_owned))
}

fn buoc_giong(bo: &Path, bj: &Value, chuong: &[String]) -> Buoc {
    let goc = bo.join("giong");
    let thu_muc = ten_giong(bj).map(|g| goc.join(g)).filter(|p| p.is_dir()).or_else(|| muc(&goc, true).into_iter().next());
    let co_mp3 = |c: &str| thu_muc.as_ref().is_some_and(|g| muc(&g.join(c), false).iter().any(|f| f.extension().is_some_and(|e| e == "mp3")));
    Buoc::tu_dong(dong_chuong(chuong, co_mp3))
}

fn dong_file(dir: &Path, hop: impl Fn(&Path) -> bool) -> Vec<Dong> {
    muc(dir, false).into_iter().filter(|p| hop(p)).map(|p| Dong::moi(ten_file(&p), true)).collect()
}

fn buoc_xuat(bo: &Path, bj: &Value) -> Buoc {
    let mp4 = |p: &Path| p.extension().is_some_and(|e| e == "mp4");
    let mut dong = dong_file(&bo.join("xuat"), mp4);
    // bộ vừa chuyển từ hệ cũ: video đã xuất còn nằm ở thư mục cũ (chỉ ghi đường dẫn trong bo.json) — đếm chỉ-đọc
    if let Some(cu) = bj.pointer("/chuyen_tu_cu/xuat_cu").and_then(Value::as_str) {
        let da: BTreeSet<String> = dong.iter().map(|d| d.ten.clone()).collect();
        dong.extend(dong_file(Path::new(cu), mp4).into_iter().filter(|d| !da.contains(&d.ten)));
    }
    Buoc::tu_dong(dong)
}

fn buoc_mo_dau(bo: &Path) -> Buoc {
    let co = ["gioi-thieu", "mo-dau"].iter().any(|t| !muc(&bo.join("kich-ban").join(t), false).is_empty());
    Buoc::tu_dong(vec![Dong::moi("Đoạn mở đầu".into(), co)])
}

fn buoc_arc(kb: &KichBan) -> Buoc {
    let mut dong: Vec<Dong> = kb.arc.iter().map(|(t, co_md, _)| Dong::moi(t.clone(), *co_md)).collect();
    dong.push(Dong::moi("Chất giọng kể của bộ".into(), kb.chat_giong));
    Buoc::tu_dong(dong)
}

fn lay_chuong(bo: &Path, kb: &KichBan) -> (Vec<String>, BTreeMap<String, usize>) {
    let mut so_anh = BTreeMap::new();
    for d in muc(&bo.join("nguon"), true) {
        so_anh.insert(ten_file(&d), dem_anh(&d));
    }
    let mut ch: BTreeSet<String> = so_anh.keys().cloned().collect();
    ch.extend(cac_json(&bo.join("khung")));
    ch.extend(kb.md.keys().cloned());
    (ch.into_iter().filter(|c| c.starts_with("ch")).collect(), so_anh)
}

fn lay_buoc(bo: &Path, bj: &Value, noi_tiep: bool) -> Vec<Buoc> {
    let kb = quet_kich_ban(bo);
    let (ch, so_anh) = lay_chuong(bo, &kb);
    let (ocr, che, khung) = (cac_json(&bo.join("ocr")), cac_json(&bo.join("che-chu")), cac_json(&bo.join("khung")));
    let nhap = Buoc::tu_dong(dong_chuong(&ch, |c| so_anh.get(c).copied().unwrap_or(0) > 0));
    let ten_rieng = Buoc::tu_dong(kb.arc.iter().map(|(t, _, co)| Dong::moi(t.clone(), *co)).collect());
    let tl = Buoc::tu_dong(dong_file(&bo.join("timeline"), |_| true));
    let mo_dau = if noi_tiep { buoc_mo_dau(bo) } else { Buoc::bo_qua() };
    vec![
        nhap,
        Buoc::tu_dong(dong_chuong(&ch, |c| ocr.contains(c))),
        Buoc::tu_dong(dong_chuong(&ch, |c| che.contains(c))),
        Buoc::tu_dong(dong_chuong(&ch, |c| khung.contains(c))),
        buoc_arc(&kb),
        Buoc::tu_dong(dong_chuong(&ch, |c| kb.md.contains_key(c) && kb.doi_chieu.contains(c))),
        ten_rieng,
        buoc_kiem(&ch, &so_anh, &kb),
        buoc_giong(bo, bj, &ch),
        tl,
        Buoc::tu_dong(dong_chuong(&ch, |c| kb.en.contains(c))),
        mo_dau,
        buoc_xuat(bo, bj),
        Buoc::tu_dong(dong_file(&bo.join("dang-youtube"), |p| p.extension().is_some_and(|e| e == "md" || e == "json"))),
    ]
}

fn cap_nhat_file(bo: &Path) -> f64 {
    ["bo.json", "nguon", "khung", "kich-ban", "giong", "timeline", "xuat", "dang-youtube", "ocr", "che-chu"].iter().map(|t| mtime(&bo.join(t))).fold(0.0, f64::max)
}

/// Quét 1 thư mục bộ. `None` = không phải thư mục bộ (không có `bo.json` đọc được).
pub fn quet_bo(thu_muc: &Path) -> Option<Bo> {
    let bj = doc_json(&thu_muc.join("bo.json"))?;
    let chu = |k: &str| bj.get(k).and_then(Value::as_str).map(str::to_owned);
    let co = |k: &str| bj.get(k).and_then(Value::as_bool).unwrap_or(false);
    let noi_tiep_anime = co("noi_tiep_anime");
    let mut tu_dong_buoc = [false; SO_BUOC];
    for (i, t) in tu_dong_buoc.iter_mut().enumerate() {
        *t = bj.pointer(&format!("/tu_dong/buoc/{i}")).and_then(Value::as_bool).unwrap_or(false);
    }
    let bia = ["bia.jpg", "bia.jpeg", "bia.png"].iter().map(|t| thu_muc.join(t)).find(|p| p.is_file());
    Some(Bo {
        ten: chu("ten").unwrap_or_else(|| ten_file(thu_muc)),
        loai: chu("loai").unwrap_or_else(|| "manga".into()),
        may: chu("may").unwrap_or_default(),
        da_dang: co("da_dang"),
        tam_dung: co("tam_dung"),
        tu_dong_ca_bo: bj.pointer("/tu_dong/ca_bo").and_then(Value::as_bool).unwrap_or(true),
        tu_dong_buoc,
        bia,
        noi_tiep_anime,
        buoc: lay_buoc(thu_muc, &bj, noi_tiep_anime),
        cap_nhat: cap_nhat_file(thu_muc),
        thu_muc: thu_muc.to_path_buf(),
        tt: TtBo::DangLam,
        cau: String::new(),
    })
}

/// Mọi bộ trong các thư mục gốc (thư mục con có `bo.json`; bỏ thư mục `.`/`_`).
pub fn quet_goc(goc: &[PathBuf]) -> Vec<Bo> {
    goc.iter().flat_map(|g| muc(g, true)).filter_map(|d| quet_bo(&d)).collect()
}

// ---------------------------------------------------------------- dịch vụ nền

#[derive(Clone, Debug, Default)]
pub struct Viec {
    pub id: String,
    pub loai: String,
    pub kieu: String,
    pub bo: String,
    pub chuong: Option<String>,
    pub trang_thai: String,
    pub lan_thu: u32,
    pub ly_do: String,
    pub gio_tao: f64,
    pub gio_bat_dau: f64,
    pub cho_toi: f64,
    pub cho_tk: Option<String>,
    pub tien_do: Option<f64>,
}

fn so(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(0.0)
}
fn chuoi(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn doc_viec(v: &Value) -> Viec {
    Viec {
        id: chuoi(v, "id"),
        loai: chuoi(v, "loai"),
        kieu: v.pointer("/tham_so/kieu").and_then(Value::as_str).unwrap_or("").to_owned(),
        bo: chuoi(v, "bo"),
        chuong: v.get("chuong").and_then(Value::as_str).map(str::to_owned),
        trang_thai: chuoi(v, "trang_thai"),
        lan_thu: so(v, "lan_thu") as u32,
        ly_do: chuoi(v, "ly_do"),
        gio_tao: so(v, "gio_tao"),
        gio_bat_dau: so(v, "gio_bat_dau"),
        cho_toi: so(v, "cho_toi"),
        cho_tk: v.get("cho_tk").and_then(Value::as_str).map(str::to_owned),
        tien_do: v.get("tien_do").and_then(Value::as_f64),
    }
}

pub fn doc_hang_viec(goc: &Path) -> Vec<Viec> {
    let mut v: Vec<Viec> = std::fs::read_dir(goc.join("hang-viec"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let t = ten_file(p);
            t.ends_with(".json") && !t.ends_with(".ket-thuc.json") && !t.starts_with('.')
        })
        .filter_map(|p| doc_json(&p))
        .map(|j| doc_viec(&j))
        .collect();
    v.sort_by(|a, b| a.id.cmp(&b.id));
    v
}

pub fn buoc_cua_viec(v: &Viec) -> Option<usize> {
    Some(match (v.loai.as_str(), v.kieu.as_str()) {
        ("tai", _) => 0,
        ("ocr", _) => 1,
        ("xoa_chu", _) => 2,
        ("do_khung", _) => 3,
        ("agy", "ten_rieng") => 6,
        ("agy", "dich") => 10,
        ("agy", "mo_dau") => 11,
        ("agy", "goi") => 13,
        ("agy", "viet" | "viet_lai") => 5,
        ("giong", _) => 8,
        ("dung_timeline", _) => 9,
        ("xuat", _) => 12,
        _ => return None,
    })
}

#[derive(Clone, Debug, Default)]
pub struct May {
    pub ten: String,
    pub truc_tuyen: bool,
    pub nhan_luc: f64,
    pub mat_tu: Option<f64>,
    pub phien_ban: String,
}

#[derive(Clone, Debug, Default)]
pub struct CanhBaoDv {
    pub khoa: String,
    pub noi_dung: String,
}

#[derive(Clone, Debug, Default)]
pub struct DichVu {
    pub song: bool,
    pub may: String,
    pub phien_ban: String,
    pub gio: f64,
    pub he_dieu_hanh: String,
    pub ap_luc: String,
    pub ram_trong_gb: Option<f64>,
    pub o_trong_gb: Option<f64>,
    pub hang: BTreeMap<String, u32>,
    pub luong: Vec<(String, u32, u32, u32)>, // loại, tối đa, hiệu lực, đang chạy
    pub agy_tai_khoan: String,
    pub agy_dang_chay: u32,
    pub agy_cat_quota: u32,
    pub sao_luu: Option<f64>,
    pub canh_bao: Vec<CanhBaoDv>,
    pub may_khac: Vec<May>,
}

pub fn doc_dich_vu(goc: &Path, bay_gio: f64) -> DichVu {
    let Some(t) = doc_json(&goc.join("trang-thai.json")) else { return DichVu::default() };
    let tn = t.get("tai_nguyen").cloned().unwrap_or(Value::Null);
    let mut dv = DichVu {
        song: bay_gio - so(&t, "gio") <= 15.0,
        may: chuoi(&t, "may"),
        phien_ban: chuoi(&t, "phien_ban"),
        gio: so(&t, "gio"),
        he_dieu_hanh: chuoi(&t, "he_dieu_hanh"),
        ap_luc: chuoi(&tn, "ap_luc"),
        ram_trong_gb: tn.get("ram_trong_gb").and_then(Value::as_f64),
        o_trong_gb: tn.get("o_trong_gb").and_then(Value::as_f64),
        agy_tai_khoan: t.pointer("/agy/tai_khoan").and_then(Value::as_str).unwrap_or("").to_owned(),
        agy_dang_chay: t.pointer("/agy/dang_chay").and_then(Value::as_u64).unwrap_or(0) as u32,
        agy_cat_quota: t.pointer("/agy/cat_cho_quota").and_then(Value::as_u64).unwrap_or(0) as u32,
        sao_luu: t.pointer("/sao_luu/lan_cuoi").and_then(Value::as_f64),
        ..DichVu::default()
    };
    if let Some(h) = t.get("hang").and_then(Value::as_object) {
        dv.hang = h.iter().map(|(k, v)| (k.clone(), v.as_u64().unwrap_or(0) as u32)).collect();
    }
    if let Some(l) = t.get("luong").and_then(Value::as_object) {
        let n = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
        dv.luong = l.iter().map(|(k, v)| (k.clone(), n(v, "toi_da"), n(v, "hieu_luc"), n(v, "dang_chay"))).collect();
    }
    for c in t.get("canh_bao").and_then(Value::as_array).into_iter().flatten() {
        dv.canh_bao.push(CanhBaoDv { khoa: chuoi(c, "khoa"), noi_dung: chuoi(c, "noi_dung") });
    }
    dv.may_khac = doc_may_khac(goc, bay_gio);
    dv
}

/// Máy khác: tự tính lại từ `nhan_luc` (> 90 giây không hỏi được = mất kết nối), không tin cờ cũ.
fn doc_may_khac(goc: &Path, bay_gio: f64) -> Vec<May> {
    let mut v: Vec<May> = muc(&goc.join("may-khac"), false)
        .iter()
        .filter_map(|p| doc_json(p))
        .map(|j| {
            let nhan_luc = so(&j, "nhan_luc");
            let on = bay_gio - nhan_luc <= 90.0 && j.get("truc_tuyen").and_then(Value::as_bool).unwrap_or(true);
            let mat_tu = j.get("mat_ket_noi_tu").and_then(Value::as_f64).or(if on { None } else { Some(nhan_luc) });
            May { ten: chuoi(&j, "ten"), truc_tuyen: on, nhan_luc, mat_tu, phien_ban: chuoi(&j, "phien_ban") }
        })
        .collect();
    v.sort_by(|a, b| a.ten.cmp(&b.ten));
    v
}

/// ≤ `n` dòng cuối của `nhat-ky.log` (chỉ đọc đoạn cuối file).
pub fn doc_nhat_ky(goc: &Path, n: usize) -> Vec<String> {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(goc.join("nhat-ky.log")) else { return vec![] };
    let dai = f.metadata().map_or(0, |m| m.len());
    let _ = f.seek(SeekFrom::Start(dai.saturating_sub(256 * 1024)));
    let mut s = Vec::new();
    let _ = f.read_to_end(&mut s);
    let s = String::from_utf8_lossy(&s);
    let dong: Vec<&str> = s.lines().collect();
    dong[dong.len().saturating_sub(n)..].iter().map(|x| (*x).to_owned()).collect()
}

// ---------------------------------------------------------------- phủ trạng thái việc lên bộ

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Muc {
    Do,
    Vang,
}

#[derive(Clone, Debug)]
pub struct CanhBao {
    pub muc: Muc,
    /// thư mục bộ + bước để bấm vào đúng chỗ; `None` = cảnh báo của máy (ổ đầy, mất kết nối…)
    pub bo: Option<(PathBuf, usize)>,
    pub noi_dung: String,
}

pub fn gio_phut(epoch: f64) -> String {
    use chrono::TimeZone;
    chrono::Local.timestamp_opt(epoch as i64, 0).single().map_or_else(String::new, |t| t.format("%H:%M").to_string())
}

fn chay_ten(ch: &[&str]) -> String {
    match ch {
        [] => String::new(),
        [a] => (*a).to_owned(),
        [a, .., z] => format!("{a}–{z}"),
    }
}

fn phu_dong(b: &mut Buoc, v: &Viec) {
    let Some(ch) = v.chuong.as_deref() else { return };
    let Some(d) = b.dong.iter_mut().find(|d| d.ten == ch) else { return };
    let (tt, ly) = match v.trang_thai.as_str() {
        "chay" => (Tt::Chay, format!("đang chạy từ {}", gio_phut(v.gio_bat_dau))),
        "loi" => (Tt::Loi, v.ly_do.clone()),
        "cat_quota" => (Tt::Cho, format!("chờ quota tài khoản {} tới {}", v.cho_tk.as_deref().unwrap_or("?"), gio_phut(v.cho_toi))),
        "dung" => (Tt::Cho, "đã dừng — bấm Thử lại để chạy tiếp".into()),
        "cho" if !v.ly_do.is_empty() => (Tt::Cho, v.ly_do.clone()),
        "cho" => (if d.xong { Tt::Xong } else { Tt::Chua }, "đang xếp hàng".into()),
        _ => return,
    };
    if d.tt != Tt::Loi || tt == Tt::Loi {
        d.tt = tt;
        d.ly_do = ly;
    }
    d.viec = Some(v.id.clone());
    d.gio = v.gio_bat_dau.max(v.gio_tao);
}

fn cau_buoc(i: usize, b: &Buoc, bat_dau: f64) -> String {
    let ten = TEN_DAI[i].to_lowercase();
    match b.tt {
        Tt::Chay => {
            let ch: Vec<&str> = b.dong.iter().filter(|d| d.tt == Tt::Chay).map(|d| d.ten.as_str()).collect();
            let ch = chay_ten(&ch);
            format!("Đang {ten} {ch} · {} việc · bắt đầu {}", b.dang_chay.len(), gio_phut(bat_dau)).replace("  ", " ")
        }
        Tt::Loi => format!("{} chỗ lỗi — lý do và việc nên làm ở bảng bên dưới", b.dong.iter().filter(|d| d.tt == Tt::Loi).count().max(b.viec_loi.len())),
        Tt::Cho => b.dong.iter().find(|d| d.tt == Tt::Cho).map_or_else(|| "Đang chờ".into(), |d| format!("{}: {}", d.ten, d.ly_do)),
        Tt::Xong => format!("Xong {}/{} {}", b.xong, b.tong, DON_VI[i]),
        Tt::BoQua => b.cau.clone(),
        Tt::Chua if b.tong == 0 => "Chưa có gì".into(),
        Tt::Chua => format!("Chưa chạy xong · {}/{} {}", b.xong, b.tong, DON_VI[i]),
    }
}

/// Gộp việc của dịch vụ lên 14 bước của 1 bộ; trả các cảnh báo của bộ.
pub fn phu_bo(bo: &mut Bo, viec: &[Viec], bay_gio: f64) -> Vec<CanhBao> {
    let ten_bo = bo.thu_muc.to_string_lossy().into_owned();
    let mut bat_dau = [0.0f64; SO_BUOC];
    let mut quota: Vec<(usize, String)> = vec![];
    for v in viec.iter().filter(|v| v.bo == ten_bo) {
        let Some(i) = buoc_cua_viec(v) else { continue };
        bo.cap_nhat = bo.cap_nhat.max(v.gio_tao).max(v.gio_bat_dau);
        let b = &mut bo.buoc[i];
        phu_dong(b, v);
        match v.trang_thai.as_str() {
            "chay" => {
                b.dang_chay.push(v.id.clone());
                bat_dau[i] = if bat_dau[i] == 0.0 { v.gio_bat_dau } else { bat_dau[i].min(v.gio_bat_dau) };
            }
            "loi" => b.viec_loi.push(v.id.clone()),
            "cat_quota" => quota.push((i, format!("chờ quota tài khoản {} tới {}", v.cho_tk.as_deref().unwrap_or("?"), gio_phut(v.cho_toi)))),
            _ => {}
        }
    }
    let mut cb = vec![];
    for (i, b) in bo.buoc.iter_mut().enumerate() {
        if b.tt == Tt::BoQua {
            continue;
        }
        let co_loi = b.dong.iter().any(|d| d.tt == Tt::Loi) || !b.viec_loi.is_empty();
        b.tt = if !b.dang_chay.is_empty() { Tt::Chay } else if co_loi { Tt::Loi } else if b.dong.iter().any(|d| d.tt == Tt::Cho) { Tt::Cho } else if b.tong > 0 && b.xong == b.tong { Tt::Xong } else { Tt::Chua };
        b.cau = cau_buoc(i, b, bat_dau[i]);
        let loi: Vec<&Dong> = b.dong.iter().filter(|d| d.tt == Tt::Loi).collect();
        if let Some(d) = loi.first() {
            let them = if loi.len() > 1 { format!(" (+{} chương khác)", loi.len() - 1) } else { String::new() };
            cb.push(CanhBao { muc: Muc::Do, bo: Some((bo.thu_muc.clone(), i)), noi_dung: format!("{} · {} · {}: {}{them}", bo.ten, TEN_DAI[i], d.ten, d.ly_do) });
        }
    }
    for (i, ly) in quota {
        cb.push(CanhBao { muc: Muc::Vang, bo: Some((bo.thu_muc.clone(), i)), noi_dung: format!("{} · {} · {ly}", bo.ten, TEN_DAI[i]) });
    }
    ket_bo(bo, !cb.is_empty(), bay_gio);
    cb
}

fn ket_bo(bo: &mut Bo, co_canh_bao: bool, _bay_gio: f64) {
    let dang_chay = bo.buoc.iter().any(|b| b.tt == Tt::Chay);
    let xuat = &bo.buoc[12];
    let xong_het = bo.buoc.iter().enumerate().all(|(i, b)| matches!(b.tt, Tt::Xong | Tt::BoQua) || i == 12 || i == 13 || i == 10);
    bo.tt = if bo.da_dang {
        TtBo::DaDang
    } else if bo.buoc.iter().any(|b| b.tt == Tt::Loi) || (co_canh_bao && bo.buoc.iter().any(|b| b.tt == Tt::Cho)) {
        TtBo::Ket
    } else if xuat.tong > 0 && xong_het {
        TtBo::XongChuaDang
    } else if bo.buoc[9].tt == Tt::Xong || bo.buoc[8].tt == Tt::Xong {
        TtBo::GanXuat
    } else if dang_chay {
        TtBo::DangLam
    } else {
        TtBo::Roi
    };
    bo.cau = cau_bo(bo);
}

fn cau_bo(bo: &Bo) -> String {
    if bo.tam_dung {
        return "Đang tạm dừng".into();
    }
    if let Some((i, b)) = bo.buoc.iter().enumerate().find(|(_, b)| b.tt == Tt::Loi) {
        let d = b.dong.iter().find(|d| d.tt == Tt::Loi);
        return format!("Kẹt ở {}{}", TEN_DAI[i].to_lowercase(), d.map_or_else(String::new, |d| format!(" — {}", d.ten)));
    }
    if let Some((i, b)) = bo.buoc.iter().enumerate().find(|(_, b)| b.tt == Tt::Chay) {
        return format!("Đang {}", TEN_DAI[i].to_lowercase()) + &(if b.tong > 0 { format!(" · {}/{}", b.xong, b.tong) } else { String::new() });
    }
    if let Some((i, _)) = bo.buoc.iter().enumerate().find(|(_, b)| b.tt == Tt::Cho) {
        return format!("Đang chờ ở {}", TEN_DAI[i].to_lowercase());
    }
    match bo.tt {
        TtBo::DaDang => "Đã đăng".into(),
        TtBo::XongChuaDang => "Xong, chờ đăng".into(),
        _ => bo.buoc.iter().enumerate().find(|(_, b)| matches!(b.tt, Tt::Chua)).map_or_else(|| "Xong".into(), |(i, _)| format!("Chưa chạy — bước kế: {}", TEN_DAI[i].to_lowercase())),
    }
}

/// Dòng số của thẻ: "Kịch bản 106/111 · Khung 111 · Timeline 14 · Đã xuất 0".
pub fn dong_so(bo: &Bo) -> String {
    let b = &bo.buoc;
    format!("Kịch bản {}/{} · Khung {} · Timeline {} · Đã xuất {}", b[5].xong, b[5].tong, b[3].xong, b[9].tong, b[12].tong)
}

/// Mức "đáng chú ý" để sắp lưới: kẹt → đang làm → gần xuất → xong chờ đăng → rảnh → đã đăng.
pub fn thu_tu_sap(t: TtBo) -> u8 {
    match t {
        TtBo::Ket => 0,
        TtBo::DangLam => 1,
        TtBo::GanXuat => 2,
        TtBo::XongChuaDang => 3,
        TtBo::Roi => 4,
        TtBo::DaDang => 5,
    }
}

pub fn phu_tat_ca(bo: &mut [Bo], viec: &[Viec], dv: &DichVu, bay_gio: f64) -> Vec<CanhBao> {
    let mut cb: Vec<CanhBao> = bo.iter_mut().flat_map(|b| phu_bo(b, viec, bay_gio)).collect();
    cb.extend(dv.may_khac.iter().filter(|m| !m.truc_tuyen).map(|m| CanhBao {
        muc: Muc::Vang,
        bo: None,
        noi_dung: format!("Máy {} mất kết nối từ {}", m.ten, m.mat_tu.map_or_else(String::new, gio_phut)),
    }));
    // cảnh báo của dịch vụ về việc lỗi đã có dòng riêng theo bộ/bước ở trên; còn lại (ổ đầy, quota hết, sao lưu cũ…) giữ nguyên
    cb.extend(dv.canh_bao.iter().filter(|c| !c.khoa.starts_with("loi") && !c.khoa.starts_with("mat:")).map(|c| CanhBao { muc: Muc::Vang, bo: None, noi_dung: c.noi_dung.clone() }));
    cb.sort_by_key(|c| c.muc == Muc::Vang);
    cb
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tao(p: &Path, noi_dung: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, noi_dung).unwrap();
    }

    #[test]
    fn dem_va_kiem_doan() {
        let d = std::env::temp_dir().join(format!("recap-gui-thu-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let bo = d.join("Thu Bo");
        tao(&bo.join("bo.json"), r#"{"ten":"Thử","giong":["g1"]}"#);
        for n in ["001.jpg", "002.jpg", "003.jpg"] {
            tao(&bo.join("nguon/ch001").join(n), "x");
        }
        tao(&bo.join("nguon/ch002/001.jpg"), "x");
        tao(&bo.join("kich-ban/arc01/ch001.md"), "## SCRIPT\n\nMột.\n\nHai.\n");
        tao(&bo.join("kich-ban/arc01/ch001.doi-chieu.md"), "x");
        tao(&bo.join("kich-ban/arc01/ch002.md"), "## SCRIPT\n\nMột.\n");
        tao(&bo.join("khung/ch001.json"), "{}");
        tao(&bo.join("giong/g1/ch002/ch002.mp3"), "x");
        let b = quet_bo(&bo).unwrap();
        assert_eq!((b.buoc[0].xong, b.buoc[0].tong), (2, 2));
        assert_eq!((b.buoc[3].xong, b.buoc[3].tong), (1, 2));
        assert_eq!((b.buoc[5].xong, b.buoc[5].tong), (1, 2)); // ch001 đủ md + đối chiếu, ch002 thiếu đối chiếu
        assert_eq!((b.buoc[7].xong, b.buoc[7].tong), (1, 2)); // ch001: 2 đoạn / 3 trang -> lỗi; ch002: 1/1 đạt
        assert_eq!(b.buoc[7].dong[0].tt, Tt::Loi);
        assert!(b.buoc[7].dong[0].ly_do.starts_with("2 đoạn cho 3 trang"));
        assert_eq!((b.buoc[8].xong, b.buoc[8].tong), (1, 2));
        assert_eq!(b.buoc[11].tt, Tt::BoQua);
        let mut b = b;
        let cb = phu_bo(&mut b, &[], 0.0);
        assert_eq!(cb.len(), 1);
        assert_eq!(b.tt, TtBo::Ket);
        let _ = std::fs::remove_dir_all(&d);
    }
}
