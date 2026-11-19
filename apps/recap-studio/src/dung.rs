//! Dựng 1 chương thành sequence FilmCraft bằng chính các lệnh của lõi (`timeline.*`, `effects.*`).
//! Công thức port từ `saas/editor/src/features/recap-bridge/` (truot-song.ts, manga-effects.ts, ten-rieng-hien.ts).
use crate::du_lieu::{Chuong, Kq, Trang};
use crate::ke_hoach::{KeHoach, TrangSong, cam_tai};
use filmcraft_engine::Session;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const TICK: f64 = 254_016_000_000.0; // ticks/giây của FilmCraft
pub const FW: f64 = 1920.0;
pub const FH: f64 = 1080.0;
pub const FPS: f64 = 30.0;
const KHUNG_NOI: f64 = 0.88; // manga-effects.ts KHUNG_NOI_TI_LE
const PHONG_CHAM: f64 = 1.07;
const NHOE_HE_SO: f64 = 0.6;
const BOX: f64 = 1.73; // hộp tương đương σ=amount/2 (xem ADR-0005)
const LIA_NHOE_DOC: f64 = (2.0 * 16.0 + 1.0) / 1920.0; // 2*(1920//120)+1 như truot-song.ts
const TRACK_VIDEO: usize = 6; // V1,V2 nền · V3,V4 bản lặp · V5 trang · V6 phụ
const MAX_O: usize = 7; // Slide/Fade: mỗi ô 1 track: V5, rồi V7..V12 (V6 dành cho tên riêng); nhiều ô hơn thì rơi về Tĩnh
const HIEN_O: f64 = 0.5; // REVEAL_DUR_SEC của manga-effects.ts

pub struct Tuy {
    pub cache: PathBuf,
    pub thu: usize,
    pub bitrate_kbps: Option<u32>,
    /// chỉ xuất đoạn [từ, đến] giây của sequence (thử/so khung nhanh)
    pub doan: Option<(f64, f64)>,
    /// `file.exportMedia` gpuRendering / hardwareEncoding: off | auto (None = mặc định của lõi)
    pub gpu: Option<String>,
    pub ma_hoa: Option<String>,
}

pub struct Dung {
    s: Session,
    chinh: Option<i64>,
    nhap: HashMap<PathBuf, i64>,
}

fn kq(r: Result<Value, impl ToString>) -> Kq<Value> {
    r.map_err(|e| e.to_string())
}

fn tk(frame: f64) -> i64 {
    (frame / FPS * TICK).round() as i64
}

fn fr(giay: f64) -> i64 {
    (giay * FPS).round() as i64
}

