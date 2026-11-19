//! Màn 5 — Timeline theo phần (spec `05-timeline.md`): cây Việt ▸ Phần ▸ chương (EN, Nội dung nếu có) bên trái,
//! lane clip của sequence đang chọn ở giữa, thanh trên: Mở trong FilmCraft · Mở trong Màn 3 · Cập nhật từ dữ liệu.
//! Kiểu B (T621): 1 bộ = 1 dự án, mỗi phần 1 dải phẳng; cập nhật thay đúng đoạn chương, chương "Đã sửa tay" được giữ nguyên.
use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use crate::giao_dien::*;
use crate::timeline::{self, MucTl, Nhan};
use crate::{App, Man};

pub struct ManTl {
    pub bo: PathBuf,
    rx: mpsc::Receiver<Vec<MucTl>>,
    pub muc: Vec<MucTl>,
    pub chon: Option<String>,
    cho_chon: Option<String>,
    zoom: f32,
    cuon: f32,
    da_quet: bool,
    pub ms_quet: u128,
}

fn gm(giay: f64) -> String {
    let s = giay.round() as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

impl ManTl {
    pub fn moi(ctx: &egui::Context, bo: &Path, ma: Option<&str>) -> Self {
        let (tx, rx) = mpsc::channel();
        let (b, c) = (bo.to_path_buf(), ctx.clone());
        std::thread::spawn(move || {
            let mut bo_nho = HashMap::new();
            loop {
                let t = std::time::Instant::now();
                let v = timeline::quet(&b, &mut bo_nho);
                let _ = t;
                if tx.send(v).is_err() {
                    return;
                }
                c.request_repaint();
                std::thread::sleep(Duration::from_secs(3));
            }
        });
        Self { bo: bo.to_path_buf(), rx, muc: vec![], chon: None, cho_chon: ma.map(str::to_owned), zoom: 1.0, cuon: 0.0, da_quet: false, ms_quet: 0 }
    }
}

fn mau_nhan(n: &Nhan) -> Color32 {
    match n {
        Nhan::Moi => XANH,
        Nhan::DaSuaTay => NHAN,
        Nhan::CuHonKichBan => VANG,
    }
}

impl App {
    pub fn mo_timeline(&mut self, ctx: &egui::Context, bo: &Path, ma: Option<&str>) {
        self.tl = Some(Box::new(ManTl::moi(ctx, bo, ma)));
        self.man = Man::Timeline(bo.to_path_buf());
    }

    pub fn man_timeline(&mut self, ui: &mut Ui) {
        let Some(mut t) = self.tl.take() else {
            self.man = Man::Thu;
            return;
        };
        while let Ok(v) = t.rx.try_recv() {
            t.muc = v;
            t.da_quet = true;
        }
        if let Some(ma) = t.cho_chon.clone() {
            if let Some(m) = t.muc.iter().find(|m| m.ma == ma && m.nhom == "viet" && !m.v2) {
                t.chon = Some(m.id.clone());
                t.cho_chon = None;
            } else if t.da_quet {
                t.cho_chon = None;
            }
        }
        let ten_bo = self.snap.bo.iter().find(|b| b.thu_muc == t.bo).map_or_else(|| crate::chuong::ten_file(&t.bo), |b| b.ten.clone());
        let chon = t.chon.clone().and_then(|f| t.muc.iter().find(|m| m.id == f).cloned());
        let mut di: Option<&str> = None;
        egui::Panel::top("tl_tren").show_separator_line(false).frame(egui::Frame::new().fill(MAT_1).inner_margin(egui::Margin::symmetric(16, 8))).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if nut(ui, "← Bảng điều khiển", false, true).clicked() {
                    di = Some("bo");
                }
                ui.label(egui::RichText::new(format!("{ten_bo} — Timeline")).size(15.0).strong());
                ui.separator();
                let ma_ch = chon.as_ref().filter(|m| m.ma.starts_with("ch")).map(|m| m.ma.clone());
                if nut(ui, "Mở trong FilmCraft", true, chon.is_some()).on_hover_text("Mở dự án .fcproj của mục đang chọn bằng trình dựng FilmCraft").clicked() {
                    if let Some(m) = &chon {
                        let r = if m.so_phan > 0 { timeline::mo_phan_filmcraft(&t.bo, &m.nhom, m.so_phan, &m.file) } else { timeline::mo_filmcraft(&m.file).map_err(|e| e.to_string()) };
                        self.gui(r.map_err(std::io::Error::other), "Đang mở FilmCraft…");
                    }
                }
                if nut(ui, "Mở trong Màn 3", false, ma_ch.is_some()).clicked() {
                    di = Some("man3");
                }
                if nut(ui, "Cập nhật từ dữ liệu", false, ma_ch.is_some()).on_hover_text("Dựng lại chương từ dữ liệu mới nhất, thay đúng đoạn chương trên dải phần (chương sau dời theo). Chương đã sửa tay: GIỮ bản của bạn, không ghi đè.").clicked() {
                    if let Some(ma) = &ma_ch {
                        self.cap_nhat_timeline(&t.bo.clone(), ma);
                    }
                }
                if nut(ui, "Thay bằng bản mới", false, chon.as_ref().is_some_and(|m| m.nhan.contains(&Nhan::DaSuaTay) && !m.phan.is_empty())).on_hover_text("Chương đã sửa tay: bỏ bản sửa tay, dựng lại từ dữ liệu").clicked() {
                    if let Some(ma) = &ma_ch {
                        self.cap_nhat_timeline_thay(&t.bo.clone(), ma);
                    }
                }
                if nut(ui, "Xuất phần này", false, true).on_hover_text("Mở Màn Xuất & gói đăng của bộ").clicked() {
                    di = Some("xuat");
                }
            });
        });
        egui::Panel::left("tl_cay").exact_size(330.0).resizable(false).frame(egui::Frame::new().fill(MAT_1).inner_margin(egui::Margin::same(10))).show(ui, |ui| self.cay(ui, &mut t));
        egui::CentralPanel::default().frame(egui::Frame::new().fill(NEN).inner_margin(egui::Margin::same(12))).show(ui, |ui| self.giua_tl(ui, &mut t, chon.as_ref()));
        let ctx = ui.ctx().clone();
        match di {
            Some("bo") => self.man = Man::Bo(t.bo.clone()),
            Some("xuat") => self.mo_xuat(&t.bo.clone()),
            Some("man3") => {
                if let Some(ma) = chon.as_ref().map(|m| m.ma.clone()) {
                    self.mo_chuong(&ctx, &t.bo.clone(), &ma, 0);
                }
            }
            _ => {}
        }
        if matches!(self.man, Man::Timeline(_)) {
            self.tl = Some(t);
        }
    }

    fn cay(&mut self, ui: &mut Ui, t: &mut ManTl) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !t.da_quet {
                ui.label(egui::RichText::new("Đang đọc timeline…").color(CHU_PHU));
                return;
            }
            let cay = timeline::cay_viet(&t.bo, &t.muc);
            let n_viet = t.muc.iter().filter(|m| m.nhom == "viet").count();
            egui::CollapsingHeader::new(egui::RichText::new(format!("Việt ({n_viet})")).strong().size(14.0)).default_open(true).show(ui, |ui| {
                if cay.is_empty() {
                    ui.label(egui::RichText::new("Chưa có timeline nào. Mở 1 chương ở Màn 3 rồi bấm \"Cập nhật chương trên timeline\", hoặc chạy bước Timeline ở Bảng điều khiển.").size(12.0).color(CHU_PHU));
                }
                for (phan, ids) in &cay {
                    let tong: f64 = ids.iter().filter(|i| !t.muc[**i].v2).map(|i| t.muc[*i].giay).sum();
                    egui::CollapsingHeader::new(format!("{phan} · {} chương · {}", ids.len(), gm(tong))).id_salt(phan).default_open(ids.len() <= 12 || ids.iter().any(|i| Some(&t.muc[*i].id) == t.chon.as_ref())).show(ui, |ui| {
                        for i in ids {
                            let m = t.muc[*i].clone();
                            hang_cay(ui, t, &m);
                        }
                    });
                }
            });
            for (nhom, ten) in [("en", "Tiếng Anh"), ("noi-dung", "Nội dung")] {
                let ids: Vec<usize> = (0..t.muc.len()).filter(|i| t.muc[*i].nhom == nhom).collect();
                egui::CollapsingHeader::new(egui::RichText::new(format!("{ten} ({})", ids.len())).strong().size(14.0)).default_open(!ids.is_empty()).show(ui, |ui| {
                    if ids.is_empty() {
                        ui.label(egui::RichText::new("Chưa có.").size(12.0).color(CHU_MO));
                    }
                    for i in ids {
                        let m = t.muc[i].clone();
                        hang_cay(ui, t, &m);
                    }
                });
            }
        });
    }

    fn giua_tl(&mut self, ui: &mut Ui, t: &mut ManTl, chon: Option<&MucTl>) {
        let Some(m) = chon else {
            ui.vertical_centered(|ui| {
                ui.add_space(120.0);
                ui.label(egui::RichText::new("Chọn một chương ở cây bên trái").size(18.0));
                ui.label(egui::RichText::new("Hiện các lane clip của chương; sửa chi tiết bằng \"Mở trong FilmCraft\".").color(CHU_PHU));
            });
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(&m.ma).size(18.0).strong());
            for n in &m.nhan {
                ui.colored_label(mau_nhan(n), format!("● {}", timeline::ten_nhan(n)));
            }
            ui.label(egui::RichText::new(format!("{} · {} clip · {} sequence · máy dựng {}", gm(m.giay), m.so_clip, m.so_seq, truoc(m.gio_dung))).color(CHU_PHU));
        });
        if let Some(goc) = t.muc.iter().find(|x| x.nhom == m.nhom && m.v2 && x.ma == m.ma && !x.v2) {
            ui.label(egui::RichText::new(format!("So với bản cũ ({}): {} → {}, {} → {} clip", crate::chuong::ten_file(&goc.file), gm(goc.giay), gm(m.giay), goc.so_clip, m.so_clip)).size(12.5).color(VANG));
        }
        if let Some(v2) = t.muc.iter().find(|x| x.nhom == m.nhom && x.v2 && x.ma == m.ma && !m.v2) {
            ui.label(egui::RichText::new(format!("Có bản mới {} ({} · {} clip) cạnh bản này — bản sửa tay của bạn còn nguyên.", crate::chuong::ten_file(&v2.file), gm(v2.giay), v2.so_clip)).size(12.5).color(VANG));
        }
        ui.horizontal(|ui| {
            if nut(ui, "−", false, true).clicked() {
                t.zoom = (t.zoom / 1.5).max(1.0);
            }
            ui.label(format!("{:.0}%", t.zoom * 100.0));
            if nut(ui, "+", false, true).clicked() {
                t.zoom = (t.zoom * 1.5).min(40.0);
            }
        });
        let (resp, p) = ui.allocate_painter(ui.available_size(), Sense::drag());
        if resp.hovered() {
            let z = ui.input(|i| i.zoom_delta());
            if (z - 1.0).abs() > 1e-3 {
                t.zoom = (t.zoom * z).clamp(1.0, 40.0);
            }
            t.cuon -= ui.input(|i| i.smooth_scroll_delta.x);
        }
        if resp.dragged() {
            t.cuon -= resp.drag_delta().x;
        }
        let rong = resp.rect.width() - 70.0;
        let tong = (rong * t.zoom).max(1.0);
        t.cuon = t.cuon.clamp(0.0, (tong - rong).max(0.0));
        p.rect_filled(resp.rect, 6.0, MAT_1);
        let giay = m.giay.max(1.0) as f32;
        let k = tong / giay;
        let lane_cao = ((resp.rect.height() - 30.0) / m.lane.len().max(1) as f32).clamp(14.0, 34.0);
        let mut y = resp.rect.min.y + 24.0;
        // thước giây
        let buoc = [1.0f32, 2.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0].into_iter().find(|b| b * k >= 60.0).unwrap_or(900.0);
        let mut s = (t.cuon / k / buoc).floor() * buoc;
        while s * k - t.cuon < rong {
            let x = resp.rect.min.x + 64.0 + s * k - t.cuon;
            if x >= resp.rect.min.x + 64.0 {
                p.line_segment([Pos2::new(x, resp.rect.min.y + 18.0), Pos2::new(x, resp.rect.min.y + 24.0)], Stroke::new(1.0, CHU_MO));
                p.text(Pos2::new(x + 3.0, resp.rect.min.y + 4.0), egui::Align2::LEFT_TOP, gm(f64::from(s)), egui::FontId::proportional(11.0), CHU_MO);
            }
            s += buoc;
        }
        let vung = Rect::from_min_max(Pos2::new(resp.rect.min.x + 64.0, resp.rect.min.y), resp.rect.max);
        let pv = p.with_clip_rect(vung);
        for (li, lane) in m.lane.iter().enumerate() {
            let ly = y + li as f32 * (lane_cao + 2.0);
            if ly > resp.rect.max.y {
                break;
            }
            p.text(Pos2::new(resp.rect.min.x + 6.0, ly + lane_cao / 2.0), egui::Align2::LEFT_CENTER, &lane.ten, egui::FontId::proportional(11.0), CHU_PHU);
            pv.rect_filled(Rect::from_min_size(Pos2::new(vung.min.x, ly), Vec2::new(vung.width(), lane_cao)), 2.0, MAT_2);
            for (ci, c) in lane.clip.iter().enumerate() {
                let x0 = vung.min.x + (c.bat_dau - m.t0) as f32 * k - t.cuon;
                let x1 = x0 + (c.dai as f32 * k).max(1.0);
                if x1 < vung.min.x || x0 > vung.max.x {
                    continue;
                }
                let r = Rect::from_min_max(Pos2::new(x0, ly + 1.0), Pos2::new(x1 - 0.5, ly + lane_cao - 1.0));
                pv.rect_filled(r, 2.0, if lane.ten.starts_with("Tiếng") { Color32::from_rgb(0x2f, 0x7d, 0x5c) } else if ci % 2 == 0 { Color32::from_rgb(0x4a, 0x58, 0xd6) } else { Color32::from_rgb(0x3b, 0x45, 0xa8) });
                if r.width() > 46.0 {
                    crate::giao_dien::ve_chu(&pv, r.min + Vec2::new(4.0, 2.0), &c.ten, 11.0, Color32::WHITE, r.width() - 6.0);
                }
            }
        }
        y += m.lane.len() as f32 * (lane_cao + 2.0);
        let _ = (y, StrokeKind::Inside);
    }
}

