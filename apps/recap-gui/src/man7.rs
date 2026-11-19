//! Màn 7 — Máy & hàng việc (spec `07-may-hang-viec.md`, mục Màn hình): thẻ mỗi máy (trực tuyến, CPU, RAM, ổ, phiên bản, từng luồng
//! đang làm), agy (tài khoản + quota từng nhóm + việc cất), cảnh báo chung, bảng hàng việc lọc + Dừng/Thử lại/Đưa lên đầu/Huỷ.
//! Chỉ ĐỌC `trang-thai.json`, `may-khac/*.json`, `hang-viec/`; chỉ GHI `lenh/` (đúng `DINH-DANG.md`).
use egui::{Color32, RichText, Sense, Ui, Vec2};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::du_lieu::{Viec, bay_gio, doc_json, gio_phut};
use crate::giao_dien::*;
use crate::lenh;
use crate::{App, Man};

#[derive(Clone, Default, Debug)]
pub struct DangLam {
    pub id: String,
    pub loai: String,
    pub kieu: String,
    pub bo: String,
    pub chuong: String,
    pub chay_giay: f64,
    pub tien_do: Option<f64>,
    pub lan_thu: u32,
}

#[derive(Clone, Default, Debug)]
pub struct MayThe {
    pub ten: String,
    pub la_may_nay: bool,
    pub truc_tuyen: bool,
    pub gio: f64,
    pub mat_tu: Option<f64>,
    pub cpu: Option<f64>,
    pub ram: Option<f64>,
    pub o: Option<f64>,
    pub phien_ban: String,
    pub ap_luc: String,
    pub he_dieu_hanh: String,
    pub luong: Vec<(String, u32, u32, u32)>,
    pub dang_lam: Vec<DangLam>,
    pub hang: BTreeMap<String, u32>,
    pub tai_khoan: String,
    pub quota: Vec<(String, Vec<(f64, f64)>)>,
    pub agy_dang_chay: u32,
    pub agy_cat: u32,
    pub canh_bao: Vec<String>,
}

fn so(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(0.0)
}
fn chu(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_owned()
}

/// 1 file trạng thái (của máy này hoặc `may-khac/<tên>.json`) → thẻ. Máy khác: tự tính online từ `nhan_luc`, không tin cờ cũ.
pub fn doc_the(j: &Value, la_may_nay: bool, bay_gio: f64) -> MayThe {
    let tn = j.get("tai_nguyen").cloned().unwrap_or(Value::Null);
    let n = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
    let (truc_tuyen, mat_tu) = if la_may_nay {
        let on = bay_gio - so(j, "gio") <= 15.0;
        (on, if on { None } else { Some(so(j, "gio")) })
    } else {
        let nl = so(j, "nhan_luc");
        let on = bay_gio - nl <= 90.0 && j.get("truc_tuyen").and_then(Value::as_bool).unwrap_or(true);
        (on, j.get("mat_ket_noi_tu").and_then(Value::as_f64).or(if on { None } else { Some(nl) }))
    };
    MayThe {
        ten: if chu(j, "ten").is_empty() { chu(j, "may") } else { chu(j, "ten") },
        la_may_nay,
        truc_tuyen,
        gio: if la_may_nay { so(j, "gio") } else { so(j, "nhan_luc") },
        mat_tu,
        cpu: tn.get("cpu_pct").and_then(Value::as_f64),
        ram: tn.get("ram_trong_gb").and_then(Value::as_f64),
        o: tn.get("o_trong_gb").and_then(Value::as_f64),
        phien_ban: chu(j, "phien_ban"),
        ap_luc: chu(&tn, "ap_luc"),
        he_dieu_hanh: chu(j, "he_dieu_hanh"),
        luong: j.get("luong").and_then(Value::as_object).map(|l| l.iter().map(|(k, v)| (k.clone(), n(v, "toi_da"), n(v, "hieu_luc"), n(v, "dang_chay"))).collect()).unwrap_or_default(),
        dang_lam: j
            .get("dang_lam")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|d| DangLam { id: chu(d, "id"), loai: chu(d, "loai"), kieu: chu(d, "kieu"), bo: chu(d, "bo"), chuong: chu(d, "chuong"), chay_giay: so(d, "chay_giay"), tien_do: d.get("tien_do").and_then(Value::as_f64), lan_thu: so(d, "lan_thu") as u32 })
                    .collect()
            })
            .unwrap_or_default(),
        hang: j.get("hang").and_then(Value::as_object).map(|h| h.iter().map(|(k, v)| (k.clone(), v.as_u64().unwrap_or(0) as u32)).collect()).unwrap_or_default(),
        tai_khoan: j.pointer("/agy/tai_khoan").and_then(Value::as_str).unwrap_or("").to_owned(),
        quota: j
            .pointer("/agy/quota")
            .and_then(Value::as_object)
            .map(|q| q.iter().map(|(k, a)| (k.clone(), a.as_array().map(|l| l.iter().filter_map(|x| Some((x.get(0)?.as_f64()?, x.get(1)?.as_f64()?))).collect()).unwrap_or_default())).collect())
            .unwrap_or_default(),
        agy_dang_chay: j.pointer("/agy/dang_chay").and_then(Value::as_u64).unwrap_or(0) as u32,
        agy_cat: j.pointer("/agy/cat_cho_quota").and_then(Value::as_u64).unwrap_or(0) as u32,
        canh_bao: j.get("canh_bao").and_then(Value::as_array).map(|a| a.iter().map(|c| chu(c, "noi_dung")).filter(|c| !c.is_empty()).collect()).unwrap_or_default(),
    }
}

