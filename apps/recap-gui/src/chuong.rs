//! Dữ liệu 1 chương cho Màn 3 (spec `03-xem-chuong.md`): trang + khung + lớp che chữ + lời kể + đối chiếu, và
//! mọi thao tác GHI có phiên bản. Luật: sửa = bản mới; bản cũ vào `.phien-ban/<loại>/<chương>/`; ghi nguyên tử
//! (file tạm cùng thư mục rồi rename — không bao giờ ghi đè tại chỗ, vì ảnh/giọng là hardlink với bộ cũ).
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Hop {
    /// x0,y0,x1,y1 theo điểm ảnh của ảnh gốc
    pub r: [f32; 4],
    pub giu: bool,
    pub chu: String,
    /// chỉ số trong file gốc (để giữ nguyên các khoá khác khi lưu); None = hộp mới
    pub goc: Option<usize>,
}

#[derive(Clone, Default)]
pub struct DanhDau {
    pub nghi_lech: bool,
    pub cao_trao: bool,
    pub bo: bool,
    pub ghi_chu: String,
    pub kieu: String,
}

#[derive(Clone)]
pub struct TrangDl {
    pub ten: String,
    pub file: Option<PathBuf>,
    pub w: f32,
    pub h: f32,
    pub khung: Vec<Hop>,
    pub che: Vec<Hop>,
    pub ocr: Vec<Hop>,
    pub dd: DanhDau,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DoiChieu {
    pub so: usize,
    pub en: String,
    pub vi: String,
    pub thay: String,
}

/// Kết quả so số đoạn với số trang.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lech {
    pub so_doan: usize,
    pub so_trang: usize,
    /// đoạn thừa (chỉ số), trang thiếu đoạn (chỉ số)
    pub thua: Vec<usize>,
    pub thieu: Vec<usize>,
    /// 2 đoạn liền kề nên gộp (i, i+1)
    pub goi_y_gop: Option<(usize, usize)>,
    /// trang → đoạn (khi lệch dùng căn theo nội dung, khi khớp là theo thứ tự)
    pub anh_xa: Vec<Option<usize>>,
}

impl Lech {
    pub fn lech(&self) -> bool {
        self.so_doan != self.so_trang
    }
}

pub struct Chuong {
    pub bo: PathBuf,
    pub ma: String,
    pub trang: Vec<TrangDl>,
    pub dau: String,
    pub doan: Vec<String>,
    pub doan_luu: Vec<String>,
    pub kb: Option<PathBuf>,
    pub doi_chieu: Vec<DoiChieu>,
    pub khung_json: Option<Value>,
    pub che_json: Option<Value>,
    pub co_ocr: bool,
    pub lech: Lech,
    pub khung_bat: bool,
    pub che_bat: bool,
    pub dd_bat: bool,
    pub tre: Option<String>,
}

pub fn ten_file(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn doc_json(p: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

pub fn ghi_nguyen_tu(p: &Path, noi_dung: &[u8]) -> std::io::Result<()> {
    if let Some(cha) = p.parent() {
        std::fs::create_dir_all(cha)?;
    }
    let tam = p.with_file_name(format!(".{}.{}.tmp", ten_file(p), std::process::id()));
    std::fs::write(&tam, noi_dung)?;
    std::fs::rename(&tam, p)
}

// ---------- kịch bản ----------

/// Tách file `.md` thành (phần đầu gồm dòng `## SCRIPT`, các đoạn). Cùng cách đếm với `kiem.py`.
pub fn tach_kich_ban(raw: &str) -> (String, Vec<String>) {
    let (dau, than) = match raw.rfind("## SCRIPT") {
        Some(i) => (raw[..i + "## SCRIPT".len()].to_owned(), &raw[i + "## SCRIPT".len()..]),
        None => (String::new(), raw),
    };
    (dau, than.trim().split("\n\n").map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect())
}

pub fn ghep_kich_ban(dau: &str, doan: &[String]) -> String {
    format!("{dau}\n\n{}\n", doan.join("\n\n"))
}

pub fn tim_kich_ban(bo: &Path, ma: &str) -> Option<PathBuf> {
    let kb = bo.join("kich-ban");
    let mut arcs: Vec<PathBuf> = std::fs::read_dir(&kb).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir() && ten_file(p).starts_with("arc")).collect();
    arcs.sort();
    arcs.into_iter().map(|a| a.join(format!("{ma}.md"))).find(|p| p.is_file())
}

pub fn doc_doi_chieu(p: &Path) -> Vec<DoiChieu> {
    let Ok(raw) = std::fs::read_to_string(p) else { return vec![] };
    raw.lines()
        .filter_map(|l| {
            let mut it = l.split(" | ");
            let so = it.next()?.trim().parse::<usize>().ok()?;
            let mut r = DoiChieu { so, en: String::new(), vi: String::new(), thay: String::new() };
            for part in it {
                if let Some(v) = part.strip_prefix("EN:") {
                    r.en = v.trim().to_owned();
                } else if let Some(v) = part.strip_prefix("VI:") {
                    r.vi = v.trim().to_owned();
                } else if let Some(v) = part.strip_prefix("THAY:") {
                    r.thay = v.trim().to_owned();
                }
            }
            Some(r)
        })
        .collect()
}

fn tu(s: &str) -> BTreeSet<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect()
}

