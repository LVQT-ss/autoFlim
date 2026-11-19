//! Màn 3 — Xem / sửa 1 chương (spec `03-xem-chuong.md`): dải trang · trang lớn (khung, che chữ, chữ đọc được) ·
//! lời kể + thoại gốc + giọng + đánh dấu. Mọi sửa đều thành PHIÊN BẢN MỚI (`chuong.rs`), không ghi đè.
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Ui, Vec2};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use crate::chuong::{self, Chuong, Hop};
use crate::giao_dien::*;
use crate::{App, Man};

const KIEU: [(&str, &str); 7] = [("", "Mặc định của bộ"), ("quet-doc", "Trượt dọc"), ("truot-song", "Trượt nhanh"), ("tinh", "Cả trang"), ("tinh-phong", "Tĩnh phóng"), ("camera-path", "Theo khung"), ("slide", "Slide")];
const THUMB_CAO: u32 = 150;
const LON_CAO: u32 = 1700;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lop {
    Khung,
    Che,
}

enum Nghe {
    Rong,
    Dang(std::time::Instant),
    Phat { giay: f64, mat: f64 },
    Loi(String),
}

struct Keo {
    idx: usize,
    cach: u8, // bit 1=trái 2=phải 4=trên 8=dưới 16=cả hộp
    r0: [f32; 4],
    p0: Pos2,
}

type AnhRa = ((PathBuf, u32), Option<egui::ColorImage>);

pub struct ManXem {
    pub bo: PathBuf,
    pub ma: String,
    pub ch: Chuong,
    pub trang: usize,
    pub tim: String,
    lop_sua: Lop,
    hien_ocr: bool,
    chon: Option<usize>,
    keo: Option<Keo>,
    zoom: f32,
    pan: Vec2,
    anh: HashMap<(PathBuf, u32), Option<TextureHandle>>,
    tx: mpsc::Sender<(PathBuf, u32)>,
    rx: mpsc::Receiver<AnhRa>,
    pub lich_su: Option<Vec<chuong::PhienBan>>,
    con_tro: usize,
    nghe: Arc<Mutex<Nghe>>,
    cach_doc: BTreeMap<String, String>,
    ghi_chu_sua: bool,
}

fn chay_tai_anh(ctx: egui::Context) -> (mpsc::Sender<(PathBuf, u32)>, mpsc::Receiver<AnhRa>) {
    let (tx, rx_y) = mpsc::channel::<(PathBuf, u32)>();
    let (tx_r, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for (p, cao) in rx_y {
            let a = image::open(&p).ok().map(|a| {
                let t = if a.height() > cao { a.resize(u32::MAX, cao, image::imageops::FilterType::Triangle) } else { a }.to_rgba8();
                egui::ColorImage::from_rgba_unmultiplied([t.width() as usize, t.height() as usize], t.as_raw())
            });
            if tx_r.send(((p, cao), a)).is_err() {
                return;
            }
            ctx.request_repaint();
        }
    });
    (tx, rx)
}

impl ManXem {
    pub fn moi(ctx: &egui::Context, bo: &Path, ma: &str, trang: usize) -> Self {
        let (tx, rx) = chay_tai_anh(ctx.clone());
        let ch = Chuong::doc(bo, ma);
        let t = trang.min(ch.trang.len().saturating_sub(1));
        let mut m = Self { bo: bo.to_path_buf(), ma: ma.to_owned(), ch, trang: t, tim: String::new(), lop_sua: Lop::Khung, hien_ocr: false, chon: None, keo: None, zoom: 1.0, pan: Vec2::ZERO, anh: HashMap::new(), tx, rx, lich_su: None, con_tro: 0, nghe: Arc::new(Mutex::new(Nghe::Rong)), cach_doc: BTreeMap::new(), ghi_chu_sua: false };
        m.trang_dau_tien_loi();
        m
    }

    fn trang_dau_tien_loi(&mut self) {
        if let Some(j) = self.ch.lech.thieu.first().or(self.ch.lech.anh_xa.iter().position(Option::is_none).as_ref()) {
            self.trang = (*j).min(self.ch.trang.len().saturating_sub(1));
        } else if let Some(d) = self.ch.lech.goi_y_gop {
            self.trang = self.ch.lech.anh_xa.iter().position(|a| *a == Some(d.0)).unwrap_or(0);
        }
    }

    fn lay_anh(&mut self, p: &Path, cao: u32) -> Option<egui::TextureId> {
        let k = (p.to_path_buf(), cao);
        match self.anh.get(&k) {
            Some(Some(t)) => Some(t.id()),
            Some(None) => None,
            None => {
                self.anh.insert(k.clone(), None);
                let _ = self.tx.send(k);
                None
            }
        }
    }

    fn nhan_anh(&mut self, ctx: &egui::Context) {
        while let Ok((k, a)) = self.rx.try_recv() {
            match a {
                Some(a) => {
                    let t = ctx.load_texture(format!("{}@{}", k.0.display(), k.1), a, Default::default());
                    self.anh.insert(k, Some(t));
                }
                None => {
                    self.anh.remove(&k);
                    self.anh.insert((k.0, u32::MAX), None); // đánh dấu hỏng
                }
            }
        }
        // chỉ giữ ảnh lớn của trang đang xem ±2; ảnh nhỏ giữ hết
        let giu: Vec<PathBuf> = (self.trang.saturating_sub(2)..=(self.trang + 2).min(self.ch.trang.len().saturating_sub(1))).filter_map(|i| self.ch.trang.get(i).and_then(|t| t.file.clone())).collect();
        self.anh.retain(|(p, c), _| *c != LON_CAO || giu.contains(p));
    }
}

fn mau_cham(t: &chuong::TrangDl, co_doan: bool, co_khung_file: bool) -> Color32 {
    if t.dd.bo {
        XAM
    } else if !co_doan || (co_khung_file && t.khung.is_empty()) {
        DO
    } else if t.dd.nghi_lech {
        VANG
    } else {
        XANH
    }
}

impl App {
    pub fn mo_chuong(&mut self, ctx: &egui::Context, bo: &Path, ma: &str, trang: usize) {
        self.xem = Some(Box::new(ManXem::moi(ctx, bo, ma, trang)));
        self.man = Man::Chuong(bo.to_path_buf(), ma.to_owned());
    }