/// Máy này trước, máy khác theo tên. Máy này không có file thì trả thẻ "chưa chạy" (không đoán).
pub fn doc_cac_may(goc: &Path, bay_gio: f64) -> Vec<MayThe> {
    let mut v = vec![];
    match doc_json(&goc.join("trang-thai.json")) {
        Some(j) => v.push(doc_the(&j, true, bay_gio)),
        None => v.push(MayThe { ten: "Máy này".into(), la_may_nay: true, ..MayThe::default() }),
    }
    let mut khac: Vec<MayThe> = std::fs::read_dir(goc.join("may-khac")).into_iter().flatten().flatten().filter_map(|e| doc_json(&e.path())).map(|j| doc_the(&j, false, bay_gio)).collect();
    khac.sort_by(|a, b| a.ten.cmp(&b.ten));
    v.extend(khac);
    v
}

pub struct ManMay {
    pub may: Vec<MayThe>,
    pub loc_may: String,
    pub loc_bo: String,
    pub loc_loai: String,
    pub loc_tt: String,
    pub xong_huy: bool,
    lan_cuoi: Option<Instant>,
}

impl ManMay {
    pub fn moi() -> Self {
        Self { may: vec![], loc_may: "Tất cả".into(), loc_bo: "Tất cả".into(), loc_loai: "Tất cả".into(), loc_tt: "Tất cả".into(), xong_huy: false, lan_cuoi: None }
    }
}

fn ten_loai(l: &str, kieu: &str) -> String {
    let t = match l {
        "tai" => "Tải",
        "ocr" => "OCR",
        "xoa_chu" => "Xoá chữ",
        "do_khung" => "Dò khung",
        "agy" => "agy",
        "giong" => "Giọng",
        "dung_timeline" => "Dựng timeline",
        "xuat" => "Xuất",
        x => x,
    };
    if kieu.is_empty() { t.to_owned() } else { format!("{t} · {kieu}") }
}

fn ten_tt(t: &str) -> (&'static str, Color32) {
    match t {
        "cho" => ("Chờ", VANG),
        "chay" => ("Đang chạy", XANH_NHAT),
        "cat_quota" => ("Chờ quota", VANG),
        "xong" => ("Xong", XANH),
        "loi" => ("Lỗi", DO),
        "dung" => ("Đã dừng", CHU_PHU),
        "huy" => ("Đã huỷ", CHU_MO),
        _ => ("?", CHU_MO),
    }
}

fn ten_cuoi(p: &str) -> String {
    Path::new(p).file_name().map_or_else(|| p.to_owned(), |n| n.to_string_lossy().into_owned())
}

/// Dòng bảng hàng việc: việc của máy này (có nút) + luồng đang chạy của máy khác (chỉ xem).
#[derive(Clone)]
struct Dong {
    may: String,
    id: String,
    loai: String,
    kieu: String,
    bo: String,
    chuong: String,
    tt: String,
    lan: u32,
    tien_do: Option<f64>,
    giay: f64,
    ly_do: String,
    dieu_khien: bool,
}

