//! Đọc dữ liệu 1 chương theo định dạng CŨ của bộ (`~/Recap SaaS Projects/<bộ>`). CHỈ ĐỌC — không ghi gì vào thư mục bộ.
use serde_json::Value;
use std::path::{Path, PathBuf};

pub type Kq<T> = Result<T, String>;

pub fn doc_json(p: &Path) -> Kq<Value> {
    let s = std::fs::read_to_string(p).map_err(|e| format!("đọc {}: {e}", p.display()))?;
    serde_json::from_str(&s).map_err(|e| format!("JSON hỏng {}: {e}", p.display()))
}

pub fn tim_moc(dir: &Path) -> Option<PathBuf> {
    tim(dir, "moc-neo-", ".json")
}

/// File đầu tiên (theo tên) trong `dir` khớp `tien_to*hau_to`.
fn tim(dir: &Path, tien_to: &str, hau_to: &str) -> Option<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(tien_to) && n.ends_with(hau_to)))
        .collect();
    v.sort();
    v.into_iter().next()
}

#[derive(Debug, Clone)]
pub struct Cau {
    pub text: String,
    pub bat_dau: f64,
    pub dai: f64,
}

#[derive(Debug, Clone)]
pub struct Trang {
    pub ten: String, // "003.jpg"
    pub tu: f64,
    pub den: f64,
    /// truot-song | camera-path | quet-doc | tinh | tinh-phong | fade | slide
    pub kieu: String,
    pub anh: PathBuf,
    pub w: u32,
    pub h: u32,
    /// các khung (viewport chuẩn hoá [x,y,w,h]) theo thứ tự đọc, từ `panel-maps/NNN.json`
    pub khung: Vec<[f64; 4]>,
}

#[derive(Debug)]
pub struct Chuong {
    pub ma: String,
    pub voice: PathBuf,
    pub trang: Vec<Trang>,
    pub cau: Vec<Cau>,
    pub tong: f64,
    /// `Video/chNNN/truot-song/ke-hoach-*.json` + thư mục chứa ảnh dải/nền đã vẽ sẵn
    pub ke_hoach: Option<(Value, PathBuf)>,
    pub ten_rieng: Vec<String>,
}

/// Kiểu dựng mặc định của bộ (`_du-an.json` → `kieu_dung`); thiếu/lạ = truot-song, như backend.
fn kieu_mac_dinh(goc: &Path) -> String {
    let v = doc_json(&goc.join("_du-an.json")).ok().and_then(|d| d["kieu_dung"].as_str().map(String::from));
    match v.as_deref() {
        Some(k @ ("truot-nhanh" | "camera-path" | "truot-song" | "tinh-phong")) => k.into(),
        _ => "truot-song".into(),
    }
}

fn thu_muc_anh(goc: &Path, ma: &str) -> Kq<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(goc.join("Manga"))
        .map_err(|e| format!("không đọc được Manga/: {e}"))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n == ma || n.starts_with(&format!("{ma} - "))))
        .collect();
    v.sort();
    v.into_iter().next().ok_or_else(|| format!("không có thư mục ảnh Manga/{ma} - *"))
}

