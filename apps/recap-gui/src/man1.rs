//! Màn 1 — Thư viện bộ (spec `01-thu-vien-bo.md`): thanh trên + dải cảnh báo + lưới thẻ (ảo hoá theo hàng).
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::path::PathBuf;
use std::sync::Arc;

use crate::du_lieu::{self, Bo, CanhBao, Muc, SO_BUOC, Tt, TtBo};
use crate::giao_dien::*;
use crate::{App, HopMoi, LocTt, Man};

const THE_RONG_MIN: f32 = 310.0;
const THE_CAO: f32 = 188.0;
const KHE: f32 = 12.0;

impl App {
    pub fn man_thu_vien(&mut self, ui: &mut Ui) {
        egui::Panel::top("thanh_tren").show_separator_line(false).show(ui, |ui| self.thanh_tren(ui));
        egui::CentralPanel::default().frame(egui::Frame::new().fill(NEN).inner_margin(egui::Margin::symmetric(16, 12))).show(ui, |ui| {
            if self.snap.co_du_lieu && self.snap.bo.is_empty() {
                self.trang_rong(ui);
                return;
            }
            self.dai_canh_bao(ui);
            self.luoi_the(ui);
        });
    }

    fn thanh_tren(&mut self, ui: &mut Ui) {
        egui::Frame::new().fill(MAT_1).inner_margin(egui::Margin::symmetric(16, 10)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Recap Studio").size(18.0).strong());
                ui.add_space(8.0);
                ui.add(egui::TextEdit::singleline(&mut self.tim).hint_text("Tìm bộ…").desired_width(200.0));
                ui.add_space(8.0);
                for (t, ten) in [(LocTt::Tat, "Tất cả"), (LocTt::DangLam, "Đang làm"), (LocTt::Ket, "Kẹt"), (LocTt::XongChuaDang, "Xong chưa đăng"), (LocTt::DaDang, "Đã đăng")] {
                    if ui.selectable_label(self.loc_tt == t, ten).clicked() {
                        self.loc_tt = t;
                    }
                }
                ui.separator();
                for m in ["Tất cả", "Mac", "PC", "Laptop"] {
                    if ui.selectable_label(self.loc_may == m, m).clicked() {
                        self.loc_may = m.into();
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if nut(ui, "+ Bộ mới", true, true).clicked() {
                        self.hop_moi = Some(HopMoi { may: self.may_dang_mo.clone(), tu_dong: true, ..HopMoi::default() });
                    }
                });
            });
        });
    }

    fn trang_rong(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(120.0);
            ui.label(egui::RichText::new("Bắt đầu bộ đầu tiên").size(22.0).strong());
            ui.add_space(4.0);
            ui.label(egui::RichText::new(format!("Chưa có bộ nào trong {}", self.duong.goc_bo.first().map_or_else(String::new, |p| p.display().to_string()))).color(CHU_PHU));
            ui.add_space(12.0);
            if nut(ui, "+ Bộ mới", true, true).clicked() {
                self.hop_moi = Some(HopMoi { may: self.may_dang_mo.clone(), tu_dong: true, ..HopMoi::default() });
            }
        });
    }

    fn dai_canh_bao(&mut self, ui: &mut Ui) {
        let cb: Arc<Vec<CanhBao>> = Arc::clone(&self.snap.canh_bao);
        if cb.is_empty() {
            return;
        }
        let toi_da = if self.hien_het_canh_bao { cb.len() } else { 4 };
        let mut mo: Option<(PathBuf, usize)> = None;
        egui::Frame::new().fill(MAT_1).corner_radius(8).inner_margin(egui::Margin::symmetric(4, 4)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for c in cb.iter().take(toi_da) {
                let (rect, r) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
                let mau = if c.muc == Muc::Do { DO } else { VANG };
                let p = ui.painter();
                if r.hovered() {
                    p.rect_filled(rect, 6.0, MAT_2);
                }
                p.rect_filled(Rect::from_min_size(rect.min + Vec2::new(8.0, 7.0), Vec2::new(4.0, 16.0)), 2.0, mau);
                ve_chu(p, Pos2::new(rect.min.x + 22.0, rect.center().y - 8.0), &c.noi_dung, 13.5, CHU, rect.width() - 150.0);
                if c.bo.is_some() {
                    p.text(Pos2::new(rect.max.x - 12.0, rect.center().y), Align2::RIGHT_CENTER, "Mở bước này ›", FontId::proportional(12.0), CHU_PHU);
                }
                if r.clicked() {
                    mo = c.bo.clone();
                }
            }
            if cb.len() > 4 {
                let t = if self.hien_het_canh_bao { "Thu gọn".to_owned() } else { format!("… và {} dòng khác", cb.len() - 4) };
                if ui.link(t).clicked() {
                    self.hien_het_canh_bao = !self.hien_het_canh_bao;
                }
            }
        });
        ui.add_space(12.0);
        if let Some((p, i)) = mo {
            self.mo_bo(p, Some(i));
        }
    }

    pub fn mo_bo(&mut self, p: PathBuf, buoc: Option<usize>) {
        self.chon_buoc = buoc;
        self.man = Man::Bo(p);
    }

    /// Chỉ số các bộ qua bộ lọc, đã sắp: kẹt trước → đang làm → gần xuất → xong chờ đăng → rảnh → đã đăng.
    fn danh_sach_loc(&self) -> Vec<usize> {
        let tim = self.tim.to_lowercase();
        let mut v: Vec<usize> = (0..self.snap.bo.len()).filter(|&i| self.qua_loc(&self.snap.bo[i], &tim)).collect();
        v.sort_by(|&a, &b| {
            let (x, y) = (&self.snap.bo[a], &self.snap.bo[b]);
            du_lieu::thu_tu_sap(x.tt).cmp(&du_lieu::thu_tu_sap(y.tt)).then_with(|| x.ten.to_lowercase().cmp(&y.ten.to_lowercase()))
        });
        v
    }

    fn qua_loc(&self, b: &Bo, tim: &str) -> bool {
        let tt = match self.loc_tt {
            LocTt::Tat => true,
            LocTt::DangLam => matches!(b.tt, TtBo::DangLam | TtBo::GanXuat | TtBo::Roi),
            LocTt::Ket => b.tt == TtBo::Ket,
            LocTt::XongChuaDang => b.tt == TtBo::XongChuaDang,
            LocTt::DaDang => b.tt == TtBo::DaDang,
        };
        let may = if b.may.is_empty() { &self.may_dang_mo } else { &b.may };
        tt && (tim.is_empty() || b.ten.to_lowercase().contains(tim)) && (self.loc_may == "Tất cả" || may.to_lowercase().starts_with(&self.loc_may.to_lowercase()))
    }

    fn luoi_the(&mut self, ui: &mut Ui) {
        let ds = self.danh_sach_loc();
        if ds.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(if self.snap.co_du_lieu { "Không có bộ nào khớp bộ lọc" } else { "Đang đọc các bộ…" }).color(CHU_PHU));
            });
            return;
        }
        let rong = ui.available_width();
        let cot = (((rong + KHE) / (THE_RONG_MIN + KHE)).floor() as usize).max(1);
        let rong_the = (rong - KHE * (cot as f32 - 1.0)) / cot as f32 - 1.0;
        let so_hang = ds.len().div_ceil(cot);
        let bo: Arc<Vec<Bo>> = Arc::clone(&self.snap.bo);
        let chay = bo.iter().any(|b| b.buoc.iter().any(|x| x.tt == Tt::Chay));
        if chay {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, THE_CAO, so_hang, |ui, hang| {
            ui.spacing_mut().item_spacing.y = KHE;
            for h in hang {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = KHE;
                    for &i in ds.iter().skip(h * cot).take(cot) {
                        self.the_bo(ui, &bo[i], Vec2::new(rong_the, THE_CAO));
                    }
                });
            }
        });
    }

    fn the_bo(&mut self, ui: &mut Ui, bo: &Bo, kich_thuoc: Vec2) {
        let (rect, r) = ui.allocate_exact_size(kich_thuoc, Sense::click());
        let may = if bo.may.is_empty() { self.may_dang_mo.clone() } else { bo.may.clone() };
        let mat_ket_noi = self.snap.dv.may_khac.iter().find(|m| !m.truc_tuyen && may.to_lowercase().starts_with(&m.ten.to_lowercase())).map(|m| format!("{} mất kết nối từ {}", m.ten, m.mat_tu.map_or_else(String::new, du_lieu::gio_phut)));
        let mo = if mat_ket_noi.is_some() { 0.45 } else { 1.0 };
        ve_nen_the(ui, rect, bo, r.hovered());
        let bia = bo.bia.as_ref().and_then(|p| self.lay_bia(p));
        ve_bia(ui, Rect::from_min_size(rect.min + Vec2::new(12.0, 12.0), Vec2::new(56.0, 80.0)), bia, &bo.ten);
        let p = ui.painter();
        let x = rect.min.x + 80.0;
        ve_chu(p, Pos2::new(x, rect.min.y + 12.0), &bo.ten, 15.0, CHU, rect.max.x - x - 12.0);
        let mut nx = x;
        for (t, m) in [(if bo.loai == "manhwa" { "Manhwa" } else { "Manga" }, CHU_PHU), (may.as_str(), CHU_PHU), (if bo.da_dang { "Đã đăng" } else { "" }, XANH)] {
            if !t.is_empty() {
                nx += nhan(p, Pos2::new(nx, rect.min.y + 36.0), t, m) + 6.0;
            }
        }
        ve_dong_so(p, rect, bo);
        let hover = ve_thanh_buoc(ui, rect, bo);
        ve_trang_thai(ui.painter(), rect, bo, mat_ket_noi);
        if mo < 1.0 {
            ui.painter().rect_filled(rect, 8.0, Color32::from_black_alpha(110));
        }
        if let Some(i) = hover {
            let b = &bo.buoc[i];
            r.clone().on_hover_text_at_pointer(format!("{} — {}", TEN_DAI_BUOC(i), if b.tt == Tt::BoQua { "bỏ qua".to_owned() } else { format!("{}/{} {}  ·  {}", b.xong, b.tong, du_lieu::DON_VI[i], ten_tt(b.tt)) }));
        }
        let thu_muc = bo.thu_muc.clone();
        if r.clicked() {
            self.mo_bo(thu_muc.clone(), None);
        }
        r.context_menu(|ui| self.menu_the(ui, bo));
    }

    fn menu_the(&mut self, ui: &mut Ui, bo: &Bo) {
        if ui.button("Mở thư mục").clicked() {
            crate::lenh::mo_thu_muc(&bo.thu_muc);
            ui.close();
        }
        if ui.button(if bo.tam_dung { "Bỏ tạm dừng bộ" } else { "Tạm dừng bộ" }).clicked() {
            let r = crate::lenh::dat_co(&bo.thu_muc, "tam_dung", !bo.tam_dung);
            self.gui(r, "Đã ghi công tắc tạm dừng vào bo.json");
            ui.close();
        }
        if ui.button(if bo.da_dang { "Bỏ đánh dấu đã đăng" } else { "Đánh dấu đã đăng" }).clicked() {
            let r = crate::lenh::dat_co(&bo.thu_muc, "da_dang", !bo.da_dang);
            self.gui(r, "Đã cập nhật đánh dấu đăng");
            ui.close();
        }
        ui.separator();
        if ui.button("Đưa vào Thùng rác…").clicked() {
            self.xac_nhan_rac = Some(bo.thu_muc.clone());
            ui.close();
        }
    }

    /// Hộp thoại dùng chung các màn: + Bộ mới, xác nhận Thùng rác, hướng dẫn đổi tài khoản agy.
    pub fn hop_thoai(&mut self, ctx: &egui::Context) {
        self.hop_bo_moi(ctx);
        self.hop_rac(ctx);
        self.hop_doi_tai_khoan(ctx);
    }

    fn hop_bo_moi(&mut self, ctx: &egui::Context) {
        let Some(mut h) = self.hop_moi.take() else { return };
        let (mut dong, mut tao) = (false, false);
        egui::Window::new("Bộ mới").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label("Tên bộ");
            ui.add(egui::TextEdit::singleline(&mut h.ten).desired_width(f32::INFINITY).hint_text("vd Kaiju No. 8"));
            ui.label("Loại");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut h.loai_manhwa, false, "Manga");
                ui.selectable_value(&mut h.loai_manhwa, true, "Manhwa");
            });
            ui.label("Máy làm");
            ui.horizontal(|ui| {
                for m in ["Mac", "PC", "Laptop"] {
                    if ui.selectable_label(h.may == m, m).clicked() {
                        h.may = m.into();
                    }
                }
            });
            if h.may == "Laptop" {
                ui.label(egui::RichText::new("Laptop chỉ nhận bộ có video dưới 3 tiếng.").color(VANG).size(12.0));
            }
            ui.label("Nguồn ảnh (đường dẫn thư mục hoặc link tải)");
            ui.add(egui::TextEdit::singleline(&mut h.nguon).desired_width(f32::INFINITY).hint_text("/Users/…/Downloads/ten-bo  hoặc  https://…"));
            ui.checkbox(&mut h.noi_tiep_anime, "Bộ nối tiếp anime (có bước Mở đầu)");
            ui.checkbox(&mut h.tu_dong, "Tự động (xong bước nào máy làm tiếp bước sau)");
            if !h.loi.is_empty() {
                ui.colored_label(DO, &h.loi);
            }
            ui.horizontal(|ui| {
                tao = nut(ui, "Tạo", true, !h.ten.trim().is_empty()).clicked();
                dong = nut(ui, "Huỷ", false, true).clicked();
            });
        });
        if tao {
            let b = crate::lenh::BoMoi { ten: h.ten.clone(), loai: if h.loai_manhwa { "manhwa" } else { "manga" }.into(), may: h.may.clone(), nguon: h.nguon.clone(), noi_tiep_anime: h.noi_tiep_anime, tu_dong: h.tu_dong };
            let goc = self.duong.goc_bo.first().cloned().unwrap_or_default();
            match crate::lenh::tao_bo(&goc, &self.duong.goc_dv, &b) {
                Ok(_) => {
                    self.tb(format!("Đã tạo bộ \"{}\" — bước Nhập đã xếp hàng", b.ten.trim()));
                    self.lam_moi.store(true, std::sync::atomic::Ordering::Relaxed);
                    return;
                }
                Err(e) => h.loi = format!("Không tạo được: {e}"),
            }
        }
        if !dong {
            self.hop_moi = Some(h);
        }
    }

    fn hop_rac(&mut self, ctx: &egui::Context) {
        let Some(p) = self.xac_nhan_rac.clone() else { return };
        let Some(bo) = self.snap.bo.iter().find(|b| b.thu_muc == p).cloned() else {
            self.xac_nhan_rac = None;
            return;
        };
        let (mut dong, mut ok) = (false, false);
        egui::Window::new("Đưa vào Thùng rác?").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.label(format!("Bộ \"{}\" sẽ được chuyển nguyên thư mục sang .thung-rac/ cạnh các bộ khác:", bo.ten));
            ui.label(egui::RichText::new(dong_so(&bo)).color(CHU_PHU));
            ui.label(egui::RichText::new("Không xoá file nào; khôi phục = đổi tên thư mục về chỗ cũ. Việc đang chạy của bộ này nên dừng trước.").color(CHU_PHU).size(12.0));
            ui.horizontal(|ui| {
                ok = nut(ui, "Đưa vào Thùng rác", true, true).clicked();
                dong = nut(ui, "Huỷ", false, true).clicked();
            });
        });
        if ok {
            let r = crate::lenh::vao_thung_rac(&bo.thu_muc).map(|_| ());
            self.gui(r, "Đã đưa bộ vào Thùng rác");
        }
        if ok || dong {
            self.xac_nhan_rac = None;
        }
    }

    fn hop_doi_tai_khoan(&mut self, ctx: &egui::Context) {
        if !self.hop_tai_khoan {
            return;
        }
        let tk = self.snap.dv.agy_tai_khoan.clone();
        let mut dong = false;
        egui::Window::new("Đổi tài khoản agy").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.label(format!("Tài khoản agy đang dùng: {}", if tk.is_empty() { "chưa rõ" } else { &tk }));
            ui.label(egui::RichText::new("Đăng nhập agy bằng tài khoản khác ở cửa sổ dòng lệnh. Dịch vụ nền nhận tài khoản mới trong vài giây và chạy lại ngay các việc đang chờ quota — không cần bấm gì thêm ở đây.").color(CHU_PHU));
            dong = nut(ui, "Đã hiểu", true, true).clicked();
        });
        if dong {
            self.hop_tai_khoan = false;
        }
    }
}