fn dong_tu_viec(may: &str, v: &Viec) -> Dong {
    let giay = if v.trang_thai == "chay" && v.gio_bat_dau > 0.0 { bay_gio() - v.gio_bat_dau } else { 0.0 };
    let ly_do = if v.trang_thai == "cat_quota" && v.cho_toi > 0.0 { format!("{} (hồi {}{})", v.ly_do, gio_phut(v.cho_toi), v.cho_tk.as_deref().map_or_else(String::new, |t| format!(", tk {t}"))) } else { v.ly_do.clone() };
    Dong { may: may.to_owned(), id: v.id.clone(), loai: v.loai.clone(), kieu: v.kieu.clone(), bo: ten_cuoi(&v.bo), chuong: v.chuong.clone().unwrap_or_default(), tt: v.trang_thai.clone(), lan: v.lan_thu, tien_do: v.tien_do, giay, ly_do, dieu_khien: true }
}

fn the_may(ui: &mut Ui, t: &MayThe, ma_goc: &str) {
    let rong = ((ui.available_width() - 16.0) / 3.0).clamp(300.0, 520.0);
    egui::Frame::new().fill(MAT_2).corner_radius(10.0).inner_margin(egui::Margin::same(12)).stroke(egui::Stroke::new(1.0, if t.truc_tuyen { VIEN } else { VANG })).show(ui, |ui| {
        ui.set_width(rong - 24.0);
        ui.horizontal(|ui| {
            ui.colored_label(if t.truc_tuyen { XANH } else { VANG }, "●");
            ui.label(RichText::new(&t.ten).size(16.0).strong());
            if t.la_may_nay {
                ui.label(RichText::new("máy này").size(11.0).color(CHU_MO));
            }
        });
        if t.truc_tuyen {
            ui.label(RichText::new(format!("Trực tuyến · cập nhật {}", truoc(t.gio))).size(12.0).color(CHU_PHU));
        } else {
            ui.label(RichText::new(format!("{} mất kết nối từ {}", t.ten, t.mat_tu.map_or_else(|| "?".into(), gio_phut))).size(12.0).color(VANG));
        }
        let (o_mau, ram_mau) = (if t.o.is_some_and(|o| o < 20.0) { DO } else { CHU_PHU }, if t.ap_luc == "cao" || t.ap_luc == "khan" { VANG } else { CHU_PHU });
        let gb = |x: Option<f64>| x.map_or_else(|| "—".to_owned(), |v| format!("{v:.1} GB"));
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("CPU {}", t.cpu.map_or_else(|| "—".into(), |c| format!("{c:.0}%")))).size(12.0).color(CHU_PHU));
            ui.label(RichText::new(format!("RAM trống {}", gb(t.ram))).size(12.0).color(ram_mau));
            ui.label(RichText::new(format!("Ổ trống {}{}", gb(t.o), if t.o.is_some_and(|o| o < 20.0) { " ⚠ < 20 GB" } else { "" })).size(12.0).color(o_mau));
        });
        let lech = !t.la_may_nay && !ma_goc.is_empty() && !t.phien_ban.is_empty() && t.phien_ban != ma_goc;
        ui.label(RichText::new(format!("Code {}{}", if t.phien_ban.is_empty() { "?" } else { &t.phien_ban }, if lech { format!(" — LỆCH Mac ({ma_goc})") } else { String::new() })).size(12.0).color(if lech { DO } else { CHU_PHU }));
        if !t.ap_luc.is_empty() && t.ap_luc != "thuong" {
            ui.label(RichText::new(format!("Áp lực máy: {} — dịch vụ đã giảm luồng", t.ap_luc)).size(12.0).color(VANG));
        }
        if !t.luong.is_empty() {
            let l: Vec<String> = t.luong.iter().map(|(k, td, hl, dc)| format!("{k} {dc}/{hl}{}", if hl < td { format!(" (tối đa {td})") } else { String::new() })).collect();
            ui.label(RichText::new(format!("Luồng: {}", l.join(" · "))).size(12.0).color(CHU_PHU));
        }
        ui.add_space(4.0);
        if t.dang_lam.is_empty() {
            ui.label(RichText::new("Không có việc đang chạy").size(12.0).color(CHU_MO));
        }
        for d in t.dang_lam.iter().take(6) {
            let td = d.tien_do.map_or_else(String::new, |p| format!(" · {:.0}%", p * 100.0));
            ui.label(RichText::new(format!("▸ {} · {} {} · {}{td}", ten_loai(&d.loai, &d.kieu), d.bo, d.chuong, khoang(d.chay_giay))).size(12.0).color(XANH_NHAT));
        }
        if t.dang_lam.len() > 6 {
            ui.label(RichText::new(format!("… và {} luồng nữa", t.dang_lam.len() - 6)).size(11.0).color(CHU_MO));
        }
        if !t.hang.is_empty() {
            let h = |k: &str| t.hang.get(k).copied().unwrap_or(0);
            ui.label(RichText::new(format!("Hàng: {} chạy · {} chờ · {} chờ quota · {} lỗi", h("chay"), h("cho"), h("cat_quota"), h("loi"))).size(12.0).color(if h("loi") > 0 { DO } else { CHU_PHU }));
        }
    });
}