fn giong_nhau(a: &str, b: &str) -> f32 {
    let (x, y) = (tu(a), tu(b));
    if x.is_empty() || y.is_empty() {
        return 0.0;
    }
    x.intersection(&y).count() as f32 / x.union(&y).count() as f32
}

/// So số đoạn với số trang. Khi lệch và có file đối chiếu: căn đơn điệu đoạn ↔ trang theo độ giống nội dung
/// (quy hoạch động, bỏ đoạn/bỏ trang có phạt) để chỉ ra đoạn thừa / trang thiếu và 2 đoạn nên gộp.
pub fn phan_tich(doan: &[String], so_trang: usize, doi_chieu: &[DoiChieu]) -> Lech {
    let (n, m) = (doan.len(), so_trang);
    let theo_thu_tu = || (0..m).map(|j| (j < n).then_some(j)).collect::<Vec<_>>();
    let mut l = Lech { so_doan: n, so_trang: m, anh_xa: theo_thu_tu(), ..Lech::default() };
    if n == m || doi_chieu.is_empty() || m == 0 {
        l.thua = (m..n).collect();
        l.thieu = (n..m).collect();
        return l;
    }
    let mo_ta = |j: usize| doi_chieu.iter().find(|r| r.so == j + 1).map(|r| format!("{} {}", r.vi, r.thay)).unwrap_or_default();
    let mo_ta: Vec<String> = (0..m).map(mo_ta).collect();
    let phat = 0.15f32;
    let mut f = vec![vec![0.0f32; m + 1]; n + 1];
    for i in 0..=n {
        for j in 0..=m {
            if i == 0 && j == 0 {
                continue;
            }
            let mut best = f32::MIN;
            if i > 0 && j > 0 {
                best = f[i - 1][j - 1] + giong_nhau(&doan[i - 1], &mo_ta[j - 1]);
            }
            if i > 0 {
                best = best.max(f[i - 1][j] - phat);
            }
            if j > 0 {
                best = best.max(f[i][j - 1] - phat);
            }
            f[i][j] = best;
        }
    }
    let (mut i, mut j) = (n, m);
    l.anh_xa = vec![None; m];
    l.thua.clear();
    l.thieu.clear();
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && (f[i][j] - (f[i - 1][j - 1] + giong_nhau(&doan[i - 1], &mo_ta[j - 1]))).abs() < 1e-5 {
            l.anh_xa[j - 1] = Some(i - 1);
            i -= 1;
            j -= 1;
        } else if i > 0 && (j == 0 || (f[i][j] - (f[i - 1][j] - phat)).abs() < 1e-5) {
            l.thua.push(i - 1);
            i -= 1;
        } else {
            l.thieu.push(j - 1);
            j -= 1;
        }
    }
    l.thua.reverse();
    l.thieu.reverse();
    if let Some(&s) = l.thua.first() {
        let cand = [s.checked_sub(1), (s + 1 < n).then_some(s + 1)];
        l.goi_y_gop = cand.into_iter().flatten().max_by(|a, b| giong_nhau(&doan[s], &doan[*a]).total_cmp(&giong_nhau(&doan[s], &doan[*b]))).map(|k| (k.min(s), k.max(s)));
    }
    l
}