    /// Việc đang xếp/chạy của chính chương này (khoá sửa lời/khung để 2 bên không ghi đè nhau).
    fn viec_dang_lam(&self, bo: &Path, ma: &str) -> Option<String> {
        let b = std::fs::canonicalize(bo).unwrap_or_else(|_| bo.to_path_buf());
        self.snap.viec.iter().find(|v| v.chuong.as_deref() == Some(ma) && matches!(v.trang_thai.as_str(), "chay" | "cho" | "cat_quota") && Path::new(&v.bo) == b && matches!(v.loai.as_str(), "agy" | "do_khung" | "xoa_chu" | "ocr")).map(|v| {
            let ten = match (v.loai.as_str(), v.kieu.as_str()) {
                ("agy", "viet_lai") => "viết lại chương",
                ("agy", _) => "việc agy",
                ("do_khung", _) => "dò lại khung",
                ("xoa_chu", _) => "xoá chữ",
                _ => "đọc lại chữ",
            };
            if v.trang_thai == "chay" { format!("Đang {ten}…") } else { format!("Đang chờ {ten}") }
        })
    }

    pub fn man_chuong(&mut self, ui: &mut Ui) {
        let Some(mut m) = self.xem.take() else {
            self.man = Man::Thu;
            return;
        };
        m.nhan_anh(&ui.ctx().clone());
        let khoa = self.viec_dang_lam(&m.bo, &m.ma);
        let mut doi: Option<String> = None;
        egui::Panel::top("chuong_tren").show_separator_line(false).frame(egui::Frame::new().fill(MAT_1).inner_margin(egui::Margin::symmetric(16, 8))).show(ui, |ui| doi = self.thanh_chuong(ui, &mut m, khoa.as_deref()));
        egui::Panel::left("chuong_dai").exact_size(132.0).resizable(false).frame(egui::Frame::new().fill(MAT_1).inner_margin(egui::Margin::same(8))).show(ui, |ui| self.dai_trang(ui, &mut m));
        egui::Panel::right("chuong_loi").exact_size(400.0).resizable(false).frame(egui::Frame::new().fill(MAT_1).inner_margin(egui::Margin::same(12))).show(ui, |ui| self.cot_loi(ui, &mut m, khoa.as_deref()));
        egui::CentralPanel::default().frame(egui::Frame::new().fill(NEN).inner_margin(egui::Margin::same(8))).show(ui, |ui| self.trang_lon(ui, &mut m, khoa.is_some()));
        self.hop_lich_su(ui.ctx(), &mut m);
        if let Some(ch) = doi {
            if ch == "<bo>" {
                self.man = Man::Bo(m.bo.clone());
            } else if ch == "<tl>" {
                self.mo_timeline(ui.ctx(), &m.bo.clone(), Some(&m.ma.clone()));
            } else if m.ch.kb_bien_doi() || m.ch.khung_bien_doi() || m.ch.che_bien_doi() {
                self.tb("Còn thay đổi chưa lưu — lưu hoặc hoàn tác trước khi đổi chương");
            } else {
                let (bo, ctx) = (m.bo.clone(), ui.ctx().clone());
                *m = ManXem::moi(&ctx, &bo, &ch, 0);
                self.man = Man::Chuong(bo, ch);
            }
        }
        if matches!(self.man, Man::Chuong(..)) {
            self.xem = Some(m);
        }
    }

    fn ds_chuong(&self, bo: &Path) -> Vec<String> {
        self.snap.bo.iter().find(|b| b.thu_muc == bo).map(|b| b.buoc[0].dong.iter().map(|d| d.ten.clone()).filter(|t| t.starts_with("ch")).collect()).unwrap_or_default()
    }

    fn thanh_chuong(&mut self, ui: &mut Ui, m: &mut ManXem, khoa: Option<&str>) -> Option<String> {
        let mut doi = None;
        let ds = self.ds_chuong(&m.bo);
        let vi = ds.iter().position(|c| *c == m.ma);
        let ten_bo = self.snap.bo.iter().find(|b| b.thu_muc == m.bo).map_or_else(|| chuong::ten_file(&m.bo), |b| b.ten.clone());
        ui.horizontal_wrapped(|ui| {
            if nut(ui, "← Bảng điều khiển", false, true).clicked() {
                doi = Some("<bo>".to_owned());
            }
            ui.label(egui::RichText::new(ten_bo).size(15.0).strong());
            ui.separator();
            if nut(ui, "◀", false, vi.is_some_and(|i| i > 0)).clicked() {
                doi = vi.and_then(|i| ds.get(i - 1).cloned());
            }
            egui::ComboBox::from_id_salt("chon_chuong").selected_text(egui::RichText::new(&m.ma).strong()).width(110.0).show_ui(ui, |ui| {
                ui.add(egui::TextEdit::singleline(&mut m.tim).hint_text("Tìm chương…").desired_width(90.0));
                let q = m.tim.trim().to_lowercase();
                for c in ds.iter().filter(|c| q.is_empty() || c.contains(&q)) {
                    if ui.selectable_label(*c == m.ma, c).clicked() {
                        doi = Some(c.clone());
                    }
                }
            });
            if nut(ui, "▶", false, vi.is_some_and(|i| i + 1 < ds.len())).clicked() {
                doi = vi.and_then(|i| ds.get(i + 1).cloned());
            }
            ui.separator();
            match m.ch.loi() {
                None => ui.colored_label(XANH, "● Đạt"),
                Some(l) => ui.colored_label(DO, format!("● Lỗi: {l}")),
            };
            if let Some(k) = khoa {
                ui.colored_label(VANG, format!("· {k}"));
            }
            if let Some(t) = m.ch.tre.clone() {
                ui.colored_label(VANG, format!("· {t}"));
                if nut(ui, "Cập nhật chương trên timeline", false, true).on_hover_text("Thay đúng đoạn chương trên dải phần; chương đã sửa tay thì giữ nguyên").clicked() {
                    self.cap_nhat_timeline(&m.bo.clone(), &m.ma.clone());
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("⋯", |ui| {
                    if ui.button("Viết lại chương bằng agy").on_hover_text("agy viết lại đúng chương này, giữ bản cũ; trang Thịnh đã sửa tay được giữ").clicked() {
                        let giu: Vec<String> = m.ch.trang.iter().enumerate().filter(|(_, t)| t.dd.ghi_chu.is_empty() && !t.dd.nghi_lech).filter_map(|(i, _)| m.ch.doan_trang(i).filter(|d| m.ch.doan[*d] != m.ch.doan_luu.get(*d).cloned().unwrap_or_default()).map(|_| format!("{}", i + 1))).collect();
                        let ghi_chu: Vec<String> = m.ch.trang.iter().filter(|t| !t.dd.ghi_chu.is_empty()).map(|t| format!("{}: {}", t.ten, t.dd.ghi_chu)).collect();
                        let r = crate::lenh::them_viec(&self.duong.goc_dv, "agy", &m.bo, Some(&m.ma), serde_json::json!({"kieu": "viet_lai", "giu_trang": giu, "ghi_chu": ghi_chu, "kiem": "kich_ban"})).map(|_| ());
                        self.gui(r, "Đã xếp việc viết lại chương vào hàng");
                        ui.close();
                    }
                    if ui.button("Dò lại khung").clicked() {
                        let r = crate::lenh::them_viec(&self.duong.goc_dv, "do_khung", &m.bo, Some(&m.ma), serde_json::json!({})).map(|_| ());
                        self.gui(r, "Đã xếp việc dò lại khung vào hàng");
                        ui.close();
                    }
                    if ui.button("Đọc lại chữ").clicked() {
                        let r = crate::lenh::them_viec(&self.duong.goc_dv, "ocr", &m.bo, Some(&m.ma), serde_json::json!({})).map(|_| ());
                        self.gui(r, "Đã xếp việc đọc lại chữ vào hàng");
                        ui.close();
                    }
                    if ui.button("Lịch sử phiên bản").clicked() {
                        m.lich_su = Some(chuong::ds_phien_ban(&m.bo, &m.ma));
                        ui.close();
                    }
                });
                if nut(ui, "Timeline", false, true).clicked() {
                    doi = Some("<tl>".to_owned());
                }
                if nut(ui, "Xem trước video chương", true, true).on_hover_text("Dựng nhanh chương này ra video tạm (không tạo/đổi timeline thật) rồi mở bằng trình phát của máy").clicked() {
                    self.xem_truoc(&m.bo.clone(), &m.ma.clone());
                }
            });
        });
        doi
    }