/// viewport chuẩn hoá [x,y,w,h] của ảnh W×H → (position tâm ảnh, scale%) để viewport lấp khung `fill`.
pub fn cam_viewport(vp: [f64; 4], w: f64, h: f64, fill: f64, max_scale: f64) -> ([f64; 2], f64) {
    let [x, y, vw, vh] = vp;
    let s = (FW / (vw * w)).min(FH / (vh * h)) * fill;
    let s = s.min(max_scale);
    let (cx, cy) = ((x + vw / 2.0) * w, (y + vh / 2.0) * h);
    ([round1(FW / 2.0 - (cx - w / 2.0) * s), round1(FH / 2.0 - (cy - h / 2.0) * s)], s * 100.0)
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Quét dọc (manga-effects.ts quetDocCameraPath): None nếu trang không dài hơn khung.
pub fn quet_doc(w: f64, h: f64) -> Option<[[f64; 4]; 2]> {
    let vh = (w / h) * (FH / FW);
    (vh < 0.98).then_some([[0.0, 0.0, 1.0, vh], [0.0, 1.0 - vh, 1.0, vh]])
}

impl Dung {
    pub fn new() -> Self {
        Self { s: Session::default(), chinh: None, nhap: HashMap::new() }
    }

    fn lenh(&mut self, id: &str, p: Value) -> Kq<Value> {
        let mut p = p;
        if id == "file.exportMedia"
            && let Some(o) = p.as_object_mut()
        {
            o.insert("wait".into(), json!(true));
        }
        kq(self.s.execute(id, p)).map_err(|e| format!("{id}: {e}"))
    }

    fn dat(&mut self, item: i64, track: &str, f0: i64, dur: i64) -> Kq<i64> {
        let r = self.lenh("timeline.place", json!({"item": item, "track": track, "seconds": f0 as f64 / FPS, "duration": {"seconds": dur as f64 / FPS}, "audio": false}))?;
        r["clips"][0].as_i64().ok_or_else(|| format!("timeline.place không trả clip: {r}"))
    }

    fn param(&mut self, clip: i64, eff: &Value, param: &str, val: Value, frame: Option<f64>) -> Kq<()> {
        let mut p = json!({"clip": clip, "effect": eff, "param": param, "value": val});
        if let (Some(f), Some(o)) = (frame, p.as_object_mut()) {
            o.insert("time".into(), json!(tk(f)));
            o.insert("keyframe".into(), json!(true));
        }
        self.lenh("effects.setParam", p).map(drop)
    }

    fn hoat_hoa(&mut self, clip: i64, eff: &Value, param: &str) -> Kq<()> {
        self.lenh("effects.toggleAnimation", json!({"clip": clip, "effect": eff, "param": param})).map(drop)
    }

    /// camera = keyframe Motion position/scale; `kf` = [(khung hình trong sequence, position, scale%)]
    fn camera(&mut self, clip: i64, kf: &[(f64, [f64; 2], f64)]) -> Kq<()> {
        let m = json!("motion");
        self.hoat_hoa(clip, &m, "position")?;
        self.hoat_hoa(clip, &m, "scale")?;
        for (f, pos, sc) in kf {
            self.param(clip, &m, "position", json!(pos), Some(*f))?;
            self.param(clip, &m, "scale", json!(sc), Some(*f))?;
        }
        Ok(())
    }

    /// Directional Blur: `kf` = [(khung hình, nửa độ dài hộp theo px ảnh gốc)]; `huong` 0 = dọc, 90 = ngang
    fn nhoe(&mut self, clip: i64, huong: f64, kf: &[(f64, f64)]) -> Kq<()> {
        let r = self.lenh("effects.apply", json!({"clips": [clip], "effect": "directional_blur"}))?;
        let idx = r["applied"][0]["index"].as_i64().ok_or_else(|| format!("effects.apply không trả index: {r}"))?;
        let e = json!(idx);
        self.param(clip, &e, "direction", json!(huong), None)?;
        self.hoat_hoa(clip, &e, "length")?;
        for (f, len) in kf {
            self.param(clip, &e, "length", json!((len * 1000.0).round() / 1000.0), Some(*f))?;
        }
        Ok(())
    }

    fn mo(&mut self, p: &Path) -> Kq<i64> {
        self.nhap.get(p).copied().ok_or_else(|| format!("chưa nhập {}", p.display()))
    }

    fn nhap_tat_ca(&mut self, paths: &[PathBuf]) -> Kq<()> {
        let r = self.lenh("file.import", json!({"paths": paths}))?;
        if let Some(e) = r["errors"].as_array().filter(|e| !e.is_empty()) {
            return Err(format!("nhập media lỗi: {e:?}"));
        }
        let ids = r["items"].as_array().ok_or("file.import không trả items")?;
        if ids.len() != paths.len() {
            return Err(format!("nhập {} file nhưng nhận {} item", paths.len(), ids.len()));
        }
        for (p, id) in paths.iter().zip(ids) {
            self.nhap.insert(p.clone(), id.as_i64().ok_or("id item không phải số")?);
        }
        Ok(())
    }
}

/// Nền mờ tính sẵn 1 ảnh nhỏ/trang vào bộ nhớ đệm (xoá được, tự tạo lại) — spec 08. Trả (đường dẫn, rộng, cao).
pub fn nen_mo(anh: &Path, cache: &Path) -> Kq<(PathBuf, f64, f64)> {
    use std::hash::{Hash, Hasher};
    let meta = std::fs::metadata(anh).map_err(|e| format!("{}: {e}", anh.display()))?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (anh, meta.len(), meta.modified().ok()).hash(&mut h);
    let out = cache.join(format!("nen-{:016x}.jpg", h.finish()));
    let img = image::open(anh).map_err(|e| format!("mở {}: {e}", anh.display()))?;
    let nho = img.resize(256, 256, image::imageops::FilterType::Triangle);
    let (w, hh) = (f64::from(nho.width()), f64::from(nho.height()));
    if !out.is_file() {
        std::fs::create_dir_all(cache).map_err(|e| e.to_string())?;
        // σ≈ bán kính 50px ở khung 1080 thu theo cỡ ảnh nhỏ; tối 0,55 như lớp phủ của app
        let mut m = image::imageops::blur(&nho.to_rgb8(), 3.5);
        for p in m.pixels_mut() {
            p.0 = p.0.map(|c| (f64::from(c) * 0.55) as u8);
        }
        m.save(&out).map_err(|e| format!("ghi nền mờ: {e}"))?;
    }
    Ok((out, w, hh))
}


/// Bóng đen mờ (alpha 0,85, mờ như `gpu-gaussian-blur` 50 của app cũ) đúng chỗ trang đã thu vào ô 88%: ảnh 192×108 tính sẵn
/// (1/10 khung) vào bộ nhớ đệm, đặt phóng 1000%. Trả đường dẫn; `rw`,`rh` = cỡ chỗ trang theo px khung 1080p.
pub fn bong_khung(rw: f64, rh: f64, cache: &Path) -> Kq<PathBuf> {
    let (rw, rh) = (rw.round().max(1.0) as u32, rh.round().max(1.0) as u32);
    let out = cache.join(format!("bong-{rw}x{rh}.png"));
    if !out.is_file() {
        std::fs::create_dir_all(cache).map_err(|e| e.to_string())?;
        let mut m = image::RgbaImage::new(192, 108);
        let (x0, y0) = ((192 - rw / 10).min(192) / 2, (108 - rh / 10).min(108) / 2);
        for y in y0..(y0 + rh / 10).min(108) {
            for x in x0..(x0 + rw / 10).min(192) {
                m.put_pixel(x, y, image::Rgba([0, 0, 0, 217]));
            }
        }
        image::imageops::blur(&m, 4.0).save(&out).map_err(|e| format!("ghi bóng khung: {e}"))?;
    }
    Ok(out)
}

pub struct KetQua {
    pub trang: usize,
    pub ten_rieng: usize,
    pub giay: f64,
    pub lenh_ms: u128,
    pub xuat_ms: Option<u128>,
}

impl Dung {
    /// Dựng cả chương vào 1 project/sequence; `ra` = (đường dẫn .fcproj, đường dẫn mp4 nếu xuất).
    pub fn dung_chuong(&mut self, ch: &Chuong, tuy: &Tuy, fcproj: &Path, mp4: Option<&Path>) -> Kq<KetQua> {
        let t0 = std::time::Instant::now();
        let ke: Option<KeHoach> = match &ch.ke_hoach {
            Some((v, _)) => Some(serde_json::from_value(v.clone()).map_err(|e| format!("kế hoạch trượt nhanh hỏng: {e}"))?),
            None => None,
        };
        let ke_dir = ch.ke_hoach.as_ref().map(|k| k.1.clone());
        let trang: Vec<&Trang> = ch.trang.iter().take(if tuy.thu > 0 { tuy.thu } else { usize::MAX }).collect();

        self.lenh("file.newProject", json!({"name": format!("{}-recap-studio", ch.ma)}))?;
        let seq = self.lenh("file.newSequence", json!({"name": ch.ma, "width": FW, "height": FH, "fps": FPS, "video": TRACK_VIDEO + MAX_O - 1, "audio": 1}))?;
        self.chinh = seq["sequence"].as_i64();

        // nhập mọi media 1 lần: giọng, ảnh trang, ảnh dải + nền của kế hoạch, nền mờ tính sẵn
        let mut files = vec![ch.voice.clone()];
        let mut nen_trang: HashMap<String, (PathBuf, f64, f64)> = HashMap::new();
        let mut bong_trang: HashMap<String, PathBuf> = HashMap::new();
        for t in &trang {
            if self.song_cua(t, &ke).is_some() {
                let s = self.song_cua(t, &ke).ok_or("kế hoạch mất trang")?;
                let d = ke_dir.as_ref().ok_or("thiếu thư mục kế hoạch")?;
                for f in std::iter::once(&s.anh).chain(std::iter::once(&s.nen)).chain(s.khung.iter().map(|k| &k.nen)) {
                    files.push(d.join(f));
                }
            } else {
                files.push(t.anh.clone());
                let n = nen_mo(&t.anh, &tuy.cache)?;
                files.push(n.0.clone());
                let (rw, rh) = Self::cho_trang(t);
                files.push(bong_khung(rw, rh, &tuy.cache)?);
                nen_trang.insert(t.ten.clone(), n);
                bong_trang.insert(t.ten.clone(), bong_khung(rw, rh, &tuy.cache)?);
            }
        }
        files.sort();
        files.dedup();
        self.nhap_tat_ca(&files)?;

        let voice = self.mo(&ch.voice)?;
        self.lenh("timeline.place", json!({"item": voice, "track": "A1", "seconds": 0, "video": false, "audio": true}))?;

        for t in &trang {
            if let (Some(s), Some(d)) = (self.song_cua(t, &ke).cloned(), ke_dir.as_ref()) {
                self.trang_truot_song(&s, d, ke.as_ref().map_or((FW, FH), |k| (k.w, k.h)))?;
            } else if let Some(n) = nen_trang.get(&t.ten).cloned() {
                let b = bong_trang.get(&t.ten).cloned().ok_or("thiếu bóng khung")?;
                self.trang_tinh(t, &n, &b)?;
            }
        }
        let n_ten = self.ten_rieng(ch)?;
        let lenh_ms = t0.elapsed().as_millis();

        self.lenh("file.saveAs", json!({"path": fcproj}))?;
        let xuat_ms = match mp4 {
            Some(p) => {
                let t1 = std::time::Instant::now();
                let mut e = json!({"path": p, "format": "h264", "audio": true});
                if let (Some(k), Some(o)) = (tuy.bitrate_kbps, e.as_object_mut()) {
                    o.insert("bitrateKbps".into(), json!(k));
                    o.insert("bitrateMode".into(), json!("vbr1Pass"));
                }
                if let Some(o) = e.as_object_mut() {
                    if let Some(g) = &tuy.gpu {
                        o.insert("gpuRendering".into(), json!(g));
                    }
                    if let Some(m) = &tuy.ma_hoa {
                        o.insert("hardwareEncoding".into(), json!(m));
                    }
                }
                if let (Some((x, y)), Some(o)) = (tuy.doan, e.as_object_mut()) {
                    o.insert("range".into(), json!("custom"));
                    o.insert("startSeconds".into(), json!(x));
                    o.insert("endSeconds".into(), json!(y));
                }
                self.lenh("file.exportMedia", e)?;
                Some(t1.elapsed().as_millis())
            }
            None => None,
        };
        Ok(KetQua { trang: trang.len(), ten_rieng: n_ten, giay: ch.tong, lenh_ms, xuat_ms })
    }

    fn song_cua<'a>(&self, t: &Trang, ke: &'a Option<KeHoach>) -> Option<&'a TrangSong> {
        if !matches!(t.kieu.as_str(), "truot-song" | "truot-nhanh") {
            return None;
        }
        ke.as_ref()?.trang.iter().find(|s| s.khoa == t.ten)
    }

    /// Kiểu thật của trang sau khi rơi về Tĩnh khi thiếu dữ liệu (quét dọc trang không dài, theo khung/Slide/Fade không có khung).
    fn che_do(t: &Trang) -> &'static str {
        let (w, h) = (f64::from(t.w), f64::from(t.h));
        match t.kieu.as_str() {
            "quet-doc" if quet_doc(w, h).is_some() => "quet-doc",
            "camera-path" if !t.khung.is_empty() => "camera-path",
            "tinh-phong" => "tinh-phong",
            "fade" | "slide" if !t.khung.is_empty() && t.khung.len() <= MAX_O => "reveal",
            _ => "tinh",
        }
    }

    /// Cỡ chỗ trang chiếm trong ô 88% (px khung 1080p) — để vẽ bóng: trang tĩnh = cỡ trang thu 88%, kiểu chuyển động phủ cả ô.
    pub fn cho_trang(t: &Trang) -> (f64, f64) {
        let (bw, bh) = (FW * KHUNG_NOI, FH * KHUNG_NOI);
        let (w, h) = (f64::from(t.w), f64::from(t.h));
        let fit = (FW / w).min(FH / h) * KHUNG_NOI;
        match Self::che_do(t) {
            "tinh" | "reveal" => ((w * fit).min(bw), (h * fit).min(bh)),
            _ => (bw, bh),
        }
    }

    /// Camera trong ô 88%: position/scale co về tâm ×0,88 và crop theo từng ~3 khung phần tràn ra ngoài ô (khungNoiItem).
    fn camera_noi(&mut self, clip: i64, kf: &[(f64, [f64; 2], f64)], w: f64, h: f64) -> Kq<()> {
        let co: Vec<(f64, [f64; 2], f64)> = kf
            .iter()
            .map(|(f, p, s)| (*f, [FW / 2.0 + (p[0] - FW / 2.0) * KHUNG_NOI, FH / 2.0 + (p[1] - FH / 2.0) * KHUNG_NOI], s * KHUNG_NOI))
            .collect();
        self.camera(clip, &co)?;
        // mốc crop: mọi mốc camera + (nếu cỡ đổi giữa 2 mốc) mỗi 3 khung, nội suy tuyến tính như lõi
        let mut mk: Vec<(f64, [f64; 2], f64)> = vec![];
        for (i, k) in co.iter().enumerate() {
            if i > 0 && (co[i - 1].2 - k.2).abs() > 1e-6 {
                let (a, b) = (co[i - 1], *k);
                let mut f = a.0 + 3.0;
                while f < b.0 {
                    let q = (f - a.0) / (b.0 - a.0);
                    mk.push((f, [a.1[0] + (b.1[0] - a.1[0]) * q, a.1[1] + (b.1[1] - a.1[1]) * q], a.2 + (b.2 - a.2) * q));
                    f += 3.0;
                }
            }
            mk.push(*k);
        }
        let r = self.lenh("effects.apply", json!({"clips": [clip], "effect": "crop"}))?;
        let idx = r["applied"][0]["index"].as_i64().ok_or_else(|| format!("effects.apply crop không trả index: {r}"))?;
        let e = json!(idx);
        let cs: Vec<(f64, [f64; 4])> = mk.iter().map(|(f, p, s)| (*f, crop_ngoai_o(*p, *s, w, h))).collect();
        for (j, ten) in ["left", "top", "right", "bottom"].into_iter().enumerate() {
            if cs.iter().all(|c| c.1[j] < 0.005) {
                continue;
            }
            self.hoat_hoa(clip, &e, ten)?;
            for (f, c) in &cs {
                self.param(clip, &e, ten, json!(c[j]), Some(*f))?;
            }
        }
        Ok(())
    }

    /// Cả trang / Tĩnh phóng / Quét dọc / Theo khung / Slide / Fade: nền mờ (V1) + bóng (V2) + ảnh trang thu vào 88% (V5).
    fn trang_tinh(&mut self, t: &Trang, nen: &(PathBuf, f64, f64), bong: &Path) -> Kq<()> {
        let a = fr(t.tu);
        let dur = (fr(t.den) - a).max(1);
        let (w, h) = (f64::from(t.w), f64::from(t.h));
        let bg = self.mo(&nen.0)?;
        let c = self.dat(bg, "V1", a, dur)?;
        self.param(c, &json!("motion"), "scale", json!((FW / nen.1).max(FH / nen.2) * 100.0), None)?;
        let sh = self.mo(bong)?;
        let c = self.dat(sh, "V2", a, dur)?;
        self.param(c, &json!("motion"), "scale", json!(1000.0), None)?;
        let item = self.mo(&t.anh)?;
        let fit = (FW / w).min(FH / h);
        let af = a as f64;
        let last = (dur - 1).max(1) as f64;
        let che = Self::che_do(t);
        if che == "reveal" {
            return self.hien_o(t, item, a, dur, fit);
        }
        let c = self.dat(item, "V5", a, dur)?;
        match che {
            "quet-doc" => {
                let [v0, v1] = quet_doc(w, h).ok_or("quet-doc")?;
                let (p0, s0) = cam_viewport(v0, w, h, 1.0, f64::MAX);
                let (p1, s1) = cam_viewport(v1, w, h, 1.0, f64::MAX);
                self.camera_noi(c, &[(af, p0, s0), (af + last, p1, s1)], w, h)?;
            }
            "camera-path" => {
                let kf = self.duong_cam(t, af, dur as f64)?;
                self.camera_noi(c, &kf, w, h)?;
            }
            "tinh-phong" => {
                let s = fit * 100.0;
                let p = [FW / 2.0, FH / 2.0];
                let n = (0.35 * FPS).round().min(last - 1.0).max(0.0);
                let mut kf = vec![(af, p, s * if n > 0.0 { 1.5 } else { 1.0 })];
                if n > 0.0 {
                    kf.push((af + n, p, s));
                }
                kf.push((af + last, p, s * PHONG_CHAM));
                self.camera_noi(c, &kf, w, h)?;
            }
            _ => self.param(c, &json!("motion"), "scale", json!(fit * KHUNG_NOI * 100.0), None)?,
        }
        Ok(())
    }

    /// Slide/Fade từng ô (manga-effects.ts revealFade/SlideKeyframes): mỗi khung 1 clip trang crop tĩnh đúng ô, hiện lần lượt
    /// (chia đều theo số khung — chưa có `project-path-*.json`), 0,5 s; Slide trượt từ phải (mặc định của app cũ).
    fn hien_o(&mut self, t: &Trang, item: i64, a: i64, dur: i64, fit: f64) -> Kq<()> {
        let n = t.khung.len() as i64;
        let (nf, last) = (fr(HIEN_O).max(1), (dur - 1).max(1));
        for (i, vp) in t.khung.iter().enumerate() {
            let st = (dur * i as i64 / n).min(last - 1).max(0);
            let c = self.dat(item, &format!("V{}", if i == 0 { 5 } else { 6 + i }), a + st, (dur - st).max(1))?;
            self.param(c, &json!("motion"), "scale", json!(fit * KHUNG_NOI * 100.0), None)?;
            let r = self.lenh("effects.apply", json!({"clips": [c], "effect": "crop"}))?;
            let idx = r["applied"][0]["index"].as_i64().ok_or_else(|| format!("effects.apply crop không trả index: {r}"))?;
            let e = json!(idx);
            let [x, y, vw, vh] = *vp;
            for (ten, v) in [("left", x), ("top", y), ("right", 1.0 - x - vw), ("bottom", 1.0 - y - vh)] {
                self.param(c, &e, ten, json!((v * 100.0).clamp(0.0, 100.0)), None)?;
            }
            let (f0, f1) = ((a + st) as f64, (a + st + nf).min(a + dur - 1) as f64);
            if t.kieu == "fade" {
                let o = json!("opacity");
                self.hoat_hoa(c, &o, "opacity")?;
                self.param(c, &o, "opacity", json!(0.0), Some(f0))?;
                self.param(c, &o, "opacity", json!(100.0), Some(f1))?;
            } else {
                let m = json!("motion");
                self.hoat_hoa(c, &m, "position")?;
                self.param(c, &m, "position", json!([FW / 2.0 + FW, FH / 2.0]), Some(f0))?;
                self.param(c, &m, "position", json!([FW / 2.0, FH / 2.0]), Some(f1))?;
            }
        }
        Ok(())
    }

    /// Theo khung: lia qua từng khung theo thứ tự đọc; trang không có khung thì đứng yên cả trang.
    fn duong_cam(&mut self, t: &Trang, af: f64, dur: f64) -> Kq<Vec<(f64, [f64; 2], f64)>> {
        let (w, h) = (f64::from(t.w), f64::from(t.h));
        let n = t.khung.len() as f64;
        let mv = 0.7 * FPS;
        let mut kf = vec![];
        for (j, vp) in t.khung.iter().enumerate() {
            let j = j as f64;
            let den = dur * j / n + if j == 0.0 { 0.0 } else { mv / 2.0 };
            let roi = (dur * (j + 1.0) / n - if j == n - 1.0 { 0.0 } else { mv / 2.0 }).min(dur - 1.0);
            let (p, s) = cam_viewport(*vp, w, h, 0.94, 2.2);
            kf.push((af + den.min(roi), p, s));
            kf.push((af + roi.max(den), p, s));
        }
        Ok(kf)
    }

    /// Trượt nhanh: port truot-song.ts (camera, nhoè dọc, hình lia, 2 bản lặp, nền theo khung xen kẽ 2 track).
    fn trang_truot_song(&mut self, tr: &TrangSong, dir: &Path, (kw, kh): (f64, f64)) -> Kq<()> {
        let sx = FW / kw;
        let _ = kh;
        let a = fr(tr.tu);
        let dur = (fr(tr.den) - a).max(1);
        // nền theo khung (nenTheoKhung)
        let ds: Vec<(f64, String, f64)> = if tr.khung.is_empty() {
            vec![(tr.tu, tr.nen.clone(), 0.0)]
        } else {
            tr.khung.iter().map(|k| (k.tu, k.nen.clone(), k.tron.unwrap_or(0.0))).collect()
        };
        let start: Vec<i64> = ds.iter().enumerate().map(|(j, x)| if j == 0 { 0 } else { fr(x.0 - tr.tu).max(1) }).collect();
        let tron: Vec<i64> = ds.iter().map(|x| fr(x.2).max(1)).collect();
        let keep = start.iter().filter(|s| **s < dur).count();
        for j in 0..keep {
            let nxt_end = if j + 1 < keep { start[j + 1] + tron[j + 1] } else { dur };
            let aft = if j + 2 < keep { start[j + 2] } else { dur };
            let end = (start[j] + 1).max(nxt_end.min(aft).min(dur));
            let track = j % 2;
            let mut ab: Vec<(i64, f64)> = vec![];
            if j >= 1 && track == 0 {
                ab.extend([(start[j], 0.0), (start[j] + tron[j], 1.0)]);
            }
            if j + 1 < keep && track == 0 {
                ab.extend([(start[j + 1], 1.0), (start[j + 1] + tron[j + 1], 0.0)]);
            }
            let item = self.mo(&dir.join(&ds[j].1))?;
            let c = self.dat(item, if track == 0 { "V2" } else { "V1" }, a + start[j], end - start[j])?;
            if !ab.is_empty() {
                let o = json!("opacity");
                self.hoat_hoa(c, &o, "opacity")?;
                let mut by: std::collections::BTreeMap<i64, f64> = Default::default();
                for (f, v) in ab {
                    by.insert(f.min(end - 1) - start[j], v);
                }
                for (f, v) in by {
                    self.param(c, &o, "opacity", json!(v * 100.0), Some((a + start[j] + f) as f64))?;
                }
            }
        }
        // dải trang nằm trong sequence lồng 1080p: Directional Blur chạy CPU trên ảnh nguồn của clip, mà dải trang cao tới
        // 7.484 px làm xuất chậm ~10× (đo 19/11). Nhoè gắn lên clip lồng (đúng cỡ khung 1080p) thì y hệt về hình, rẻ hơn nhiều.
        let item = self.mo(&dir.join(&tr.anh))?;
        let chinh = self.chinh.ok_or("chưa có sequence chính")?;
        let pg = self.lenh("file.newSequence", json!({"name": format!("trang {}", tr.khoa), "width": FW, "height": FH, "fps": FPS, "video": 3, "audio": 0}))?;
        let pg = pg["sequence"].as_i64().ok_or("file.newSequence không trả sequence")?;
        if let Some(l) = tr.lap {
            let ld = fr(l.dai).max(1).min(dur);
            for (i, lech) in [-1.0, -2.0].into_iter().enumerate() {
                let c = self.dat(item, if i == 0 { "V2" } else { "V1" }, 0, ld)?;
                let kf = self.cam_kf(tr, 0, l.tu, l.tu + ld as f64 / FPS, ld, lech, sx);
                self.camera(c, &kf)?;
            }
        }
        let c = self.dat(item, "V3", 0, dur)?;
        let kf = self.cam_kf(tr, 0, tr.tu, tr.tu + dur as f64 / FPS, dur, 0.0, sx);
        self.camera(c, &kf)?;
        self.lenh("sequence.open", json!({"item": chinh}))?;
        let c = self.dat(pg, "V5", a, dur)?;
        // nhoè dọc (nhoeKeyframes) và nhoè ngang 1 khung của hình lia (liaKeyframes); length = nửa hộp theo px khung hình
        let doc: std::collections::BTreeMap<i64, f64> = tr
            .nhoe
            .iter()
            .filter_map(|m| {
                let f = fr(m.t - tr.tu);
                (0..dur).contains(&f).then(|| (f, (NHOE_HE_SO * m.doc).min(0.3) * BOX * FH / 2.0))
            })
            .collect();
        if doc.values().any(|v| *v > 0.0) {
            let kf: Vec<(f64, f64)> = doc.iter().map(|(f, v)| ((a + f) as f64, *v)).collect();
            self.nhoe(c, 0.0, &kf)?;
        }
        if let Some(l) = tr.lia.filter(|l| fr(l.t - tr.tu).max(0) < dur) {
            let (f0, n) = (fr(l.t - tr.tu).max(0), fr(l.dai).max(1));
            let amt = (NHOE_HE_SO * LIA_NHOE_DOC).min(0.3) * BOX * FW / 2.0;
            let mut kf = vec![((a + f0) as f64, amt)];
            if f0 + n < dur {
                kf.push(((a + f0 + n) as f64, 0.0));
            }
            self.nhoe(c, 90.0, &kf)?;
        }
        Ok(())
    }

    /// camKeyframes: mốc đầu, mọi mốc camera giữa, mốc cuối; mốc trùng khung lấy mốc sau. Trả khung theo sequence.
    #[allow(clippy::too_many_arguments)]
    fn cam_kf(&self, tr: &TrangSong, a: i64, t0: f64, t1: f64, dur: i64, lech: f64, sx: f64) -> Vec<(f64, [f64; 2], f64)> {
        let mut by: std::collections::BTreeMap<i64, ([f64; 2], f64)> = Default::default();
        let ts = std::iter::once(t0).chain(tr.cam.iter().map(|c| c.t).filter(|t| *t > t0 && *t < t1)).chain(std::iter::once(t1));
        for t in ts {
            let c = cam_tai(&tr.cam, t);
            let buoc = tr.lap.map_or(0.0, |l| l.buoc * c.zoom * sx + l.ho * FW);
            let pos = [round2(FW / 2.0 + c.x * FW + lech * buoc), round2(FH / 2.0 + c.y * FH)];
            let f = (fr(t - t0)).clamp(0, dur - 1);
            by.insert(f, (pos, c.zoom * sx * 100.0));
        }
        by.into_iter().map(|(f, (p, s))| ((a + f) as f64, p, s)).collect()
    }

    /// Tên riêng hiện ở LẦN ĐẦU trong chương (ten-rieng-hien.ts): giờ = đầu câu + tỉ lệ ký tự × độ dài câu.
    fn ten_rieng(&mut self, ch: &Chuong) -> Kq<usize> {
        let hien = chon_ten_hien(ch, &ch.ten_rieng);
        let n = hien.len();
        // quy ra khung hình: tên gần nhau < 1 khung vẫn không chồng clip (track "not free"), mỗi tên ≥ 1 khung
        let mut bd: Vec<i64> = vec![];
        for h in &hien {
            let s = fr(h.1).max(bd.last().map_or(0, |p| p + 1));
            bd.push(s);
        }
        let hien: Vec<(String, f64, f64)> = hien
            .into_iter()
            .enumerate()
            .map(|(i, (ten, _, dai))| {
                let d = fr(dai).max(1).min(bd.get(i + 1).map_or(i64::MAX, |nx| nx - bd[i]));
                (ten, bd[i] as f64 / FPS, d as f64 / FPS)
            })
            .collect();
        for (ten, t, dai) in hien {
            let r = self
                .lenh("graphics.newText", json!({"text": ten, "size": 58, "track": 5, "seconds": dai, "time": tk(t * FPS), "box": [FW * 0.82, FH * 0.09], "position": [FW * 0.09, FH * 0.875]}))
                .map_err(|e| format!("tên riêng '{ten}': {e}"))?;
            let clip = r["clip"].as_i64().ok_or("graphics.newText không trả clip")?;
            // ten-rieng-hien.ts: chữ trắng đậm, viền đen ~0,6% cao khung, giữa
            let props = json!({"font_style": "Bold", "align": "center", "fill_color": "#ffffff", "stroke": true, "stroke_color": "#000000", "stroke_width": 6});
            self.lenh("graphics.set", json!({"clip": clip, "props": props})).map_err(|e| format!("tên riêng '{ten}': {e}"))?;
        }
        Ok(n)
    }
}