pub fn gop_doan(doan: &mut Vec<String>, i: usize) {
    if i + 1 < doan.len() {
        let b = doan.remove(i + 1);
        doan[i] = format!("{} {}", doan[i].trim_end(), b.trim_start());
    }
}

/// Tách đoạn `i` tại vị trí ký tự `at` (đến hết câu gần nhất sau `at` nếu `at` nằm giữa chữ).
pub fn tach_doan(doan: &mut Vec<String>, i: usize, at: usize) -> bool {
    let Some(s) = doan.get(i).cloned() else { return false };
    let ch: Vec<char> = s.chars().collect();
    let mut cat = at.min(ch.len());
    while cat < ch.len() && !(cat > 0 && matches!(ch[cat - 1], '.' | '!' | '?' | '…' | '"') && ch[cat].is_whitespace()) {
        cat += 1;
    }
    let (a, b): (String, String) = (ch[..cat].iter().collect(), ch[cat..].iter().collect());
    if a.trim().is_empty() || b.trim().is_empty() {
        return false;
    }
    doan[i] = a.trim().to_owned();
    doan.insert(i + 1, b.trim().to_owned());
    true
}

// ---------- đọc ----------

fn la_anh(p: &Path) -> bool {
    matches!(p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref(), Some("jpg" | "jpeg" | "png" | "webp"))
}

fn hop_tu_mang(v: Option<&Value>, giu: &str, chu: &str) -> Vec<Hop> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .enumerate()
                .filter_map(|(k, h)| {
                    let b = h.get("box").or_else(|| h.get("bbox"))?.as_array()?;
                    let r: Vec<f32> = b.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();
                    (r.len() == 4).then(|| Hop { r: [r[0], r[1], r[2], r[3]], giu: h.get(giu).and_then(Value::as_bool).unwrap_or(false), chu: h.get(chu).and_then(Value::as_str).unwrap_or("").to_owned(), goc: Some(k) })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn trang_json<'a>(j: &'a Option<Value>, key: &str, ten: &str) -> Option<&'a Value> {
    j.as_ref()?.get("trang")?.as_array()?.iter().find(|t| t.get(key).and_then(Value::as_str) == Some(ten))
}