    fn dai_trang(&mut self, ui: &mut Ui, m: &mut ManXem) {
        ui.label(egui::RichText::new(format!("{} trang", m.ch.trang.len())).size(12.0).color(CHU_PHU));
        let co_khung = m.ch.khung_json.is_some();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for i in 0..m.ch.trang.len() {
                let co_doan = m.ch.doan_trang(i).is_some();
                let (cham, file, ten) = {
                    let t = &m.ch.trang[i];
                    (mau_cham(t, co_doan, co_khung), t.file.clone(), t.ten.clone())
                };
                let (rect, r) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 132.0), Sense::click());
                ui.painter().rect_filled(rect, 6.0, if r.hovered() { MAT_3 } else { MAT_2 });
                if i == m.trang {
                    ui.painter().rect_stroke(rect, 6.0, Stroke::new(2.0, NHAN), StrokeKind::Inside);
                }
                let anh_rect = Rect::from_min_size(rect.min + Vec2::new(6.0, 6.0), Vec2::new(rect.width() - 12.0, 102.0));
                if ui.is_rect_visible(rect) {
                    if let Some(t) = file.as_ref().and_then(|f| m.lay_anh(f, THUMB_CAO)) {
                        let sz = m.anh.iter().find(|(k, _)| Some(&k.0) == file.as_ref() && k.1 == THUMB_CAO).and_then(|(_, v)| v.as_ref().map(TextureHandle::size_vec2)).unwrap_or(Vec2::new(1.0, 1.4));
                        let k = (anh_rect.width() / sz.x).min(anh_rect.height() / sz.y);
                        let ve = Rect::from_center_size(anh_rect.center(), sz * k);
                        ui.painter().image(t, ve, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), if m.ch.trang[i].dd.bo { Color32::from_gray(90) } else { Color32::WHITE });
                    }
                }
                ui.painter().circle_filled(rect.min + Vec2::new(14.0, 120.0), 4.0, cham);
                ve_chu(ui.painter(), rect.min + Vec2::new(24.0, 113.0), &format!("{} {}", i + 1, if m.ch.trang[i].dd.bo { "· bỏ" } else { "" }), 12.0, CHU_PHU, rect.width() - 30.0);
                if r.clicked() {
                    m.trang = i;
                    m.chon = None;
                    m.pan = Vec2::ZERO;
                }
                r.on_hover_text(ten);
            }
        });
    }

    fn trang_lon(&mut self, ui: &mut Ui, m: &mut ManXem, khoa: bool) {
        let n = m.ch.trang.len();
        if n == 0 {
            ui.vertical_centered(|ui| {
                ui.add_space(120.0);
                ui.label(egui::RichText::new("Chương này chưa có trang").size(18.0));
                ui.label(egui::RichText::new(format!("Không thấy ảnh trong nguon/{}/. Chạy bước Nhập ảnh hoặc kiểm bản chuyển.", m.ma)).color(CHU_PHU));
            });
            return;
        }
        if !ui.ctx().egui_wants_keyboard_input() {
            let (tr, ph) = ui.input(|i| (i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight)));
            if tr && m.trang > 0 {
                m.trang -= 1;
                m.chon = None;
            }
            if ph && m.trang + 1 < n {
                m.trang += 1;
                m.chon = None;
            }
        }
        // thanh công cụ của trang
        ui.horizontal_wrapped(|ui| {
            let (khung_n, che_n, ocr_n) = { let t = &m.ch.trang[m.trang]; (t.khung.len(), t.che.len(), t.ocr.len()) };
            ui.checkbox(&mut m.ch.khung_bat, format!("Khung ({khung_n})"));
            ui.checkbox(&mut m.ch.che_bat, format!("Che chữ ({che_n})"));
            ui.add_enabled(m.ch.co_ocr, egui::Checkbox::new(&mut m.hien_ocr, format!("Chữ đọc được ({ocr_n})"))).on_disabled_hover_text("Chương chưa có ocr/ — chạy bước Đọc chữ");
            ui.separator();
            ui.label(egui::RichText::new("Đang sửa:").color(CHU_PHU));
            ui.selectable_value(&mut m.lop_sua, Lop::Khung, "Khung");
            ui.selectable_value(&mut m.lop_sua, Lop::Che, "Che chữ");
            ui.separator();
            if nut(ui, "+ Thêm", false, !khoa).clicked() {
                let (w, h) = { let t = &m.ch.trang[m.trang]; (t.w, t.h) };
                let hop = Hop { r: [w * 0.25, h * 0.25, w * 0.75, h * 0.75], giu: false, chu: String::new(), goc: None };
                let t = &mut m.ch.trang[m.trang];
                let v = if m.lop_sua == Lop::Khung { &mut t.khung } else { &mut t.che };
                v.push(hop);
                m.chon = Some(v.len() - 1);
            }
            let co_chon = m.chon.is_some();
            if nut(ui, "Xoá hộp", false, co_chon && !khoa).clicked() {
                if let Some(i) = m.chon.take() {
                    let t = &mut m.ch.trang[m.trang];
                    let v = if m.lop_sua == Lop::Khung { &mut t.khung } else { &mut t.che };
                    if i < v.len() {
                        v.remove(i);
                    }
                }
            }
            if m.lop_sua == Lop::Che && nut(ui, "Giữ chữ này", false, co_chon && !khoa).on_hover_text("Không che hộp này (vd tiếng động vẽ tay)").clicked() {
                if let Some(h) = m.chon.and_then(|i| m.ch.trang[m.trang].che.get_mut(i)) {
                    h.giu = !h.giu;
                }
            }
            let (kb, ch) = (m.ch.khung_bien_doi(), m.ch.che_bien_doi());
            if nut(ui, "Lưu khung", true, kb && !khoa).clicked() {
                let r = m.ch.luu_khung("Thịnh", "sửa khung");
                self.gui(r, "Đã lưu khung (bản cũ còn trong lịch sử)");
            }
            if nut(ui, "Lưu che chữ", true, ch && !khoa).clicked() {
                let r = m.ch.luu_che("Thịnh", "sửa che chữ");
                self.gui(r, "Đã lưu lớp che chữ (bản cũ còn trong lịch sử)");
            }
            if nut(ui, "Hoàn tác", false, kb || ch).clicked() {
                let (bo, ma, t) = (m.bo.clone(), m.ma.clone(), m.trang);
                let (doan, doan_luu) = (m.ch.doan.clone(), m.ch.doan_luu.clone());
                m.ch = Chuong::doc(&bo, &ma);
                m.ch.doan = doan;
                m.ch.doan_luu = doan_luu;
                m.ch.tinh_lai_lech();
                m.trang = t;
                m.chon = None;
            }
            ui.separator();
            if nut(ui, "−", false, true).clicked() {
                m.zoom = (m.zoom / 1.25).max(0.3);
            }
            ui.label(format!("{:.0}%", m.zoom * 100.0));
            if nut(ui, "+", false, true).clicked() {
                m.zoom = (m.zoom * 1.25).min(6.0);
            }
            if nut(ui, "Vừa khung", false, true).clicked() {
                m.zoom = 1.0;
                m.pan = Vec2::ZERO;
            }
        });
        if m.lop_sua == Lop::Che && m.ch.che_json.is_none() && m.ch.trang.iter().all(|t| t.che.is_empty()) {
            ui.label(egui::RichText::new("Chương này chưa có lớp che chữ (bước Xoá chữ chưa chạy). Có thể tự vẽ hộp che rồi bấm Lưu che chữ.").size(12.0).color(CHU_MO));
        }
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        painter.rect_filled(resp.rect, 6.0, Color32::from_rgb(0x0a, 0x0c, 0x10));
        let (file, w, h) = { let t = &m.ch.trang[m.trang]; (t.file.clone(), t.w.max(1.0), t.h.max(1.0)) };
        let ve = |m: &ManXem| {
            let k = (resp.rect.width() / w).min(resp.rect.height() / h) * m.zoom;
            Rect::from_center_size(resp.rect.center() + m.pan, Vec2::new(w * k, h * k))
        };
        if resp.hovered() {
            let z = ui.input(|i| i.zoom_delta());
            if (z - 1.0).abs() > 1e-3 {
                m.zoom = (m.zoom * z).clamp(0.3, 6.0);
            } else {
                let s = ui.input(|i| i.smooth_scroll_delta);
                m.pan += s;
            }
        }
        // trang kề được giải mã sẵn (đổi trang < 100 ms)
        for d in [m.trang.saturating_sub(1), (m.trang + 1).min(n - 1)] {
            if let Some(f) = m.ch.trang[d].file.clone() {
                let _ = m.lay_anh(&f, LON_CAO);
            }
        }
        let rect = ve(m);
        match file.as_ref().and_then(|f| m.lay_anh(f, LON_CAO)) {
            Some(t) => {
                painter.image(t, rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            None => {
                let hong = file.as_ref().is_none_or(|f| m.anh.contains_key(&(f.clone(), u32::MAX)));
                chu_giua(&painter, resp.rect.center(), if hong { "Không đọc được ảnh trang này" } else { "Đang đọc ảnh…" }, 14.0, CHU_PHU);
            }
        }
        let qua = |r: [f32; 4]| Rect::from_min_max(rect.min + Vec2::new(r[0] / w * rect.width(), r[1] / h * rect.height()), rect.min + Vec2::new(r[2] / w * rect.width(), r[3] / h * rect.height()));
        let t = &m.ch.trang[m.trang];
        if m.ch.che_bat {
            for (i, hp) in t.che.iter().enumerate() {
                let r = qua(hp.r);
                let sua = m.lop_sua == Lop::Che;
                if !hp.giu {
                    painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(240, 93, 93, if sua { 40 } else { 22 }));
                }
                painter.rect_stroke(r, 0.0, Stroke::new(if sua && m.chon == Some(i) { 2.0 } else { 1.0 }, if hp.giu { XANH } else { DO }), StrokeKind::Inside);
            }
        }
        if m.hien_ocr {
            for hp in &t.ocr {
                let r = qua(hp.r);
                painter.rect_stroke(r, 0.0, Stroke::new(1.0, VANG), StrokeKind::Inside);
                if !hp.chu.is_empty() {
                    let g = painter.layout(hp.chu.clone(), FontId::proportional(11.0), Color32::WHITE, r.width().max(40.0));
                    painter.rect_filled(Rect::from_min_size(r.min, g.size() + Vec2::splat(2.0)), 2.0, Color32::from_black_alpha(190));
                    painter.galley(r.min + Vec2::splat(1.0), g, Color32::WHITE);
                }
            }
        }
        if m.ch.khung_bat {
            for (i, hp) in t.khung.iter().enumerate() {
                let r = qua(hp.r);
                let chon = m.lop_sua == Lop::Khung && m.chon == Some(i);
                painter.rect_stroke(r, 0.0, Stroke::new(if chon { 3.0 } else { 2.0 }, if chon { Color32::WHITE } else { NHAN }), StrokeKind::Inside);
                let b = Rect::from_min_size(r.min, Vec2::splat(22.0));
                painter.rect_filled(b, 0.0, NHAN);
                chu_giua(&painter, b.center(), &(i + 1).to_string(), 13.0, Color32::WHITE);
            }
        }
        let bo_trang = t.dd.bo;
        if !khoa {
            sua_hop(m, &resp, rect, w, h);
        }
        if bo_trang {
            painter.rect_filled(rect, 0.0, Color32::from_black_alpha(150));
            chu_giua(&painter, rect.center(), "Trang bỏ (không dùng khi dựng)", 18.0, CHU);
        }
        painter.text(resp.rect.right_bottom() - Vec2::new(8.0, 6.0), Align2::RIGHT_BOTTOM, format!("{}/{} · {}", m.trang + 1, n, m.ch.trang[m.trang].ten), FontId::proportional(12.0), CHU_MO);
    }

    fn cot_loi(&mut self, ui: &mut Ui, m: &mut ManXem, khoa: Option<&str>) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width());
            self.khoi_lech(ui, m, khoa.is_some());
            let (trang, dd_ten) = (m.trang, m.ch.trang.get(m.trang).map(|t| t.ten.clone()).unwrap_or_default());
            ui.label(egui::RichText::new(format!("Lời kể — trang {} ({dd_ten})", trang + 1)).size(15.0).strong());
            let mapped = m.ch.doan_trang(trang);
            match mapped {
                Some(d) => {
                    let r = ui.add_enabled(khoa.is_none(), egui::TextEdit::multiline(&mut m.ch.doan[d]).desired_rows(7).desired_width(f32::INFINITY).id_salt(("loi", d)));
                    if let Some(c) = egui::TextEdit::load_state(ui.ctx(), r.id).and_then(|s| s.cursor.char_range()) {
                        if r.has_focus() {
                            m.con_tro = c.primary.index.into();
                        }
                    }
                    ui.label(egui::RichText::new(format!("{} chữ · đoạn {}/{}", m.ch.doan[d].split_whitespace().count(), d + 1, m.ch.doan.len())).size(11.5).color(CHU_MO));
                    ui.horizontal_wrapped(|ui| {
                        if nut(ui, "Lưu lời", true, m.ch.kb_bien_doi() && khoa.is_none()).clicked() {
                            let r = m.ch.luu_kich_ban("Thịnh", "sửa lời");
                            m.ch.tinh_lai_lech();
                            self.gui(r, "Đã lưu lời kể — bản cũ còn trong lịch sử");
                        }
                        if nut(ui, "Hoàn tác", false, m.ch.kb_bien_doi()).clicked() {
                            m.ch.doan = m.ch.doan_luu.clone();
                            m.ch.tinh_lai_lech();
                        }
                        if nut(ui, "Gộp với đoạn sau", false, d + 1 < m.ch.doan.len() && khoa.is_none()).clicked() {
                            chuong::gop_doan(&mut m.ch.doan, d);
                            m.ch.tinh_lai_lech();
                        }
                        if nut(ui, "Tách đoạn tại con trỏ", false, khoa.is_none()).on_hover_text("Tách ở cuối câu gần con trỏ nhất").clicked() && chuong::tach_doan(&mut m.ch.doan, d, m.con_tro) {
                            m.ch.tinh_lai_lech();
                        }
                    });
                }
                None => {
                    ui.colored_label(DO, "Trang này chưa có đoạn lời.");
                    if nut(ui, "Thêm đoạn trống cho trang này", false, khoa.is_none()).clicked() {
                        let pos = (trang + 1..m.ch.trang.len()).find_map(|j| m.ch.doan_trang(j)).unwrap_or(m.ch.doan.len());
                        m.ch.doan.insert(pos, String::new());
                        m.ch.tinh_lai_lech();
                    }
                }
            }
            if let Some(r) = m.ch.doi_chieu.iter().find(|r| r.so == trang + 1).cloned() {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Thoại gốc (EN)").size(13.0).color(CHU_PHU));
                ui.label(if r.en.is_empty() { "—".to_owned() } else { r.en });
                ui.label(egui::RichText::new("Tả cảnh").size(13.0).color(CHU_PHU));
                ui.label(if r.thay.is_empty() { "—".to_owned() } else { r.thay });
            } else if !m.ch.doi_chieu.is_empty() {
                ui.label(egui::RichText::new("Không có dòng đối chiếu cho trang này.").color(CHU_MO));
            }
            ui.add_space(8.0);
            self.khoi_giong(ui, m, mapped, khoa.is_some());
            self.khoi_chu_sai(ui, m, mapped);
            self.khoi_dung_va_danh_dau(ui, m, khoa.is_some());
        });
    }

    fn khoi_lech(&mut self, ui: &mut Ui, m: &mut ManXem, khoa: bool) {
        let l = m.ch.lech.clone();
        if !l.lech() && m.ch.kb.is_some() {
            return;
        }
        egui::Frame::new().fill(Color32::from_rgb(0x3a, 0x1c, 0x20)).corner_radius(8).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if m.ch.kb.is_none() {
                ui.colored_label(DO, "Chương này chưa có kịch bản (kich-ban/…/chương.md).");
                return;
            }
            ui.colored_label(DO, egui::RichText::new(format!("{} đoạn cho {} trang", l.so_doan, l.so_trang)).strong());
            for d in &l.thua {
                let sn: String = m.ch.doan[*d].chars().take(70).collect();
                ui.label(egui::RichText::new(format!("Đoạn {} thừa: “{sn}…”", d + 1)).size(12.5));
            }
            for p in &l.thieu {
                ui.label(egui::RichText::new(format!("Trang {} chưa có đoạn", p + 1)).size(12.5));
            }
            ui.horizontal_wrapped(|ui| {
                if let Some((a, b)) = l.goi_y_gop {
                    if nut(ui, &format!("Gộp đoạn {} + {}", a + 1, b + 1), true, !khoa).on_hover_text("Nối 2 đoạn kề nhau thành 1 (lưu thành bản mới)").clicked() {
                        chuong::gop_doan(&mut m.ch.doan, a);
                        m.ch.tinh_lai_lech();
                        let r = m.ch.luu_kich_ban("Thịnh", &format!("gộp đoạn {} và {}", a + 1, b + 1));
                        self.gui(r, &format!("Đã gộp đoạn {} và {} — bản cũ còn trong lịch sử", a + 1, b + 1));
                    }
                }
                if nut(ui, "Viết lại chương", false, !khoa).on_hover_text("Xếp việc cho agy viết lại đúng chương này (giữ bản cũ)").clicked() {
                    let r = crate::lenh::them_viec(&self.duong.goc_dv, "agy", &m.bo, Some(&m.ma), serde_json::json!({"kieu": "viet_lai", "kiem": "kich_ban", "ghi_chu": [format!("{} đoạn cho {} trang", l.so_doan, l.so_trang)]})).map(|_| ());
                    self.gui(r, "Đã xếp việc viết lại chương vào hàng");
                }
            });
            ui.label(egui::RichText::new("Hoặc tách một đoạn dài ở ô lời kể bên dưới (nút Tách đoạn).").size(12.0).color(CHU_PHU));
        });
        ui.add_space(8.0);
    }

    fn khoi_giong(&mut self, ui: &mut Ui, m: &mut ManXem, mapped: Option<usize>, khoa: bool) {
        ui.label(egui::RichText::new("Giọng").size(13.0).color(CHU_PHU));
        let trang_thai = {
            let g = m.nghe.lock();
            match g.as_deref() {
                Ok(Nghe::Dang(t)) => Some(format!("Đang đọc… {:.0} giây", t.elapsed().as_secs_f32())),
                Ok(Nghe::Phat { giay, mat }) => Some(format!("Đã đọc {giay:.1} giây (làm mất {mat:.1} giây)")),
                Ok(Nghe::Loi(e)) => Some(format!("Không đọc được: {e}")),
                _ => None,
            }
        };
        ui.horizontal_wrapped(|ui| {
            if nut(ui, "▶ Nghe đoạn này", false, mapped.is_some()).clicked() {
                if let Some(d) = mapped {
                    nghe_thu(&self.duong.goc_dv, &m.bo, &m.ch.doan[d], Arc::clone(&m.nghe), ui.ctx().clone());
                }
            }
            if nut(ui, "Làm lại giọng đoạn này", false, mapped.is_some() && !khoa).on_hover_text("Xếp việc làm giọng cho đoạn này (dịch vụ nền); nghe thử bằng nút bên cạnh").clicked() {
                let r = crate::lenh::them_viec(&self.duong.goc_dv, "giong", &m.bo, Some(&m.ma), serde_json::json!({"doan": mapped.map_or(0, |d| d + 1)})).map(|_| ());
                self.gui(r, "Đã xếp việc làm lại giọng vào hàng");
            }
        });
        if let Some(t) = trang_thai {
            ui.label(egui::RichText::new(t).size(12.0).color(CHU_PHU));
        }
        ui.add_space(8.0);
    }

    fn khoi_chu_sai(&mut self, ui: &mut Ui, m: &mut ManXem, mapped: Option<usize>) {
        let Some(d) = mapped else { return };
        let da_co = bang_phat_am(&m.bo);
        let ten: Vec<String> = m.ch.doan[d]
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.chars().count() >= 3 && w.chars().next().is_some_and(char::is_uppercase) && w.chars().all(|c| c.is_ascii_alphabetic()) && !da_co.contains_key(*w))
            .map(str::to_owned)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        ui.label(egui::RichText::new("Chữ đọc sai? (tên / từ nước ngoài)").size(13.0).color(CHU_PHU));
        if ten.is_empty() {
            ui.label(egui::RichText::new("Không có tên nào chưa có cách đọc trong đoạn này.").size(12.0).color(CHU_MO));
        }
        for w in ten {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&w).strong());
                let e = m.cach_doc.entry(w.clone()).or_default();
                ui.add(egui::TextEdit::singleline(e).hint_text("cách đọc, vd Nô da ki").desired_width(150.0));
                let v = e.trim().to_owned();
                if nut(ui, "Lưu vào bảng phát âm", false, !v.is_empty()).clicked() {
                    let r = crate::lenh::sua_bo_json(&m.bo, |j| j["phat_am"][&w] = serde_json::json!(v));
                    self.gui(r, "Đã lưu vào bảng phát âm của bộ (bo.json)");
                }
            });
        }
        ui.add_space(8.0);
    }

    fn khoi_dung_va_danh_dau(&mut self, ui: &mut Ui, m: &mut ManXem, khoa: bool) {
        let i = m.trang;
        if i >= m.ch.trang.len() {
            return;
        }
        ui.label(egui::RichText::new("Kiểu dựng trang").size(13.0).color(CHU_PHU));
        let mut kieu = m.ch.trang[i].dd.kieu.clone();
        let ten_kieu = KIEU.iter().find(|k| k.0 == kieu).map_or("Mặc định của bộ", |k| k.1);
        egui::ComboBox::from_id_salt("kieu_dung").selected_text(ten_kieu).width(220.0).show_ui(ui, |ui| {
            for (k, t) in KIEU {
                ui.selectable_value(&mut kieu, k.to_owned(), t);
            }
        });
        let mut luu = false;
        if kieu != m.ch.trang[i].dd.kieu {
            m.ch.trang[i].dd.kieu = kieu;
            luu = true;
        }
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Đánh dấu").size(13.0).color(CHU_PHU));
        {
            let dd = &mut m.ch.trang[i].dd;
            luu |= ui.add_enabled(!khoa, egui::Checkbox::new(&mut dd.nghi_lech, "Nghi lệch tranh")).changed();
            luu |= ui.add_enabled(!khoa, egui::Checkbox::new(&mut dd.cao_trao, "Trang cao trào")).changed();
            luu |= ui.add_enabled(!khoa, egui::Checkbox::new(&mut dd.bo, "Bỏ trang này (không xoá file, chỉ đánh dấu)")).changed();
            ui.label(egui::RichText::new("Ghi chú cho Antigravity (dùng khi viết lại)").size(12.0).color(CHU_PHU));
            let r = ui.add(egui::TextEdit::multiline(&mut dd.ghi_chu).desired_rows(2).desired_width(f32::INFINITY));
            if r.changed() {
                m.ghi_chu_sua = true;
            }
            if r.lost_focus() && m.ghi_chu_sua {
                m.ghi_chu_sua = false;
                luu = true;
            }
        }
        if luu {
            let r = m.ch.luu_danh_dau("Thịnh", "đánh dấu trang");
            self.gui(r, "Đã lưu đánh dấu");
        }
    }

    fn hop_lich_su(&mut self, ctx: &egui::Context, m: &mut ManXem) {
        let Some(ds) = m.lich_su.clone() else { return };
        let mut mo = true;
        let mut khoi: Option<chuong::PhienBan> = None;
        egui::Window::new("Lịch sử phiên bản").open(&mut mo).default_size([560.0, 420.0]).show(ctx, |ui| {
            if ds.is_empty() {
                ui.label(egui::RichText::new("Chưa có lần sửa nào của chương này.").color(CHU_PHU));
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for pb in &ds {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&pb.gio).color(CHU_PHU));
                        ui.label(egui::RichText::new(chuong::ten_loai(&pb.loai)).strong());
                        ui.label(format!("{} · {}", pb.ai, pb.gi));
                        if nut(ui, "Khôi phục", false, true).on_hover_text("Bản hiện tại thành 1 phiên bản mới, rồi dùng lại bản này").clicked() {
                            khoi = Some(pb.clone());
                        }
                    });
                }
            });
        });
        if let Some(pb) = khoi {
            let r = chuong::khoi_phuc(&m.bo, &m.ma, &pb);
            let ok = r.is_ok();
            self.gui(r, "Đã khôi phục — bản vừa thay vẫn còn trong lịch sử");
            if ok {
                let (bo, ma, t) = (m.bo.clone(), m.ma.clone(), m.trang);
                *m = ManXem::moi(ctx, &bo, &ma, t);
                m.lich_su = Some(chuong::ds_phien_ban(&bo, &ma));
                return;
            }
        }
        if !mo {
            m.lich_su = None;
        }
    }

    pub fn cap_nhat_timeline(&mut self, bo: &Path, ma: &str) {
        self.cap_nhat_bo(bo, ma, false);
    }

    pub fn cap_nhat_timeline_thay(&mut self, bo: &Path, ma: &str) {
        self.cap_nhat_bo(bo, ma, true);
    }

    fn cap_nhat_bo(&mut self, bo: &Path, ma: &str, thay: bool) {
        let (bo, ma, tx) = (bo.to_path_buf(), ma.to_owned(), self.tac_vu_tx.clone());
        self.tb(format!("Đang cập nhật {ma} trên timeline…"));
        std::thread::spawn(move || {
            let _ = tx.send(match crate::timeline::cap_nhat_bo(&bo, &ma, thay) {
                Ok(t) => t,
                Err(e) => format!("Không cập nhật được timeline: {e}"),
            });
        });
    }

    fn xem_truoc(&mut self, bo: &Path, ma: &str) {
        let (bo, ma, tx) = (bo.to_path_buf(), ma.to_owned(), self.tac_vu_tx.clone());
        self.tb(format!("Đang dựng bản xem trước {ma}…"));
        std::thread::spawn(move || {
            let kq = (|| -> Result<PathBuf, String> {
                let lenh = crate::timeline::tim_lenh().ok_or("không thấy lệnh recap-studio")?;
                let dir = std::env::temp_dir().join("recap-studio").join("xem-truoc");
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let ra = dir.join(format!("{ma}.mp4"));
                let nguon = chuong::doc_json(&bo.join("bo.json")).and_then(|j| j.pointer("/chuyen_tu_cu/nguon")?.as_str().map(PathBuf::from)).filter(|p| p.is_dir()).unwrap_or_else(|| bo.clone());
                let r = std::process::Command::new(lenh).arg("dung-chuong").arg(nguon).arg(&ma).arg("--ra").arg(&ra).arg("--du-an").arg(dir.join(format!("{ma}.fcproj"))).args(["--bitrate-kbps", "1200", "--cache"]).arg(dir.join("cache")).output().map_err(|e| e.to_string())?;
                if r.status.success() { Ok(ra) } else { Err(String::from_utf8_lossy(&r.stderr).lines().last().unwrap_or("lõi dựng báo lỗi").trim_start_matches("recap-studio: ").to_owned()) }
            })();
            let _ = tx.send(match kq {
                Ok(p) => {
                    crate::lenh::mo_thu_muc(&p);
                    format!("Bản xem trước đã dựng: {}", p.display())
                }
                Err(e) => format!("Không dựng được bản xem trước: {e}"),
            });
        });
    }
}