impl App {
    pub fn mo_may(&mut self) {
        self.may = Some(Box::new(ManMay::moi()));
        self.man = Man::May;
    }

    pub fn man_may(&mut self, ui: &mut Ui) {
        let Some(mut m) = self.may.take() else {
            self.man = Man::Thu;
            return;
        };
        let nay = bay_gio();
        if m.lan_cuoi.is_none_or(|t| t.elapsed() > Duration::from_millis(1500)) {
            m.may = doc_cac_may(&self.duong.goc_dv, nay);
            m.lan_cuoi = Some(Instant::now());
        }
        let mut di = false;
        egui::Panel::top("m_tren").show_separator_line(false).frame(egui::Frame::new().fill(MAT_1).inner_margin(egui::Margin::symmetric(16, 8))).show(ui, |ui| {
            ui.horizontal(|ui| {
                if nut(ui, "← Thư viện", false, true).clicked() {
                    di = true;
                }
                ui.label(RichText::new("Máy & hàng việc").size(15.0).strong());
                let h = |k: &str| self.snap.dv.hang.get(k).copied().unwrap_or(0);
                ui.label(RichText::new(format!("{} chạy · {} chờ · {} chờ quota · {} lỗi", h("chay"), h("cho"), h("cat_quota"), h("loi"))).size(12.0).color(CHU_PHU));
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(NEN).inner_margin(egui::Margin::same(16))).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("m_cuon").auto_shrink([false, false]).show(ui, |ui| self.noi_dung_may(ui, &mut m));
        });
        if di {
            self.man = Man::Thu;
        } else {
            self.may = Some(m);
        }
        ui.ctx().request_repaint_after(Duration::from_millis(500));
    }

    fn noi_dung_may(&mut self, ui: &mut Ui, m: &mut ManMay) {
        let ma_goc = m.may.iter().find(|t| t.ten.to_lowercase().contains("mac")).or(m.may.first()).map(|t| t.phien_ban.clone()).unwrap_or_default();
        // 1. thẻ mỗi máy
        ui.horizontal_top(|ui| {
            for t in &m.may {
                ui.vertical(|ui| the_may(ui, t, &ma_goc));
            }
        });
        // 2. agy (máy này)
        ui.add_space(10.0);
        ui.label(RichText::new("agy").size(15.0).strong());
        let nay = m.may.first().cloned().unwrap_or_default();
        let cat: Vec<String> = self.snap.viec.iter().filter(|v| v.trang_thai == "cat_quota").map(|v| v.id.clone()).collect();
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Tài khoản: {}", if nay.tai_khoan.is_empty() { "chưa rõ" } else { &nay.tai_khoan }));
            ui.label(RichText::new(format!("· {} đang chạy · {} chờ · {} cất chờ quota", nay.agy_dang_chay, self.snap.viec.iter().filter(|v| v.loai == "agy" && v.trang_thai == "cho").count(), cat.len())).color(CHU_PHU));
            if nut(ui, "Đổi tài khoản", false, true).on_hover_text("Hướng dẫn đăng nhập agy; dịch vụ tự chạy lại việc cất khi tài khoản đổi").clicked() {
                self.hop_tai_khoan = true;
            }
            if nut(ui, "Lấy việc cất ngay", false, !cat.is_empty()).on_hover_text("Đưa mọi việc đang cất chờ quota về hàng chờ để thử lại ngay").clicked() {
                let kq = cat.iter().try_for_each(|id| lenh::gui_lenh(&self.duong.goc_dv, "thu_lai", id));
                self.gui(kq, &format!("Đã gửi lệnh thử lại cho {} việc cất", cat.len()));
            }
        });
        if nay.quota.is_empty() {
            ui.label(RichText::new("Chưa đọc được quota (dịch vụ chưa báo /usage).").size(12.0).color(CHU_MO));
        }
        ui.horizontal_wrapped(|ui| {
            for (nhom, ds) in &nay.quota {
                for (p, hoi) in ds {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(210.0, 44.0), Sense::hover());
                    let pt = ui.painter();
                    pt.rect_filled(r, 6.0, MAT_2);
                    ve_chu(pt, r.min + Vec2::new(10.0, 6.0), &format!("{nhom} · {p:.0}%"), 12.0, CHU, 190.0);
                    let b = egui::Rect::from_min_size(r.min + Vec2::new(10.0, 24.0), Vec2::new(190.0, 4.0));
                    pt.rect_filled(b, 2.0, MAT_1);
                    pt.rect_filled(egui::Rect::from_min_size(b.min, Vec2::new(b.width() * (*p as f32 / 100.0).clamp(0.0, 1.0), 4.0)), 2.0, if *p >= 90.0 { DO } else if *p >= 70.0 { VANG } else { XANH });
                    ve_chu(pt, r.min + Vec2::new(10.0, 30.0), &format!("hồi {}", gio_phut(*hoi)), 11.0, CHU_MO, 190.0);
                }
            }
        });

        // 3. cảnh báo
        let cb: Vec<(String, String)> = m.may.iter().flat_map(|t| t.canh_bao.iter().map(|c| (t.ten.clone(), c.clone()))).collect();
        ui.add_space(10.0);
        ui.label(RichText::new(format!("Cảnh báo ({})", cb.len())).size(15.0).strong());
        if cb.is_empty() {
            ui.label(RichText::new("Không có cảnh báo.").size(12.0).color(XANH));
        }
        for (may, c) in &cb {
            ui.label(RichText::new(format!("⚠ {may}: {c}")).size(12.0).color(VANG));
        }

        // 4. hàng việc
        ui.add_space(10.0);
        ui.label(RichText::new("Hàng việc").size(15.0).strong());
        let mut dong: Vec<Dong> = self.snap.viec.iter().map(|v| dong_tu_viec(&nay.ten, v)).collect();
        for t in m.may.iter().filter(|t| !t.la_may_nay) {
            dong.extend(t.dang_lam.iter().map(|d| Dong { may: t.ten.clone(), id: d.id.clone(), loai: d.loai.clone(), kieu: d.kieu.clone(), bo: d.bo.clone(), chuong: d.chuong.clone(), tt: "chay".into(), lan: d.lan_thu, tien_do: d.tien_do, giay: d.chay_giay, ly_do: "máy khác — chỉ xem".into(), dieu_khien: false }));
        }
        let chon = |ui: &mut Ui, nhan: &str, gt: &mut String, ds: Vec<String>| {
            ui.label(RichText::new(nhan).size(12.0).color(CHU_MO));
            egui::ComboBox::from_id_salt(nhan).selected_text(gt.as_str()).show_ui(ui, |ui| {
                for d in std::iter::once("Tất cả".to_owned()).chain(ds) {
                    ui.selectable_value(gt, d.clone(), d);
                }
            });
        };
        let uniq = |f: &dyn Fn(&Dong) -> String| -> Vec<String> {
            let mut v: Vec<String> = dong.iter().map(f).filter(|s| !s.is_empty()).collect();
            v.sort();
            v.dedup();
            v
        };
        let (ds_may, ds_bo, ds_loai) = (uniq(&|d| d.may.clone()), uniq(&|d| d.bo.clone()), uniq(&|d| d.loai.clone()));
        ui.horizontal_wrapped(|ui| {
            chon(ui, "Máy", &mut m.loc_may, ds_may);
            chon(ui, "Bộ", &mut m.loc_bo, ds_bo);
            chon(ui, "Loại", &mut m.loc_loai, ds_loai);
            chon(ui, "Trạng thái", &mut m.loc_tt, ["cho", "chay", "cat_quota", "loi", "dung", "xong", "huy"].map(String::from).to_vec());
            ui.checkbox(&mut m.xong_huy, "Hiện cả việc xong/huỷ");
        });
        let mut hien: Vec<&Dong> = dong
            .iter()
            .filter(|d| (m.loc_may == "Tất cả" || d.may == m.loc_may) && (m.loc_bo == "Tất cả" || d.bo == m.loc_bo) && (m.loc_loai == "Tất cả" || d.loai == m.loc_loai) && (m.loc_tt == "Tất cả" || d.tt == m.loc_tt) && (m.xong_huy || m.loc_tt != "Tất cả" || !matches!(d.tt.as_str(), "xong" | "huy")))
            .collect();
        let thu_tu = |t: &str| ["chay", "loi", "cat_quota", "cho", "dung", "xong", "huy"].iter().position(|x| *x == t).unwrap_or(9);
        hien.sort_by_key(|d| thu_tu(&d.tt));
        ui.label(RichText::new(format!("{} việc", hien.len())).size(12.0).color(CHU_PHU));
        let mut lenh_gui: Option<(&'static str, String)> = None;
        egui::Grid::new("m_hang").num_columns(8).spacing([14.0, 6.0]).striped(true).show(ui, |ui| {
            for h in ["Máy", "Việc", "Bộ · chương", "Trạng thái", "Lần", "Thời gian", "Lý do", ""] {
                ui.label(RichText::new(h).size(11.0).color(CHU_MO));
            }
            ui.end_row();
            for d in hien.iter().take(300) {
                let (tt, mau) = ten_tt(&d.tt);
                ui.label(RichText::new(&d.may).size(12.0));
                ui.label(RichText::new(ten_loai(&d.loai, &d.kieu)).size(12.0));
                ui.label(RichText::new(format!("{} {}", d.bo, d.chuong)).size(12.0));
                ui.label(RichText::new(match d.tien_do { Some(p) if d.tt == "chay" => format!("{tt} {:.0}%", p * 100.0), _ => tt.to_owned() }).size(12.0).color(mau));
                ui.label(RichText::new(d.lan.to_string()).size(12.0).color(if d.lan >= 3 { DO } else { CHU_PHU }));
                ui.label(RichText::new(if d.giay > 0.0 { khoang(d.giay) } else { String::new() }).size(12.0).color(CHU_PHU));
                ui.add(egui::Label::new(RichText::new(&d.ly_do).size(11.0).color(CHU_PHU)).truncate()).on_hover_text(&d.ly_do);
                ui.horizontal(|ui| {
                    if !d.dieu_khien {
                        return;
                    }
                    let (chay, cho) = (d.tt == "chay", d.tt == "cho");
                    if nut(ui, "Dừng", false, chay).clicked() {
                        lenh_gui = Some(("dung", d.id.clone()));
                    }
                    if nut(ui, "Thử lại", false, matches!(d.tt.as_str(), "loi" | "dung" | "huy" | "cat_quota")).clicked() {
                        lenh_gui = Some(("thu_lai", d.id.clone()));
                    }
                    if nut(ui, "Đưa lên đầu", false, cho || d.tt == "cat_quota").clicked() {
                        lenh_gui = Some(("len_dau", d.id.clone()));
                    }
                    if nut(ui, "Huỷ", false, !matches!(d.tt.as_str(), "huy" | "xong")).on_hover_text("Vào Thùng rác việc — Thử lại để lấy ra").clicked() {
                        lenh_gui = Some(("huy", d.id.clone()));
                    }
                });
                ui.end_row();
            }
        });
        if let Some((l, id)) = lenh_gui {
            let r = lenh::gui_lenh(&self.duong.goc_dv, l, &id);
            if l == "dung" {
                self.dang_dung.insert(id);
            }
            self.gui(r, &format!("Đã gửi lệnh {l}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn may_khac_mat_ket_noi_trong_2_phut() {
        let j = json!({"ten": "PC", "truc_tuyen": true, "nhan_luc": 1000.0, "dang_lam": [{"id": "a", "loai": "xuat", "chay_giay": 5}]});
        assert!(doc_the(&j, false, 1060.0).truc_tuyen);
        let t = doc_the(&j, false, 1100.0);
        assert!(!t.truc_tuyen);
        assert_eq!(t.mat_tu, Some(1000.0));
        assert_eq!(t.dang_lam.len(), 1);
    }
}
