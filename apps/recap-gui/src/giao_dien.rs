//! Màu + widget nhỏ dùng chung. Màu khai báo 1 chỗ (không rải hex trong màn); nền tối theo `ui-design-standards.md`:
//! mặt càng cao càng sáng, không dùng bóng đổ; đỏ/xanh/vàng chỉ đúng nghĩa trạng thái.
use egui::{Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::du_lieu::{Tt, bay_gio};

pub const NEN: Color32 = Color32::from_rgb(0x0e, 0x10, 0x14);
pub const MAT_1: Color32 = Color32::from_rgb(0x15, 0x18, 0x1f);
pub const MAT_2: Color32 = Color32::from_rgb(0x1b, 0x1f, 0x28);
pub const MAT_3: Color32 = Color32::from_rgb(0x24, 0x29, 0x34);
pub const VIEN: Color32 = Color32::from_rgb(0x2c, 0x32, 0x3f);
pub const CHU: Color32 = Color32::from_rgb(0xe4, 0xe7, 0xec);
pub const CHU_PHU: Color32 = Color32::from_rgb(0x9a, 0xa3, 0xb2);
pub const CHU_MO: Color32 = Color32::from_rgb(0x6b, 0x74, 0x84);
pub const NHAN: Color32 = Color32::from_rgb(0x7c, 0x8c, 0xff);
pub const XANH: Color32 = Color32::from_rgb(0x34, 0xc7, 0x7b);
pub const XANH_NHAT: Color32 = Color32::from_rgb(0x9a, 0xf0, 0xc0);
pub const XAM: Color32 = Color32::from_rgb(0x3b, 0x42, 0x50);
pub const DO: Color32 = Color32::from_rgb(0xf0, 0x5d, 0x5d);
pub const VANG: Color32 = Color32::from_rgb(0xf2, 0xb8, 0x4b);

pub fn mau_tt(tt: Tt) -> Color32 {
    match tt {
        Tt::Xong => XANH,
        Tt::Chay => XANH_NHAT,
        Tt::Chua => XAM,
        Tt::Loi => DO,
        Tt::Cho => VANG,
        Tt::BoQua => MAT_3,
    }
}

pub fn ten_tt(tt: Tt) -> &'static str {
    match tt {
        Tt::Xong => "Xong",
        Tt::Chay => "Đang chạy",
        Tt::Chua => "Chưa",
        Tt::Loi => "Lỗi",
        Tt::Cho => "Chờ",
        Tt::BoQua => "Bỏ qua",
    }
}

/// Xanh nhạt nhấp nháy khi đang chạy (đổi độ sáng theo giờ; gọi `repaint_after` ở nơi vẽ).
pub fn mau_dong(tt: Tt, thoi_gian: f64) -> Color32 {
    let c = mau_tt(tt);
    if tt != Tt::Chay {
        return c;
    }
    let k = 0.65 + 0.35 * ((thoi_gian * 3.0).sin() as f32 * 0.5 + 0.5);
    Color32::from_rgb((c.r() as f32 * k) as u8, (c.g() as f32 * k) as u8, (c.b() as f32 * k) as u8)
}

pub fn dat_kieu(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = NEN;
    v.window_fill = MAT_2;
    v.extreme_bg_color = MAT_1;
    v.faint_bg_color = MAT_1;
    v.override_text_color = Some(CHU);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, VIEN);
    v.widgets.inactive.weak_bg_fill = MAT_3;
    v.widgets.inactive.bg_fill = MAT_3;
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x30, 0x37, 0x46);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x3a, 0x42, 0x55);
    v.selection.bg_fill = Color32::from_rgb(0x3b, 0x45, 0x8f);
    v.window_stroke = Stroke::new(1.0, VIEN);
    ctx.options_mut(|o| o.theme_preference = egui::ThemePreference::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, v);
    nap_font(ctx);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 8.0);
        s.spacing.button_padding = Vec2::new(12.0, 6.0);
    });
}