/// Kéo/đổi cỡ/chọn hộp của lớp đang sửa; kéo ngoài hộp = dời trang.
fn sua_hop(m: &mut ManXem, resp: &egui::Response, rect: Rect, w: f32, h: f32) {
    let sang_anh = |p: Pos2| Pos2::new((p.x - rect.min.x) / rect.width() * w, (p.y - rect.min.y) / rect.height() * h);
    let (lop, trang) = (m.lop_sua, m.trang);
    let hops: Vec<Hop> = { let t = &m.ch.trang[trang]; if lop == Lop::Khung { t.khung.clone() } else { t.che.clone() } };
    let bat = if lop == Lop::Khung { m.ch.khung_bat } else { m.ch.che_bat };
    if !bat {
        if resp.dragged() {
            m.pan += resp.drag_delta();
        }
        return;
    }
    let k = rect.width() / w;
    let test = |p: Pos2, i: usize| -> Option<u8> {
        let r = hops[i].r;
        let (l, t, rr, b) = (rect.min.x + r[0] * k, rect.min.y + r[1] * k, rect.min.x + r[2] * k, rect.min.y + r[3] * k);
        if p.x < l - 8.0 || p.x > rr + 8.0 || p.y < t - 8.0 || p.y > b + 8.0 {
            return None;
        }
        let mut c = 0u8;
        if (p.x - l).abs() <= 8.0 { c |= 1 } else if (p.x - rr).abs() <= 8.0 { c |= 2 }
        if (p.y - t).abs() <= 8.0 { c |= 4 } else if (p.y - b).abs() <= 8.0 { c |= 8 }
        if c != 0 { Some(c) } else if p.x > l && p.x < rr && p.y > t && p.y < b { Some(16) } else { None }
    };
    let ds: Vec<usize> = m.chon.into_iter().chain((0..hops.len()).rev()).collect();
    if let Some(p) = resp.hover_pos() {
        if let Some(c) = ds.iter().find_map(|i| test(p, *i)) {
            ui_con_tro(resp, c);
        }
    }
    if resp.drag_started() {
        m.keo = None;
        if let Some(p) = resp.interact_pointer_pos() {
            if let Some((i, c)) = ds.iter().find_map(|i| test(p, *i).map(|c| (*i, c))) {
                m.chon = Some(i);
                m.keo = Some(Keo { idx: i, cach: c, r0: hops[i].r, p0: sang_anh(p) });
            }
        }
    } else if resp.clicked() {
        m.chon = resp.interact_pointer_pos().and_then(|p| (0..hops.len()).rev().find(|i| test(p, *i).is_some()));
    }
    if resp.dragged() {
        match (&m.keo, resp.interact_pointer_pos()) {
            (Some(kk), Some(p)) => {
                let (dx, dy) = (sang_anh(p).x - kk.p0.x, sang_anh(p).y - kk.p0.y);
                let mut r = kk.r0;
                if kk.cach & 16 != 0 {
                    let (rw, rh) = (r[2] - r[0], r[3] - r[1]);
                    r[0] = (r[0] + dx).clamp(0.0, w - rw);
                    r[1] = (r[1] + dy).clamp(0.0, h - rh);
                    r[2] = r[0] + rw;
                    r[3] = r[1] + rh;
                } else {
                    if kk.cach & 1 != 0 { r[0] = (r[0] + dx).clamp(0.0, r[2] - 8.0) }
                    if kk.cach & 2 != 0 { r[2] = (r[2] + dx).clamp(r[0] + 8.0, w) }
                    if kk.cach & 4 != 0 { r[1] = (r[1] + dy).clamp(0.0, r[3] - 8.0) }
                    if kk.cach & 8 != 0 { r[3] = (r[3] + dy).clamp(r[1] + 8.0, h) }
                }
                let t = &mut m.ch.trang[trang];
                let v = if lop == Lop::Khung { &mut t.khung } else { &mut t.che };
                if let Some(x) = v.get_mut(kk.idx) {
                    x.r = r;
                }
            }
            (None, _) => m.pan += resp.drag_delta(),
            _ => {}
        }
    }
    if resp.drag_stopped() {
        m.keo = None;
    }
}