impl Chuong {
    pub fn doc(bo: &Path, ma: &str) -> Chuong {
        let anh_dir = bo.join("nguon").join(ma);
        let mut anh: Vec<PathBuf> = std::fs::read_dir(&anh_dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_file() && la_anh(p)).collect();
        anh.sort();
        let khung_json = doc_json(&bo.join("khung").join(format!("{ma}.json")));
        let che_json = doc_json(&bo.join("che-chu").join(format!("{ma}.json")));
        let ocr_json = doc_json(&bo.join("ocr").join(format!("{ma}.json")));
        let dd_json = doc_json(&bo.join("danh-dau").join(format!("{ma}.json")));
        let kb = tim_kich_ban(bo, ma);
        let raw = kb.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        let (dau, doan) = tach_kich_ban(&raw);
        let doi_chieu = kb.as_ref().map(|p| doc_doi_chieu(&p.with_file_name(format!("{ma}.doi-chieu.md")))).unwrap_or_default();
        let ten_trang: Vec<String> = if anh.is_empty() {
            khung_json.as_ref().and_then(|j| j.get("trang")?.as_array()).map(|a| a.iter().filter_map(|t| t.get("image").and_then(Value::as_str).map(str::to_owned)).collect()).unwrap_or_default()
        } else {
            anh.iter().map(|p| ten_file(p)).collect()
        };
        let trang: Vec<TrangDl> = ten_trang
            .iter()
            .map(|ten| {
                let file = anh.iter().find(|p| ten_file(p) == *ten).cloned();
                let kj = trang_json(&khung_json, "image", ten);
                let kich = file.as_ref().and_then(|f| image::image_dimensions(f).ok());
                let dim = |k: &str, d: Option<u32>| kj.and_then(|t| t.get(k)).and_then(Value::as_f64).map(|x| x as f32).or(d.map(|x| x as f32)).unwrap_or(0.0);
                let (w, h) = (dim("width", kich.map(|k| k.0)), dim("height", kich.map(|k| k.1)));
                let khung = kj
                    .and_then(|t| t.get("panels"))
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .enumerate()
                            .filter_map(|(k, p)| {
                                let b: Vec<f32> = p.get("box")?.as_array()?.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();
                                (b.len() == 4).then(|| Hop { r: [b[0], b[1], b[2], b[3]], giu: false, chu: String::new(), goc: Some(k) })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let che = hop_tu_mang(trang_json(&che_json, "anh", ten).and_then(|t| t.get("hop")), "giu", "chu");
                let ocr = hop_tu_mang(trang_json(&ocr_json, "anh", ten).or_else(|| trang_json(&ocr_json, "image", ten)).and_then(|t| t.get("boxes").or_else(|| t.get("hop"))), "giu", "text");
                let d = dd_json.as_ref().and_then(|j| j.get("trang")?.get(ten));
                let b = |k: &str| d.and_then(|x| x.get(k)).and_then(Value::as_bool).unwrap_or(false);
                let s = |k: &str| d.and_then(|x| x.get(k)).and_then(Value::as_str).unwrap_or("").to_owned();
                let bo_tu_che = trang_json(&che_json, "anh", ten).and_then(|t| t.get("bo")).and_then(Value::as_bool).unwrap_or(false);
                TrangDl { ten: ten.clone(), file, w, h, khung, che, ocr, dd: DanhDau { nghi_lech: b("nghi_lech"), cao_trao: b("cao_trao"), bo: b("bo") || bo_tu_che, ghi_chu: s("ghi_chu"), kieu: s("kieu") } }
            })
            .collect();
        let lech = phan_tich(&doan, trang.len(), &doi_chieu);
        let co_ocr = ocr_json.is_some();
        let tre = tre_timeline(bo, ma, kb.as_deref());
        Chuong { bo: bo.to_path_buf(), ma: ma.to_owned(), trang, dau, doan_luu: doan.clone(), doan, kb, doi_chieu, khung_json, che_json, co_ocr, lech, khung_bat: true, che_bat: true, dd_bat: false, tre }
    }

    pub fn tinh_lai_lech(&mut self) {
        self.lech = phan_tich(&self.doan, self.trang.len(), &self.doi_chieu);
    }

    pub fn doan_trang(&self, j: usize) -> Option<usize> {
        self.lech.anh_xa.get(j).copied().flatten()
    }

    pub fn kb_bien_doi(&self) -> bool {
        self.doan != self.doan_luu
    }

    /// Trạng thái chương: None = Đạt, Some(lý do).
    pub fn loi(&self) -> Option<String> {
        if self.kb.is_none() {
            return Some("chưa có kịch bản".into());
        }
        if self.lech.lech() {
            return Some(format!("{} đoạn cho {} trang", self.lech.so_doan, self.lech.so_trang));
        }
        None
    }

    pub fn khung_bien_doi(&self) -> bool {
        self.trang.iter().any(|t| t.khung != hop_tu_mang_khung(&self.khung_json, &t.ten))
    }

    pub fn che_bien_doi(&self) -> bool {
        self.trang.iter().any(|t| t.che != hop_tu_mang(trang_json(&self.che_json, "anh", &t.ten).and_then(|x| x.get("hop")), "giu", "chu"))
    }

    // ---------- ghi (đều tạo phiên bản) ----------

    pub fn luu_kich_ban(&mut self, ai: &str, gi: &str) -> std::io::Result<()> {
        let Some(p) = self.kb.clone() else { return Err(std::io::Error::other("chương chưa có file kịch bản")) };
        ghi_phien_ban(&self.bo, "kich-ban", &self.ma, &p, ghep_kich_ban(&self.dau, &self.doan).as_bytes(), ai, gi)?;
        self.doan_luu = self.doan.clone();
        Ok(())
    }

    pub fn luu_khung(&mut self, ai: &str, gi: &str) -> std::io::Result<()> {
        let p = self.bo.join("khung").join(format!("{}.json", self.ma));
        let mut j = self.khung_json.clone().unwrap_or_else(|| json!({"chuong": self.ma, "nguon": "tay", "trang": []}));
        let ds = j["trang"].as_array().cloned().unwrap_or_default();
        let mut moi: Vec<Value> = vec![];
        for t in &self.trang {
            let mut e = ds.iter().find(|x| x.get("image").and_then(Value::as_str) == Some(&t.ten)).cloned().unwrap_or_else(|| json!({"version": 2, "detector": "tay", "image": t.ten, "width": t.w, "height": t.h, "panels": []}));
            let cu = e["panels"].as_array().cloned().unwrap_or_default();
            let mut ps: Vec<Value> = vec![];
            for (k, h) in t.khung.iter().enumerate() {
                let vp = if t.w > 0.0 && t.h > 0.0 { json!([h.r[0] / t.w, h.r[1] / t.h, (h.r[2] - h.r[0]) / t.w, (h.r[3] - h.r[1]) / t.h]) } else { json!([0, 0, 1, 1]) };
                let mut p = h.goc.and_then(|g| cu.get(g).cloned()).unwrap_or_else(|| json!({"id": format!("p{:02}", k + 1), "confidence": 1.0}));
                let doi = h.goc.and_then(|g| cu.get(g)).and_then(|c| c.get("box")).and_then(Value::as_array).is_none_or(|b| b.iter().filter_map(Value::as_f64).map(|x| x as f32).collect::<Vec<_>>() != h.r);
                p["box"] = json!(h.r.map(|x| x.round()));
                if doi {
                    p["viewport"] = vp;
                    p["reviewed"] = json!(true);
                    p["nguon"] = json!("tay");
                }
                p["id"] = json!(format!("p{:02}", k + 1));
                ps.push(p);
            }
            e["panels"] = Value::Array(ps);
            moi.push(e);
        }
        j["trang"] = Value::Array(moi);
        ghi_phien_ban(&self.bo, "khung", &self.ma, &p, serde_json::to_string_pretty(&j)?.as_bytes(), ai, gi)?;
        self.khung_json = Some(j);
        Ok(())
    }

    pub fn luu_che(&mut self, ai: &str, gi: &str) -> std::io::Result<()> {
        let p = self.bo.join("che-chu").join(format!("{}.json", self.ma));
        let mut j = self.che_json.clone().unwrap_or_else(|| json!({"chuong": self.ma, "phien": 1, "trang": []}));
        let ds = j["trang"].as_array().cloned().unwrap_or_default();
        let moi: Vec<Value> = self
            .trang
            .iter()
            .map(|t| {
                let mut e = ds.iter().find(|x| x.get("anh").and_then(Value::as_str) == Some(&t.ten)).cloned().unwrap_or_else(|| json!({"anh": t.ten, "bo": false, "hop": []}));
                let cu = e["hop"].as_array().cloned().unwrap_or_default();
                e["hop"] = Value::Array(
                    t.che
                        .iter()
                        .map(|h| {
                            let mut x = h.goc.and_then(|g| cu.get(g).cloned()).unwrap_or_else(|| json!({}));
                            x["box"] = json!(h.r.map(|v| v.round()));
                            x["giu"] = json!(h.giu);
                            x
                        })
                        .collect(),
                );
                e
            })
            .collect();
        j["trang"] = Value::Array(moi);
        ghi_phien_ban(&self.bo, "che-chu", &self.ma, &p, serde_json::to_string_pretty(&j)?.as_bytes(), ai, gi)?;
        self.che_json = Some(j);
        Ok(())
    }

    pub fn luu_danh_dau(&mut self, ai: &str, gi: &str) -> std::io::Result<()> {
        let p = self.bo.join("danh-dau").join(format!("{}.json", self.ma));
        let mut m = serde_json::Map::new();
        for t in &self.trang {
            m.insert(t.ten.clone(), json!({"nghi_lech": t.dd.nghi_lech, "cao_trao": t.dd.cao_trao, "bo": t.dd.bo, "ghi_chu": t.dd.ghi_chu, "kieu": t.dd.kieu}));
        }
        let j = json!({"chuong": self.ma, "trang": m});
        ghi_phien_ban(&self.bo, "danh-dau", &self.ma, &p, serde_json::to_string_pretty(&j)?.as_bytes(), ai, gi)
    }
}

fn hop_tu_mang_khung(j: &Option<Value>, ten: &str) -> Vec<Hop> {
    trang_json(j, "image", ten).and_then(|t| t.get("panels")).map(|ps| ps.as_array().map(|a| {
        a.iter().enumerate().filter_map(|(k, p)| {
            let b: Vec<f32> = p.get("box")?.as_array()?.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();
            (b.len() == 4).then(|| Hop { r: [b[0], b[1], b[2], b[3]], giu: false, chu: String::new(), goc: Some(k) })
        }).collect()
    }).unwrap_or_default()).unwrap_or_default()
}

// ---------- phiên bản ----------

fn lam_sach(s: &str) -> String {
    let t: String = s.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' }).collect();
    t.chars().take(40).collect()
}

/// Ghi `noi_dung` vào `dich`; nếu `dich` đã có thì bản cũ chép vào `.phien-ban/<loai>/<ma>/` trước.
pub fn ghi_phien_ban(bo: &Path, loai: &str, ma: &str, dich: &Path, noi_dung: &[u8], ai: &str, gi: &str) -> std::io::Result<()> {
    if let Ok(cu) = std::fs::read(dich) {
        let dir = bo.join(".phien-ban").join(loai).join(ma);
        std::fs::create_dir_all(&dir)?;
        let ext = dich.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
        let t = chrono::Local::now().format("%Y%m%d-%H%M%S%.3f");
        ghi_nguyen_tu(&dir.join(format!("{t}--{}--{}.{ext}", lam_sach(ai), lam_sach(gi))), &cu)?;
    }
    ghi_nguyen_tu(dich, noi_dung)
}

#[derive(Clone)]
pub struct PhienBan {
    pub loai: String,
    pub file: PathBuf,
    pub gio: String,
    pub ai: String,
    pub gi: String,
}

pub fn ds_phien_ban(bo: &Path, ma: &str) -> Vec<PhienBan> {
    let mut v = vec![];
    for loai in ["kich-ban", "khung", "che-chu", "danh-dau"] {
        for e in std::fs::read_dir(bo.join(".phien-ban").join(loai).join(ma)).into_iter().flatten().flatten() {
            let ten = ten_file(&e.path());
            let mut it = ten.splitn(3, "--");
            let (g, a, gi) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or(""));
            let gio = chrono::NaiveDateTime::parse_from_str(g, "%Y%m%d-%H%M%S%.3f").map(|d| d.format("%d/%m %H:%M:%S").to_string()).unwrap_or_else(|_| g.to_owned());
            let gi = gi.rsplit_once('.').map_or(gi, |x| x.0).replace('_', " ");
            v.push(PhienBan { loai: loai.into(), file: e.path(), gio, ai: a.replace('_', " "), gi });
        }
    }
    v.sort_by(|a, b| ten_file(&b.file).cmp(&ten_file(&a.file)));
    v
}

pub fn ten_loai(l: &str) -> &'static str {
    match l {
        "kich-ban" => "Lời kể",
        "khung" => "Khung",
        "che-chu" => "Che chữ",
        _ => "Đánh dấu",
    }
}