fn doc_khung(goc: &Path, ma: &str, trang: &str) -> Vec<[f64; 4]> {
    let stem = trang.rsplit_once('.').map_or(trang, |x| x.0);
    let Ok(pm) = doc_json(&goc.join(format!("Video/{ma}/panel-maps/{stem}.json"))) else { return vec![] };
    pm["panels"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    let v = p["viewport"].as_array()?;
                    Some([v.first()?.as_f64()?, v.get(1)?.as_f64()?, v.get(2)?.as_f64()?, v.get(3)?.as_f64()?])
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Tên riêng của arc chứa chương (`Script/arcNN-*/_ten-rieng.json`: khoá = tên viết trong kịch bản). Bản lời cuối
/// (`Script/chinh/chNNN.md`) nằm ngoài thư mục arc, nên tìm arc theo file kịch bản `chNNN.md`.
fn doc_ten_rieng(goc: &Path, ma: &str, arc: &str) -> Vec<String> {
    let mut arcs: Vec<PathBuf> = std::fs::read_dir(goc.join("Script")).into_iter().flatten().flatten().map(|e| e.path()).collect();
    arcs.sort();
    let dir = arcs.into_iter().find(|d| d.join(format!("{ma}.md")).is_file() && d.join("_ten-rieng.json").is_file()).unwrap_or_else(|| goc.join("Script").join(arc));
    doc_json(&dir.join("_ten-rieng.json"))
        .ok()
        .and_then(|v| v.as_object().map(|o| o.keys().cloned().collect()))
        .unwrap_or_default()
}

/// Thứ tự trang + giờ bắt đầu từng trang theo mốc câu (trang đầu từ 0; trang cuối tới hết giọng).
fn trang_theo_moc(cau_json: &[Value]) -> (Vec<Cau>, Vec<(String, f64)>) {
    let mut cau = vec![];
    let mut bd: Vec<(String, f64)> = vec![];
    for c in cau_json {
        let (t, d) = (c["bat_dau"].as_f64().unwrap_or(0.0), c["dai"].as_f64().unwrap_or(0.0));
        cau.push(Cau { text: c["loi_ke"].as_str().unwrap_or_default().into(), bat_dau: t, dai: d });
        if let Some(tr) = c["trang"].as_str().filter(|tr| !bd.iter().any(|(x, _)| x == tr)) {
            bd.push((tr.into(), t));
        }
    }
    (cau, bd)
}

pub fn doc_chuong(goc: &Path, ma: &str) -> Kq<Chuong> {
    let vd = goc.join("Video").join(ma);
    let moc_f = tim(&vd.join("_technical"), "moc-neo-", ".json").ok_or_else(|| format!("{ma}: chưa có mốc câu (moc-neo-*.json)"))?;
    let moc = doc_json(&moc_f)?;
    let arc = moc["arc"].as_str().unwrap_or("chinh");
    let ban = moc["ban_tieng"].as_str().unwrap_or_default();
    let voice = goc.join(format!("Script/{arc}/voice-{ban}/{ma}.mp3"));
    if !voice.is_file() {
        return Err(format!("{ma}: chưa có giọng {}", voice.display()));
    }
    let cau_json = moc["moc"].as_array().ok_or("moc-neo thiếu mảng moc")?;
    let kieu_bo = kieu_mac_dinh(goc);
    let plan = doc_json(&vd.join("_technical/chapter-plan.json")).unwrap_or(Value::Null);
    let anh_dir = thu_muc_anh(goc, ma)?;
    let ke_hoach = tim(&vd.join("truot-song"), "ke-hoach-", ".json").and_then(|f| doc_json(&f).ok().map(|v| (v, vd.join("truot-song"))));

    let (cau, bat_dau_trang) = trang_theo_moc(cau_json);
    let last = cau_json.last().ok_or("mảng moc rỗng")?;
    let tong = last["bat_dau"].as_f64().unwrap_or(0.0) + last["dai"].as_f64().unwrap_or(0.0);
    let mut trang = vec![];
    for (i, (ten, tu)) in bat_dau_trang.iter().enumerate() {
        let den = bat_dau_trang.get(i + 1).map_or(tong, |x| x.1);
        let anh = anh_dir.join(ten);
        let (w, h) = image::image_dimensions(&anh).map_err(|e| format!("đọc cỡ ảnh {}: {e}", anh.display()))?;
        let kieu = plan["hieu_ung_trang"][ten.as_str()].as_str().map_or(kieu_bo.clone(), String::from);
        trang.push(Trang { ten: ten.clone(), tu: if i == 0 { 0.0 } else { *tu }, den, kieu, anh, w, h, khung: doc_khung(goc, ma, ten) });
    }
    Ok(Chuong { ma: ma.into(), voice, trang, cau, tong, ke_hoach, ten_rieng: doc_ten_rieng(goc, ma, arc) })
}
