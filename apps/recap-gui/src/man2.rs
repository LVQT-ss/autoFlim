//! Màn 2 — Bảng điều khiển 1 bộ (spec `02-bang-dieu-khien-bo.md`): đầu trang, dải 14 bước, panel chi tiết bước, nhật ký.
use egui::{Align2, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::path::Path;

use crate::du_lieu::{self, Bo, Buoc, DON_VI, LOAI_BUOC, SO_BUOC, TEN_DAI, TEN_NGAN, Tt};
use crate::giao_dien::*;
use crate::{App, Man};

const MAN_SAU: &str = "Màn này làm ở mốc sau (spec 03–06)";

impl App {
    pub fn man_bo(&mut self, ui: &mut Ui, thu_muc: &Path) {
        let Some(bo) = self.snap.bo.iter().find(|b| b.thu_muc == thu_muc).cloned() else {
            return self.bo_khong_thay(ui, thu_muc);
        };
        let chon = self.chon_buoc.unwrap_or_else(|| buoc_mac_dinh(&bo));
        egui::CentralPanel::default().frame(egui::Frame::new().fill(NEN).inner_margin(egui::Margin::symmetric(16, 12))).show(ui, |ui| {
            self.dau_trang(ui, &bo);
            ui.add_space(8.0);
            self.dai_buoc(ui, &bo, chon);
            ui.add_space(8.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                self.panel_buoc(ui, &bo, chon);
                ui.add_space(8.0);
                self.nhat_ky(ui, &bo, chon);
            });
        });
    }

    fn bo_khong_thay(&mut self, ui: &mut Ui, thu_muc: &Path) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(80.0);
            ui.vertical_centered(|ui| {
                if self.snap.co_du_lieu {
                    ui.label(egui::RichText::new(format!("Không thấy bộ \"{}\"", thu_muc.file_name().map_or_else(String::new, |s| s.to_string_lossy().into_owned()))).size(18.0));
                    ui.label(egui::RichText::new("Có thể bộ đã vào Thùng rác hoặc bị đổi tên.").color(CHU_PHU));
                } else {
                    ui.label(egui::RichText::new("Đang đọc bộ…").color(CHU_PHU));
                }
                if nut(ui, "← Thư viện", false, true).clicked() {
                    self.man = Man::Thu;
                }
            });
        });
    }

    fn dau_trang(&mut self, ui: &mut Ui, bo: &Bo) {
        ui.horizontal(|ui| {
            if nut(ui, "← Thư viện", false, true).clicked() {
                self.man = Man::Thu;
            }
            let (rect, _) = ui.allocate_exact_size(Vec2::new(36.0, 52.0), Sense::hover());
            match bo.bia.as_ref().and_then(|p| self.lay_bia(p)) {
                Some(t) => {
                    ui.painter().image(t, rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), egui::Color32::WHITE);
                }
                None => {
                    ui.painter().rect_filled(rect, 4.0, MAT_3);
                    chu_giua(ui.painter(), rect.center(), &bo.ten.chars().next().map_or_else(String::new, |c| c.to_string()), 18.0, CHU_PHU);
                }
            }
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(&bo.ten).size(20.0).strong());
                let may = if bo.may.is_empty() { &self.may_dang_mo } else { &bo.may };
                ui.label(egui::RichText::new(format!("{} · {} · {}", if bo.loai == "manhwa" { "Manhwa" } else { "Manga" }, may, bo.cau)).size(12.0).color(CHU_PHU));
            });
            ui.add_space(12.0);
            if cong_tac(ui, bo.tu_dong_ca_bo, "Tự động cả bộ").on_hover_text("Bật: xong bước nào máy tự làm bước sau. Ghi vào bo.json (tu_dong.ca_bo).").clicked() {
                let r = crate::lenh::dat_tu_dong_ca_bo(&bo.thu_muc, !bo.tu_dong_ca_bo);
                self.gui(r, "Đã ghi công tắc Tự động cả bộ vào bo.json");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                nut(ui, "Nội dung", false, false).on_disabled_hover_text(MAN_SAU);
                if nut(ui, "Xuất", false, true).clicked() {
                    self.mo_xuat(&bo.thu_muc);
                }
                if nut(ui, "Timeline", false, true).clicked() {
                    let ctx = ui.ctx().clone();
                    self.mo_timeline(&ctx, &bo.thu_muc, None);
                }
                let dau = bo.buoc[0].dong.iter().map(|d| d.ten.clone()).find(|t| bo.buoc[7].dong.iter().any(|d| d.ten == *t && d.tt == Tt::Loi)).or_else(|| bo.buoc[0].dong.first().map(|d| d.ten.clone()));
                if nut(ui, "Mở chương", false, dau.is_some()).on_hover_text("Mở chương lỗi đầu tiên (hoặc chương đầu) ở Màn Xem chương").clicked() {
                    if let Some(ma) = dau {
                        let ctx = ui.ctx().clone();
                        self.mo_chuong(&ctx, &bo.thu_muc, &ma, 0);
                    }
                }
                if nut(ui, "Mở thư mục", false, true).clicked() {
                    crate::lenh::mo_thu_muc(&bo.thu_muc);
                }
            });
        });
    }

    /// Dải 14 ô cuộn ngang được: tên ngắn · "N/M" · màu trạng thái.
    fn dai_buoc(&mut self, ui: &mut Ui, bo: &Bo, chon: usize) {
        let t = ui.input(|i| i.time);
        if bo.buoc.iter().any(|b| b.tt == Tt::Chay) {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
        }
        egui::ScrollArea::horizontal().id_salt("dai_buoc").auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for (i, b) in bo.buoc.iter().enumerate() {
                    let (rect, r) = ui.allocate_exact_size(Vec2::new(104.0, 72.0), Sense::click());
                    ve_o_buoc(ui, rect, i, b, i == chon, r.hovered(), t);
                    if r.clicked() {
                        self.chon_buoc = Some(i);
                    }
                    r.on_hover_text(TEN_DAI[i]);
                }
            });
        });
    }

    fn panel_buoc(&mut self, ui: &mut Ui, bo: &Bo, i: usize) {
        let b = &bo.buoc[i];
        egui::Frame::new().fill(MAT_1).corner_radius(8).inner_margin(egui::Margin::same(16)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("{}. {}", i + 1, TEN_DAI[i])).size(17.0).strong());
                ui.label(egui::RichText::new(ten_tt(b.tt)).color(mau_tt(b.tt)).size(13.0));
            });
            ui.label(egui::RichText::new(&b.cau).color(CHU_PHU));
            ui.add_space(4.0);
            self.nut_buoc(ui, bo, i);
            if b.tt != Tt::BoQua {
                ui.add_space(8.0);
                self.bang_chuong(ui, bo, i);
            }
        });
    }

    fn nut_buoc(&mut self, ui: &mut Ui, bo: &Bo, i: usize) {
        let b = &bo.buoc[i];
        let loai = LOAI_BUOC[i];
        let dang_dung = b.dang_chay.iter().any(|id| self.dang_dung.contains(id));
        ui.horizontal_wrapped(|ui| {
            let co_viec_may = loai.is_some_and(|l| l != "agy");
            let chay = nut(ui, "Chạy", true, co_viec_may && b.tt != Tt::BoQua && !bo.tam_dung);
            let chay = if loai == Some("agy") { chay.on_disabled_hover_text("Bước agy cần mẫu đề của bước này — chưa nối ở mốc này, vẫn chạy bằng quy trình cũ") } else if loai.is_none() { chay.on_disabled_hover_text("Bước này do máy tự đo từ file, không có việc để chạy") } else { chay };
            if chay.clicked() {
                let r = crate::lenh::chay_buoc(&self.duong.goc_dv, bo, i).map(|n| n.to_string());
                match r {
                    Ok(n) => self.tb(if n == "0" { "Không có chương nào cần chạy ở bước này".to_owned() } else { format!("Đã xếp {n} việc vào hàng") }),
                    Err(e) => self.tb(format!("Không làm được: {e}")),
                }
                self.lam_moi.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            let dung = nut(ui, if dang_dung { "Đang dừng…" } else { "Dừng" }, false, !b.dang_chay.is_empty() && !dang_dung);
            if dung.on_disabled_hover_text(if dang_dung { "Dừng sạch sau việc đang làm dở" } else { "Không có việc nào đang chạy ở bước này" }).clicked() {
                let r = b.dang_chay.iter().try_for_each(|id| crate::lenh::gui_lenh(&self.duong.goc_dv, "dung", id));
                self.dang_dung.extend(b.dang_chay.iter().cloned());
                self.gui(r, "Đã gửi lệnh Dừng — chờ dịch vụ dừng sạch");
            }
            if nut(ui, "Chạy lại chương lỗi", false, !b.viec_loi.is_empty()).on_disabled_hover_text("Không có việc lỗi ở bước này").clicked() {
                let r = b.viec_loi.iter().try_for_each(|id| crate::lenh::gui_lenh(&self.duong.goc_dv, "thu_lai", id));
                self.gui(r, "Đã gửi lệnh Thử lại");
            }
            if cong_tac(ui, bo.tu_dong_buoc[i], "Tự động bước này").clicked() {
                let r = crate::lenh::dat_tu_dong_buoc(&bo.thu_muc, i, !bo.tu_dong_buoc[i]);
                self.gui(r, "Đã ghi công tắc bước vào bo.json");
            }
            if loai == Some("agy") && nut(ui, "Đổi tài khoản agy", false, true).clicked() {
                self.hop_tai_khoan = true;
            }
        });
    }

    fn bang_chuong(&mut self, ui: &mut Ui, bo: &Bo, i: usize) {
        let b = &bo.buoc[i];
        if b.dong.is_empty() {
            ui.label(egui::RichText::new("Chưa có mục nào để hiện ở bước này.").color(CHU_MO));
            return;
        }
        let w = ui.available_width();
        let cot = [0.0, 110.0, 250.0, 340.0];
        let nay = du_lieu::bay_gio();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::hover());
        for (c, t) in cot.iter().zip([DON_VI_HOA[i], "Trạng thái", "Thời gian", "Lý do / việc nên làm"]) {
            ui.painter().text(Pos2::new(rect.min.x + 8.0 + c, rect.center().y), Align2::LEFT_CENTER, t, FontId::proportional(11.5), CHU_MO);
        }
        let mut thu_lai: Vec<String> = vec![];
        let mut xem: Option<String> = None;
        let la_chuong = DON_VI_HOA[i] == "Chương";
        egui::ScrollArea::vertical().id_salt(("bang", i)).max_height(380.0).auto_shrink([false, true]).show_rows(ui, 32.0, b.dong.len(), |ui, hang| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for k in hang {
                ve_dong_bang(ui, &b.dong[k], k, w, &cot, nay, &mut thu_lai, la_chuong && b.dong[k].ten.starts_with("ch"), &mut xem);
            }
        });
        if let Some(ma) = xem {
            let ctx = ui.ctx().clone();
            self.mo_chuong(&ctx, &bo.thu_muc, &ma, 0);
        }
        if !thu_lai.is_empty() {
            let r = thu_lai.iter().try_for_each(|id| crate::lenh::gui_lenh(&self.duong.goc_dv, "thu_lai", id));
            self.gui(r, "Đã gửi lệnh Thử lại");
        }
    }

    fn nhat_ky(&mut self, ui: &mut Ui, bo: &Bo, i: usize) {
        let tu_khoa = bo.thu_muc.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let loai = LOAI_BUOC[i];
        let loc_buoc = self.loc_nhat_ky_buoc && loai.is_some();
        let dong: Vec<&String> = self.snap.nhat_ky.iter().filter(|l| l.contains(&tu_khoa) && (!loc_buoc || loai.is_none_or(|k| l.contains(k)))).collect();
        let dong: Vec<&&String> = dong.iter().rev().take(50).collect::<Vec<_>>().into_iter().rev().collect();
        egui::CollapsingHeader::new(egui::RichText::new(format!("Nhật ký ({} dòng gần nhất của bộ)", dong.len())).size(14.0)).default_open(false).show(ui, |ui| {
            if loai.is_some() {
                ui.checkbox(&mut self.loc_nhat_ky_buoc, format!("Chỉ bước {}", TEN_NGAN[i]));
            }
            if dong.is_empty() {
                ui.label(egui::RichText::new("Chưa có dòng nhật ký nào cho bộ này.").color(CHU_MO));
            }
            for l in dong {
                ui.label(egui::RichText::new(l.as_str()).size(12.0).color(CHU_PHU).family(egui::FontFamily::Monospace));
            }
        });
    }
}