fn ui_con_tro(resp: &egui::Response, c: u8) {
    let icon = match c {
        16 => egui::CursorIcon::Grab,
        1 | 2 => egui::CursorIcon::ResizeHorizontal,
        4 | 8 => egui::CursorIcon::ResizeVertical,
        5 | 10 => egui::CursorIcon::ResizeNwSe,
        _ => egui::CursorIcon::ResizeNeSw,
    };
    resp.ctx.set_cursor_icon(icon);
}

/// Bảng phát âm của bộ: `bo.json → phat_am` + `kich-ban/arc*/_ten-rieng.json`.
fn bang_phat_am(bo: &Path) -> BTreeMap<String, String> {
    let mut v = BTreeMap::new();
    let mut doc = |p: PathBuf, ptr: &str| {
        if let Some(o) = chuong::doc_json(&p).and_then(|j| if ptr.is_empty() { Some(j) } else { j.get(ptr).cloned() }).and_then(|j| j.as_object().cloned()) {
            for (k, x) in o {
                if let Some(s) = x.as_str() {
                    v.insert(k, s.to_owned());
                }
            }
        }
    };
    doc(bo.join("bo.json"), "phat_am");
    for e in std::fs::read_dir(bo.join("kich-ban")).into_iter().flatten().flatten() {
        doc(e.path().join("_ten-rieng.json"), "");
    }
    v
}