/// phần ảnh (%) nằm ngoài ô 88% khi tâm ảnh ở `p`, cỡ `scale`% — [trái, trên, phải, dưới]
fn crop_ngoai_o(p: [f64; 2], scale: f64, w: f64, h: f64) -> [f64; 4] {
    let (sw, sh) = (w * scale / 100.0, h * scale / 100.0);
    let (bx, by) = (FW * (1.0 - KHUNG_NOI) / 2.0, FH * (1.0 - KHUNG_NOI) / 2.0);
    let c = |v: f64, d: f64| (v / d * 100.0).clamp(0.0, 100.0);
    [
        c(bx - (p[0] - sw / 2.0), sw),
        c(by - (p[1] - sh / 2.0), sh),
        c((p[0] + sw / 2.0) - (FW - bx), sw),
        c((p[1] + sh / 2.0) - (FH - by), sh),
    ]
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn la_chu(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric())
}

/// Tên đúng như viết trong kịch bản, lần đầu mỗi chương; tên dài khớp trước, nguyên từ, phân biệt hoa thường.
pub fn chon_ten_hien(ch: &Chuong, names: &[String]) -> Vec<(String, f64, f64)> {
    let mut uniq: Vec<&String> = names.iter().filter(|n| n.trim().chars().count() >= 2).collect();
    uniq.sort_by_key(|n| std::cmp::Reverse(n.chars().count()));
    uniq.dedup();
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<(String, f64, f64)> = vec![];
    for c in &ch.cau {
        let chars: Vec<char> = c.text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let hit = uniq.iter().find(|n| {
                let nc: Vec<char> = n.chars().collect();
                chars[i..].starts_with(&nc) && !la_chu(i.checked_sub(1).map(|k| chars[k])) && !la_chu(chars.get(i + nc.len()).copied())
            });
            if let Some(n) = hit {
                if seen.insert((*n).clone()) {
                    let t = c.bat_dau + (i as f64 / chars.len().max(1) as f64) * c.dai;
                    let con_lai = c.bat_dau + c.dai - t;
                    out.push(((*n).clone(), t, (con_lai + 1.0).clamp(1.0 / FPS, 2.5)));
                }
                i += n.chars().count();
            } else {
                i += 1;
            }
        }
    }
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    for i in 0..out.len().saturating_sub(1) {
        let cap = out[i + 1].1 - out[i].1;
        if cap > 1.0 / FPS {
            out[i].2 = out[i].2.min(cap);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(cau: &[(&str, f64, f64)]) -> Chuong {
        Chuong {
            ma: "ch001".into(),
            voice: PathBuf::new(),
            trang: vec![],
            cau: cau.iter().map(|(t, b, d)| crate::du_lieu::Cau { text: (*t).into(), bat_dau: *b, dai: *d }).collect(),
            tong: 10.0,
            ke_hoach: None,
            ten_rieng: vec![],
        }
    }

    #[test]
    fn ten_rieng_lan_dau_nguyen_tu_ten_dai_truoc() {
        let c = ch(&[("Aiba Izou gặp Aibanh rồi Aiba", 0.0, 4.0), ("Izou nói Aiba Izou", 5.0, 3.0)]);
        let names: Vec<String> = ["Aiba", "Aiba Izou", "Izou"].iter().map(|s| (*s).to_string()).collect();
        let r = chon_ten_hien(&c, &names);
        let ten: Vec<&str> = r.iter().map(|x| x.0.as_str()).collect();
        // "Aiba Izou" khớp trước "Aiba"; "Aibanh" không khớp; "Aiba" (cuối câu 1) là lần đầu của "Aiba"; "Izou" lần đầu ở câu 2
        assert_eq!(ten, ["Aiba Izou", "Aiba", "Izou"]);
        assert!(r[0].1 < r[1].1 && r[1].1 < r[2].1);
    }

    #[test]
    fn crop_cat_phan_tran_ra_ngoai_o() {
        // ảnh 1000×3000 phủ ngang ô 88%: trái/phải 0, trên (lúc ở đỉnh) = phần dưới tràn
        let (p, s) = cam_viewport(quet_doc(1000.0, 3000.0).unwrap()[0], 1000.0, 3000.0, 1.0, f64::MAX);
        let p = [FW / 2.0 + (p[0] - FW / 2.0) * KHUNG_NOI, FH / 2.0 + (p[1] - FH / 2.0) * KHUNG_NOI];
        let c = crop_ngoai_o(p, s * KHUNG_NOI, 1000.0, 3000.0);
        assert!(c[0] < 0.01 && c[1] < 0.01 && c[2] < 0.01, "{c:?}");
        // phần hiện = viewport (vh ≈ 0,316 của ảnh) nên crop dưới ≈ 68,4%
        assert!((c[3] - (100.0 - 100.0 * (1000.0 / 3000.0) * (FH / FW))).abs() < 0.1, "{c:?}");
    }

    #[test]
    fn quet_doc_chi_khi_trang_dai() {
        assert!(quet_doc(1000.0, 3000.0).is_some());
        assert!(quet_doc(1600.0, 900.0).is_none());
        let [a, b] = quet_doc(1000.0, 3000.0).unwrap();
        assert_eq!(a[1], 0.0);
        assert!((b[1] + b[3] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cam_viewport_dua_khung_ve_giua() {
        // cả ảnh 1920×1080 fill 1 → scale 100%, tâm ở giữa
        let (p, s) = cam_viewport([0.0, 0.0, 1.0, 1.0], 1920.0, 1080.0, 1.0, 9.0);
        assert_eq!(p, [960.0, 540.0]);
        assert!((s - 100.0).abs() < 1e-9);
    }
}