const DON_VI_HOA: [&str; SO_BUOC] = ["Chương", "Chương", "Chương", "Chương", "Mục", "Chương", "Arc", "Chương", "Chương", "Phần", "Chương", "Mục", "Video", "File"];

/// Bước mặc định: đang chạy → lỗi đầu tiên → chờ → bước đầu tiên chưa xong.
fn buoc_mac_dinh(bo: &Bo) -> usize {
    [Tt::Chay, Tt::Loi, Tt::Cho, Tt::Chua].iter().find_map(|t| bo.buoc.iter().position(|b| b.tt == *t)).unwrap_or(0)
}

fn ve_o_buoc(ui: &Ui, rect: Rect, i: usize, b: &Buoc, chon: bool, hover: bool, t: f64) {
    let p = ui.painter();
    p.rect_filled(rect, 8.0, if hover { MAT_3 } else { MAT_2 });
    p.rect_stroke(rect, 8.0, if chon { Stroke::new(2.0, NHAN) } else { Stroke::new(1.0, VIEN) }, StrokeKind::Inside);
    p.rect_filled(Rect::from_min_size(rect.min + Vec2::new(10.0, 10.0), Vec2::new(rect.width() - 20.0, 6.0)), 3.0, mau_dong(b.tt, t));
    ve_chu(p, rect.min + Vec2::new(10.0, 24.0), &format!("{}. {}", i + 1, TEN_NGAN[i]), 13.0, CHU, rect.width() - 20.0);
    let so = if b.tt == Tt::BoQua { "bỏ qua".to_owned() } else if i == 9 || i == 12 || i == 13 { format!("{} {}", b.tong, DON_VI[i]) } else { format!("{}/{}", b.xong, b.tong) };
    ve_chu(p, rect.min + Vec2::new(10.0, 44.0), &so, 12.5, mau_tt_chu(b.tt), rect.width() - 20.0);
}