/// Khôi phục = bản hiện tại thành 1 phiên bản mới (không mất), rồi ghi nội dung bản cũ làm bản hiện hành.
pub fn khoi_phuc(bo: &Path, ma: &str, pb: &PhienBan) -> std::io::Result<()> {
    let dich = match pb.loai.as_str() {
        "kich-ban" => tim_kich_ban(bo, ma).ok_or_else(|| std::io::Error::other("không thấy file kịch bản"))?,
        "khung" => bo.join("khung").join(format!("{ma}.json")),
        "che-chu" => bo.join("che-chu").join(format!("{ma}.json")),
        _ => bo.join("danh-dau").join(format!("{ma}.json")),
    };
    ghi_phien_ban(bo, &pb.loai, ma, &dich, &std::fs::read(&pb.file)?, "Thịnh", &format!("khôi phục bản {}", pb.gio))
}

// ---------- timeline cũ hơn dữ liệu ----------

fn mtime(p: &Path) -> f64 {
    std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0.0, |d| d.as_secs_f64())
}

/// `Some(nhãn)` nếu chương đã có timeline mà dữ liệu (kịch bản/khung/che chữ) mới hơn lúc máy dựng.
fn tre_timeline(bo: &Path, ma: &str, kb: Option<&Path>) -> Option<String> {
    let tl = bo.join("timeline");
    if !tl.join(format!("{ma}.fcproj")).is_file() {
        return None;
    }
    let dau = doc_json(&tl.join(format!("{ma}.dau-may.json")));
    let gio = dau.as_ref().and_then(|d| d.get("gio")?.as_f64()).unwrap_or_else(|| mtime(&tl.join(format!("{ma}.fcproj"))));
    let moi = [kb.map(mtime).unwrap_or(0.0), mtime(&bo.join("khung").join(format!("{ma}.json"))), mtime(&bo.join("che-chu").join(format!("{ma}.json")))].into_iter().fold(0.0, f64::max);
    (moi > gio + 1.0).then(|| "Timeline đang dùng bản cũ".to_owned())
}