#[allow(non_snake_case)]
fn TEN_DAI_BUOC(i: usize) -> &'static str {
    du_lieu::TEN_DAI[i]
}

fn ve_nen_the(ui: &Ui, rect: Rect, bo: &Bo, hover: bool) {
    let p = ui.painter();
    let ket = bo.tt == TtBo::Ket;
    p.rect_filled(rect, 8.0, if hover { MAT_3 } else { MAT_2 });
    let vien = if ket { Stroke::new(1.5, DO) } else { Stroke::new(1.0, VIEN) };
    p.rect_stroke(rect, 8.0, vien, StrokeKind::Inside);
}

fn ve_bia(ui: &Ui, r: Rect, tex: Option<egui::TextureId>, ten: &str) {
    let p = ui.painter();
    match tex {
        Some(t) => {
            p.image(t, r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        None => {
            p.rect_filled(r, 4.0, MAT_3);
            let c = ten.chars().next().map_or_else(String::new, |c| c.to_uppercase().to_string());
            chu_giua(p, r.center(), &c, 24.0, CHU_PHU);
        }
    }
}

fn dong_so(bo: &Bo) -> String {
    du_lieu::dong_so(bo)
}

fn ve_dong_so(p: &egui::Painter, rect: Rect, bo: &Bo) {
    let x = rect.min.x + 80.0;
    ve_chu(p, Pos2::new(x, rect.min.y + 62.0), &format!("{} chương", bo.buoc[0].tong), 12.0, CHU_PHU, rect.max.x - x - 12.0);
    ve_chu(p, Pos2::new(rect.min.x + 12.0, rect.min.y + 126.0), &dong_so(bo), 12.0, CHU_PHU, rect.width() - 24.0);
}

/// 14 ô nhỏ; trả ô đang được rê chuột.
fn ve_thanh_buoc(ui: &Ui, rect: Rect, bo: &Bo) -> Option<usize> {
    let t = ui.input(|i| i.time);
    let (x0, y0, w) = (rect.min.x + 12.0, rect.min.y + 104.0, rect.width() - 24.0);
    let o = (w - 3.0 * (SO_BUOC as f32 - 1.0)) / SO_BUOC as f32;
    let con_tro = ui.input(|i| i.pointer.hover_pos());
    let mut hover = None;
    for (i, b) in bo.buoc.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x0 + i as f32 * (o + 3.0), y0), Vec2::new(o, 12.0));
        ui.painter().rect_filled(r, 3.0, mau_dong(b.tt, t));
        if con_tro.is_some_and(|c| r.expand2(Vec2::new(1.5, 4.0)).contains(c)) {
            hover = Some(i);
        }
    }
    hover
}

fn ve_trang_thai(p: &egui::Painter, rect: Rect, bo: &Bo, mat_ket_noi: Option<String>) {
    let y = rect.min.y + 152.0;
    let mau = match bo.tt {
        TtBo::Ket => DO,
        TtBo::DangLam | TtBo::GanXuat => XANH_NHAT,
        TtBo::XongChuaDang | TtBo::DaDang => XANH,
        TtBo::Roi => CHU_PHU,
    };
    let (chu, mau) = mat_ket_noi.map_or((bo.cau.clone(), mau), |m| (m, VANG));
    let truoc_chu = if bo.cap_nhat > 0.0 { truoc(bo.cap_nhat) } else { String::new() };
    let w_truoc = p.layout_no_wrap(truoc_chu.clone(), FontId::proportional(11.0), CHU_MO).size().x;
    p.circle_filled(Pos2::new(rect.min.x + 18.0, y + 8.0), 4.0, mau);
    ve_chu(p, Pos2::new(rect.min.x + 30.0, y), &chu, 13.0, mau, rect.width() - 54.0 - w_truoc);
    p.text(Pos2::new(rect.max.x - 12.0, y + 8.0), Align2::RIGHT_CENTER, truoc_chu, FontId::proportional(11.0), CHU_MO);
}