fn hang_cay(ui: &mut Ui, t: &mut ManTl, m: &MucTl) {
    let (rect, r) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
    let chon = t.chon.as_ref() == Some(&m.id);
    ui.painter().rect_filled(rect, 4.0, if chon { Color32::from_rgb(0x2a, 0x30, 0x5c) } else if r.hovered() { MAT_3 } else { MAT_2 });
    let ten = if m.phan.is_empty() { crate::chuong::ten_file(&m.file).trim_end_matches(".fcproj").replace('.', " ") } else { m.ma.clone() };
    ve_chu(ui.painter(), rect.min + Vec2::new(8.0, 6.0), &ten, 13.0, CHU, 110.0);
    ve_chu(ui.painter(), rect.min + Vec2::new(86.0, 7.0), &gm(m.giay), 12.0, CHU_PHU, 50.0);
    let mut x = rect.max.x - 6.0;
    for n in m.nhan.iter().rev() {
        let ten = match n {
            Nhan::Moi => "Mới",
            Nhan::DaSuaTay => "Đã sửa tay",
            Nhan::CuHonKichBan => "Cũ hơn kịch bản",
        };
        let g = ui.painter().layout_no_wrap(ten.to_owned(), egui::FontId::proportional(11.0), mau_nhan(n));
        x -= g.size().x + 12.0;
        let b = Rect::from_min_size(Pos2::new(x, rect.min.y + 5.0), Vec2::new(g.size().x + 10.0, 18.0));
        ui.painter().rect_filled(b, 9.0, MAT_3);
        ui.painter().galley(Pos2::new(b.min.x + 5.0, b.min.y + (18.0 - g.size().y) / 2.0), g, mau_nhan(n));
        x -= 2.0;
    }
    if r.clicked() {
        t.chon = Some(m.id.clone());
        t.cuon = 0.0;
    }
}