fn tts_dir() -> PathBuf {
    std::env::var_os("RECAP_TTS_DIR").map(PathBuf::from).unwrap_or_else(|| {
        let repo = std::env::var_os("RECAP_REPO").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).unwrap_or_default()).join("Youtube/Assets/Anime & Manga recap"));
        repo.join("engine/tts")
    })
}

/// Đọc thử 1 đoạn bằng Piper của engine/tts (giọng trong config.json; VieNeu chưa nghe thử ở đây → rơi về giọng Piper),
/// ghi wav vào thư mục tạm rồi phát. Không ghi gì vào `giong/` của bộ.
fn nghe_thu(_dv: &Path, bo: &Path, text: &str, st: Arc<Mutex<Nghe>>, ctx: egui::Context) {
    if let Ok(mut g) = st.lock() {
        *g = Nghe::Dang(std::time::Instant::now());
    }
    let (text, bo) = (text.to_owned(), bo.to_path_buf());
    std::thread::spawn(move || {
        let t0 = std::time::Instant::now();
        let kq = (|| -> Result<f64, String> {
            let dir = tts_dir();
            let cfg = chuong::doc_json(&dir.join("config.json")).unwrap_or_default();
            let model = cfg.get("model").and_then(|v| v.as_str()).filter(|s| !s.starts_with("vieneu:")).unwrap_or("engine/tts/voices/ngochuyennew.onnx");
            let model = dir.join("voices").join(Path::new(model).file_name().unwrap_or_default());
            let py = dir.join(".venv").join(if cfg!(windows) { "Scripts/python.exe" } else { "bin/python" });
            let td = std::env::temp_dir().join("recap-studio").join("nghe");
            std::fs::create_dir_all(&td).map_err(|e| e.to_string())?;
            let mut chu = text.clone();
            for (k, v) in bang_phat_am(&bo) {
                chu = chu.split(' ').map(|w| { let (a, b) = w.split_at(w.find(|c: char| !c.is_alphanumeric()).unwrap_or(w.len())); if a == k { format!("{v}{b}") } else { w.to_owned() } }).collect::<Vec<_>>().join(" ");
            }
            let (txt, wav) = (td.join("doan.txt"), td.join("doan.wav"));
            std::fs::write(&txt, chu).map_err(|e| e.to_string())?;
            let r = std::process::Command::new(&py).arg(dir.join("piper-runner.py")).arg("-m").arg(&model).arg("-i").arg(&txt).arg("-f").arg(&wav).output().map_err(|e| format!("không chạy được Piper ({})", e))?;
            if !r.status.success() {
                return Err(String::from_utf8_lossy(&r.stderr).lines().last().unwrap_or("Piper lỗi").to_owned());
            }
            let b = std::fs::read(&wav).map_err(|e| e.to_string())?;
            let giay = if b.len() > 44 { (b.len() - 44) as f64 / f64::from(u32::from_le_bytes([b[28], b[29], b[30], b[31]]).max(1)) } else { 0.0 };
            let _ = if cfg!(target_os = "macos") { std::process::Command::new("afplay").arg(&wav).spawn() } else if cfg!(windows) { std::process::Command::new("powershell").args(["-c", &format!("(New-Object Media.SoundPlayer '{}').PlaySync()", wav.display())]).spawn() } else { std::process::Command::new("aplay").arg(&wav).spawn() };
            Ok(giay)
        })();
        if let Ok(mut g) = st.lock() {
            *g = match kq {
                Ok(giay) => Nghe::Phat { giay, mat: t0.elapsed().as_secs_f64() },
                Err(e) => Nghe::Loi(e),
            };
        }
        ctx.request_repaint();
    });
}