#[cfg(test)]
mod thu {
    use super::*;

    fn ds(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn tach_ghep_giu_nguyen_dau() {
        let raw = "### Ch.064 — A\n\n## SCRIPT\n\n[bỏ qua]\n\nĐoạn hai.\n";
        let (dau, d) = tach_kich_ban(raw);
        assert_eq!(d, ds(&["[bỏ qua]", "Đoạn hai."]));
        assert_eq!(ghep_kich_ban(&dau, &d), raw);
    }

    #[test]
    fn lech_chi_dung_doan_thua() {
        let rows: Vec<DoiChieu> = ["Chiyo than thở cân nặng", "Nozaki mang bánh kem đến", "Chiyo giận vì bánh kem"].iter().enumerate().map(|(i, s)| DoiChieu { so: i + 1, en: String::new(), vi: (*s).into(), thay: String::new() }).collect();
        let d = ds(&["Chiyo than thở cân nặng", "Nozaki mang bánh kem đến cho Chiyo", "Nozaki mang bánh kem", "Chiyo giận vì bánh kem"]);
        let l = phan_tich(&d, 3, &rows);
        assert!(l.lech());
        assert_eq!(l.thua.len(), 1);
        assert!(matches!(l.goi_y_gop, Some((1, 2))));
        assert_eq!(l.anh_xa.iter().flatten().count(), 3);
        let mut d2 = d.clone();
        gop_doan(&mut d2, 1);
        assert!(!phan_tich(&d2, 3, &rows).lech());
    }

    #[test]
    fn tach_doan_o_ranh_gioi_cau() {
        let mut d = ds(&["Câu một. Câu hai dài hơn. Câu ba."]);
        assert!(tach_doan(&mut d, 0, 10));
        assert_eq!(d, ds(&["Câu một. Câu hai dài hơn.", "Câu ba."]));
    }

    #[test]
    fn phien_ban_giu_ban_cu() {
        let t = std::env::temp_dir().join(format!("rg-thu-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        std::fs::create_dir_all(&t).unwrap();
        let f = t.join("a.md");
        std::fs::write(&f, "cu").unwrap();
        ghi_phien_ban(&t, "kich-ban", "ch001", &f, b"moi", "Thịnh", "gộp 2 đoạn").unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "moi");
        let v = ds_phien_ban(&t, "ch001");
        assert_eq!(v.len(), 1);
        assert_eq!(std::fs::read_to_string(&v[0].file).unwrap(), "cu");
        let _ = std::fs::remove_dir_all(&t);
    }
}

#[cfg(test)]
mod that {
    use super::*;

    /// Chạy tay trên bản đã chuyển: `RECAP_TEST_BO=… cargo test -p recap-gui -- --ignored ch064_gop`.
    #[test]
    #[ignore]
    fn ch064_gop() {
        let bo = PathBuf::from(std::env::var("RECAP_TEST_BO").unwrap());
        let mut c = Chuong::doc(&bo, "ch064");
        assert_eq!(c.loi().as_deref(), Some("15 đoạn cho 14 trang"));
        let (a, _) = c.lech.goi_y_gop.unwrap();
        gop_doan(&mut c.doan, a);
        c.tinh_lai_lech();
        assert!(c.loi().is_none());
        c.luu_kich_ban("Thịnh", "gộp 2 đoạn").unwrap();
        let c2 = Chuong::doc(&bo, "ch064");
        assert!(c2.loi().is_none());
        assert_eq!(ds_phien_ban(&bo, "ch064").len(), 1);
        // khung: kéo 1 hộp, lưu, đọc lại
        let mut c = c2;
        c.trang[1].khung[0].r[2] -= 20.0;
        let moi = c.trang[1].khung[0].r;
        c.luu_khung("Thịnh", "sửa khung").unwrap();
        let c3 = Chuong::doc(&bo, "ch064");
        assert_eq!(c3.trang[1].khung[0].r, moi);
        assert_eq!(ds_phien_ban(&bo, "ch064").len(), 2);
    }
}