/// Chữ 1 dòng cắt "…" ở bề rộng `rong`; trả về bề rộng đã vẽ.
pub fn ve_chu(p: &egui::Painter, vi_tri: Pos2, chu: &str, co: f32, mau: Color32, rong: f32) -> f32 {
    let mut job = egui::text::LayoutJob::simple_singleline(chu.to_owned(), FontId::proportional(co), mau);
    job.wrap.max_width = rong.max(1.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    let g = p.layout_job(job);
    let w = g.size().x;
    p.galley(vi_tri, g, mau);
    w
}

/// Nhãn nhỏ bo tròn; trả bề rộng.
pub fn nhan(p: &egui::Painter, goc: Pos2, chu: &str, mau: Color32) -> f32 {
    let g = p.layout_no_wrap(chu.to_owned(), FontId::proportional(11.0), mau);
    let r = Rect::from_min_size(goc, Vec2::new(g.size().x + 12.0, 18.0));
    p.rect_filled(r, 9.0, MAT_3);
    p.galley(Pos2::new(r.min.x + 6.0, r.min.y + (18.0 - g.size().y) / 2.0), g, mau);
    r.width()
}

/// Công tắc bật/tắt (không có sẵn trong egui). Trả Response; `clicked()` = người dùng bấm.
pub fn cong_tac(ui: &mut Ui, bat: bool, chu: &str) -> Response {
    let g = ui.painter().layout_no_wrap(chu.to_owned(), FontId::proportional(13.0), CHU);
    let (rect, r) = ui.allocate_exact_size(Vec2::new(36.0 + 8.0 + g.size().x, 24.0), Sense::click());
    let nen = Rect::from_min_size(rect.min + Vec2::new(0.0, 2.0), Vec2::new(36.0, 20.0));
    let p = ui.painter();
    p.rect_filled(nen, 10.0, if bat { NHAN } else { XAM });
    let x = if bat { nen.max.x - 10.0 } else { nen.min.x + 10.0 };
    p.circle_filled(Pos2::new(x, nen.center().y), 7.0, Color32::WHITE);
    p.galley(Pos2::new(nen.max.x + 8.0, rect.center().y - g.size().y / 2.0), g, CHU);
    if r.hovered() {
        p.rect_stroke(nen, 10.0, Stroke::new(1.0, CHU_PHU), StrokeKind::Outside);
    }
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn nut(ui: &mut Ui, chu: &str, chinh: bool, bat: bool) -> Response {
    let mut b = egui::Button::new(egui::RichText::new(chu).size(13.0).color(if chinh { Color32::WHITE } else { CHU }));
    if chinh {
        b = b.fill(Color32::from_rgb(0x4a, 0x58, 0xd6));
    }
    ui.add_enabled(bat, b.min_size(Vec2::new(0.0, 32.0)))
}

/// "3′ trước" / "2 giờ trước" / "chưa có".
pub fn truoc(epoch: f64) -> String {
    if epoch <= 0.0 {
        return "chưa có".into();
    }
    let s = (bay_gio() - epoch).max(0.0) as u64;
    match s {
        0..=59 => "vừa xong".into(),
        60..=3599 => format!("{}′ trước", s / 60),
        3600..=86399 => format!("{} giờ trước", s / 3600),
        _ => format!("{} ngày trước", s / 86400),
    }
}

/// Khoảng thời gian dạng "1h12′" / "5′" / "40 giây".
pub fn khoang(giay: f64) -> String {
    let s = giay.max(0.0) as u64;
    if s >= 3600 { format!("{}h{:02}′", s / 3600, (s % 3600) / 60) } else if s >= 60 { format!("{}′", s / 60) } else { format!("{s} giây") }
}

pub fn chu_giua(p: &egui::Painter, tam: Pos2, chu: &str, co: f32, mau: Color32) {
    p.text(tam, Align2::CENTER_CENTER, chu, FontId::proportional(co), mau);
}

/// Font mặc định của egui thiếu dấu chồng tiếng Việt (ễ, ế, ặ…): nạp font hệ thống có đủ glyph, đặt lên đầu.
fn nap_font(ctx: &egui::Context) {
    const UNG_VIEN: [&str; 6] = [
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ];
    let Some(du_lieu) = UNG_VIEN.iter().find_map(|p| std::fs::read(p).ok()) else { return };
    let mut f = egui::FontDefinitions::default();
    f.font_data.insert("viet".into(), std::sync::Arc::new(egui::FontData::from_owned(du_lieu)));
    for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        f.families.entry(fam).or_default().insert(0, "viet".into());
    }
    ctx.set_fonts(f);
}