fn mau_tt_chu(tt: Tt) -> egui::Color32 {
    if matches!(tt, Tt::Chua | Tt::BoQua) { CHU_PHU } else { mau_tt(tt) }
}

#[allow(clippy::too_many_arguments)]
fn ve_dong_bang(ui: &mut Ui, d: &du_lieu::Dong, k: usize, w: f32, cot: &[f32; 4], nay: f64, thu_lai: &mut Vec<String>, co_xem: bool, xem: &mut Option<String>) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::hover());
    let p = ui.painter();
    if k % 2 == 0 {
        p.rect_filled(rect, 4.0, MAT_2);
    }
    let y = rect.center().y - 8.0;
    ve_chu(p, Pos2::new(rect.min.x + 8.0 + cot[0], y), &d.ten, 13.0, CHU, cot[1] - 8.0);
    p.circle_filled(Pos2::new(rect.min.x + 14.0 + cot[1], rect.center().y), 4.0, mau_dong(d.tt, ui.input(|i| i.time)));
    ve_chu(p, Pos2::new(rect.min.x + 26.0 + cot[1], y), ten_tt(d.tt), 13.0, mau_tt_chu(d.tt), cot[2] - cot[1] - 30.0);
    let tg = if d.tt == Tt::Chay { format!("đã chạy {}", khoang(nay - d.gio)) } else if d.gio > 0.0 { truoc(d.gio) } else { String::new() };
    ve_chu(p, Pos2::new(rect.min.x + 8.0 + cot[2], y), &tg, 12.5, CHU_PHU, cot[3] - cot[2] - 8.0);
    let hanh = 170.0;
    ve_chu(p, Pos2::new(rect.min.x + 8.0 + cot[3], y), &d.ly_do, 12.5, if d.tt == Tt::Loi { DO } else { CHU_PHU }, w - cot[3] - hanh - 16.0);
    let nut_xem = Rect::from_min_size(Pos2::new(rect.max.x - hanh, rect.min.y + 3.0), Vec2::new(70.0, 26.0));
    if co_xem {
        if ui.put(nut_xem, egui::Button::new("Xem").small()).on_hover_text("Mở chương này ở Màn Xem chương").clicked() {
            *xem = Some(d.ten.clone());
        }
    } else {
        ui.scope(|ui| {
            ui.disable();
            ui.put(nut_xem, egui::Button::new("Xem").small()).on_disabled_hover_text(MAN_SAU);
        });
    }
    if d.viec.is_some() && matches!(d.tt, Tt::Loi | Tt::Cho) {
        let nut_thu = Rect::from_min_size(Pos2::new(rect.max.x - hanh + 78.0, rect.min.y + 3.0), Vec2::new(80.0, 26.0));
        if ui.put(nut_thu, egui::Button::new("Thử lại").small()).clicked() {
            thu_lai.extend(d.viec.clone());
        }
    }
}
