//! Recap Studio — Màn 1 Thư viện bộ + Màn 2 Bảng điều khiển bộ (spec `specs/app-moi/01`, `02`).
//!
//! Màn hình chỉ ĐỌC `bo.json` của các bộ và file trạng thái của dịch vụ nền; lệnh điều khiển gửi đúng cơ chế
//! `DINH-DANG.md` (`lenh/`, file mới trong `hang-viec/`). Thư mục: `RECAP_DU_LIEU` (các bộ, mặc định
//! `~/recap-studio-du-lieu`) và `RECAP_DV_GOC` (dịch vụ nền, mặc định `~/recap-studio-dich-vu`).
//!
//! `recap-gui [--goc <thư mục bộ>] [--dv <thư mục dịch vụ>] [--man bo:<tên>] [--buoc N] [--chup <file.png> [--sau <giây>]] [--do]`
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

mod chuong;
mod du_lieu;
mod giao_dien;
mod lenh;
mod man1;
mod man2;
mod man3;
mod man5;
mod man6;
mod man7;
mod timeline;

use du_lieu::{Bo, CanhBao, DichVu, Viec};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub struct Snap {
    pub co_du_lieu: bool,
    pub bo: Arc<Vec<Bo>>,
    pub viec: Arc<Vec<Viec>>,
    pub dv: DichVu,
    pub canh_bao: Arc<Vec<CanhBao>>,
    pub nhat_ky: Arc<Vec<String>>,
    pub ms_quet_bo: u128,
    pub gio_quet_bo: f64,
}

#[derive(Clone)]
pub struct Duong {
    pub goc_bo: Vec<PathBuf>,
    pub goc_dv: PathBuf,
}

fn nha() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map_or_else(|| PathBuf::from("."), PathBuf::from)
}

fn duong_dan(args: &Args) -> Duong {
    let goc_dv = args.dv.clone().or_else(|| std::env::var_os("RECAP_DV_GOC").map(PathBuf::from)).unwrap_or_else(|| nha().join("recap-studio-dich-vu"));
    let goc_bo = args.goc.clone().or_else(|| std::env::var_os("RECAP_DU_LIEU").map(PathBuf::from)).map(|p| vec![p]).unwrap_or_else(|| {
        let tu_cai_dat: Vec<PathBuf> = du_lieu::doc_json(&goc_dv.join("cai-dat.json"))
            .and_then(|j| j.get("bo_goc").and_then(|b| b.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(PathBuf::from)).collect()))
            .unwrap_or_default();
        if tu_cai_dat.is_empty() { vec![nha().join("recap-studio-du-lieu")] } else { tu_cai_dat }
    });
    Duong { goc_bo, goc_dv }
}

/// Luồng nền: đọc dịch vụ mỗi 2 giây, quét thư mục bộ mỗi 10 giây (hoặc ngay khi được gọi làm mới).
fn chay_quet(duong: Duong, ctx: egui::Context, tx: mpsc::Sender<Snap>, lam_moi: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let mut facts: Vec<Bo> = vec![];
        let (mut ms, mut gio_bo, mut lan_bo) = (0u128, 0.0, Instant::now() - Duration::from_secs(3600));
        loop {
            let nay = du_lieu::bay_gio();
            if lan_bo.elapsed() >= Duration::from_secs(10) || lam_moi.swap(false, Ordering::Relaxed) {
                let t = Instant::now();
                facts = du_lieu::quet_goc(&duong.goc_bo);
                (ms, gio_bo, lan_bo) = (t.elapsed().as_millis(), nay, Instant::now());
            }
            let viec = du_lieu::doc_hang_viec(&duong.goc_dv);
            let dv = du_lieu::doc_dich_vu(&duong.goc_dv, nay);
            let mut bo = facts.clone();
            let canh_bao = du_lieu::phu_tat_ca(&mut bo, &viec, &dv, nay);
            let nhat_ky = du_lieu::doc_nhat_ky(&duong.goc_dv, 600);
            let snap = Snap { co_du_lieu: true, bo: Arc::new(bo), viec: Arc::new(viec), dv, canh_bao: Arc::new(canh_bao), nhat_ky: Arc::new(nhat_ky), ms_quet_bo: ms, gio_quet_bo: gio_bo };
            if tx.send(snap).is_err() {
                return;
            }
            ctx.request_repaint();
            for _ in 0..8 {
                std::thread::sleep(Duration::from_millis(250));
                if lam_moi.load(Ordering::Relaxed) {
                    break;
                }
            }
        }
    });
}

pub enum Bia {
    Dang,
    Co(egui::TextureHandle),
    Hong,
}

type BiaRa = (PathBuf, Option<egui::ColorImage>);

/// Luồng giải mã ảnh bìa nhỏ (không chặn khung hình).
fn chay_bia(ctx: egui::Context) -> (mpsc::Sender<PathBuf>, mpsc::Receiver<BiaRa>) {
    let (tx_yeu_cau, rx_yeu_cau) = mpsc::channel::<PathBuf>();
    let (tx_ra, rx_ra) = mpsc::channel();
    std::thread::spawn(move || {
        for p in rx_yeu_cau {
            let anh = image::open(&p).ok().map(|a| {
                let t = a.thumbnail(160, 224).to_rgba8();
                egui::ColorImage::from_rgba_unmultiplied([t.width() as usize, t.height() as usize], t.as_raw())
            });
            if tx_ra.send((p, anh)).is_err() {
                return;
            }
            ctx.request_repaint();
        }
    });
    (tx_yeu_cau, rx_ra)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LocTt {
    Tat,
    DangLam,
    Ket,
    XongChuaDang,
    DaDang,
}

pub enum Man {
    Thu,
    Bo(PathBuf),
    Chuong(PathBuf, String),
    Timeline(PathBuf),
    Xuat(PathBuf),
    May,
}

#[derive(Default)]
pub struct HopMoi {
    pub ten: String,
    pub loai_manhwa: bool,
    pub may: String,
    pub nguon: String,
    pub noi_tiep_anime: bool,
    pub tu_dong: bool,
    pub loi: String,
}

#[derive(Default, Clone)]
pub struct Args {
    pub goc: Option<PathBuf>,
    pub dv: Option<PathBuf>,
    pub man: Option<String>,
    pub chup: Option<PathBuf>,
    pub sau: f32,
    pub chon_buoc: Option<usize>,
    pub do_dac: bool,
}

pub struct App {
    pub duong: Duong,
    pub snap: Snap,
    rx: mpsc::Receiver<Snap>,
    pub lam_moi: Arc<AtomicBool>,
    pub man: Man,
    pub tim: String,
    pub loc_tt: LocTt,
    pub loc_may: String,
    pub bia: HashMap<PathBuf, Bia>,
    bia_tx: mpsc::Sender<PathBuf>,
    bia_rx: mpsc::Receiver<BiaRa>,
    pub chon_buoc: Option<usize>,
    pub hop_moi: Option<HopMoi>,
    pub xac_nhan_rac: Option<PathBuf>,
    pub dang_dung: HashSet<String>,
    pub thong_bao: Option<(String, Instant)>,
    pub hien_het_canh_bao: bool,
    pub hop_tai_khoan: bool,
    pub loc_nhat_ky_buoc: bool,
    pub may_dang_mo: String,
    pub xem: Option<Box<man3::ManXem>>,
    pub tl: Option<Box<man5::ManTl>>,
    pub xuat: Option<Box<man6::ManXuat>>,
    pub may: Option<Box<man7::ManMay>>,
    pub tac_vu_tx: mpsc::Sender<String>,
    tac_vu_rx: mpsc::Receiver<String>,
    args: Args,
    bat_dau: Instant,
    da_do: bool,
    da_chup: Arc<Mutex<Option<String>>>,
    chup_da_xin: bool,
}

impl App {
    fn moi(cc: &eframe::CreationContext<'_>, args: Args) -> Self {
        giao_dien::dat_kieu(&cc.egui_ctx);
        let duong = duong_dan(&args);
        let (tx, rx) = mpsc::channel();
        let lam_moi = Arc::new(AtomicBool::new(false));
        chay_quet(duong.clone(), cc.egui_ctx.clone(), tx, Arc::clone(&lam_moi));
        let (bia_tx, bia_rx) = chay_bia(cc.egui_ctx.clone());
        let goc1 = duong.goc_bo.first().cloned().unwrap_or_default();
        let mut man = Man::Thu;
        let (mut xem, mut tl) = (None, None);
        if let Some(m) = args.man.as_deref() {
            if let Some(ten) = m.strip_prefix("bo:") {
                man = Man::Bo(goc1.join(ten));
            } else if let Some(r) = m.strip_prefix("chuong:") {
                // chuong:<tên bộ>:chNNN[:trang]
                let p: Vec<&str> = r.split(':').collect();
                if let [ten, ma, rest @ ..] = p.as_slice() {
                    let bo = goc1.join(ten);
                    xem = Some(Box::new(man3::ManXem::moi(&cc.egui_ctx, &bo, ma, rest.first().and_then(|t| t.parse::<usize>().ok()).map_or(0, |t| t.saturating_sub(1)))));
                    man = Man::Chuong(bo, (*ma).to_owned());
                }
            } else if let Some(r) = m.strip_prefix("timeline:") {
                let (ten, ma) = r.split_once(':').map_or((r, None), |(a, b)| (a, Some(b)));
                let bo = goc1.join(ten);
                tl = Some(Box::new(man5::ManTl::moi(&cc.egui_ctx, &bo, ma)));
                man = Man::Timeline(bo);
            }
        }
        let (tac_vu_tx, tac_vu_rx) = mpsc::channel();
        let (mut xuat, mut may) = (None, None);
        if let Some(ten) = args.man.as_deref().and_then(|m| m.strip_prefix("xuat:")) {
            let bo = goc1.join(ten);
            xuat = Some(Box::new(man6::ManXuat::moi(&bo, ten)));
            man = Man::Xuat(bo);
        } else if args.man.as_deref() == Some("may") {
            may = Some(Box::new(man7::ManMay::moi()));
            man = Man::May;
        }
        Self {
            chon_buoc: args.chon_buoc,
            duong,
            snap: Snap::default(),
            rx,
            lam_moi,
            man,
            tim: String::new(),
            loc_tt: LocTt::Tat,
            loc_may: "Tất cả".into(),
            bia: HashMap::new(),
            bia_tx,
            bia_rx,
            hop_moi: None,
            xac_nhan_rac: None,
            dang_dung: HashSet::new(),
            thong_bao: None,
            hien_het_canh_bao: false,
            hop_tai_khoan: false,
            loc_nhat_ky_buoc: true,
            may_dang_mo: "Mac".into(),
            xem,
            tl,
            xuat,
            may,
            tac_vu_tx,
            tac_vu_rx,
            args,
            bat_dau: Instant::now(),
            da_do: false,
            da_chup: Arc::new(Mutex::new(None)),
            chup_da_xin: false,
        }
    }

    pub fn tb(&mut self, s: impl Into<String>) {
        self.thong_bao = Some((s.into(), Instant::now()));
    }

    pub fn gui(&mut self, r: std::io::Result<()>, ok: &str) {
        match r {
            Ok(()) => self.tb(ok),
            Err(e) => self.tb(format!("Không làm được: {e}")),
        }
        self.lam_moi.store(true, Ordering::Relaxed);
    }

    fn nhan_du_lieu(&mut self, ctx: &egui::Context) {
        while let Ok(s) = self.rx.try_recv() {
            self.snap = s;
        }
        while let Ok(s) = self.tac_vu_rx.try_recv() {
            self.tb(s);
        }
        while let Ok((p, anh)) = self.bia_rx.try_recv() {
            let b = anh.map_or(Bia::Hong, |a| Bia::Co(ctx.load_texture(p.to_string_lossy(), a, Default::default())));
            self.bia.insert(p, b);
        }
        // việc đã thôi chạy thì không còn "Đang dừng…"
        let dang: HashSet<&str> = self.snap.viec.iter().filter(|v| v.trang_thai == "chay").map(|v| v.id.as_str()).collect();
        self.dang_dung.retain(|id| dang.contains(id.as_str()));
        if !self.snap.dv.may.is_empty() {
            self.may_dang_mo = self.snap.dv.may.clone();
        }
    }

    /// Ảnh bìa: trả texture nếu đã có, ngược lại xếp hàng giải mã (1 lần).
    pub fn lay_bia(&mut self, p: &PathBuf) -> Option<egui::TextureId> {
        match self.bia.get(p) {
            Some(Bia::Co(t)) => Some(t.id()),
            Some(_) => None,
            None => {
                self.bia.insert(p.clone(), Bia::Dang);
                let _ = self.bia_tx.send(p.clone());
                None
            }
        }
    }

    fn do_dac(&mut self) {
        if !self.args.do_dac || self.da_do || !self.snap.co_du_lieu {
            return;
        }
        self.da_do = true;
        let ms = self.bat_dau.elapsed().as_millis();
        let rss = std::process::Command::new("ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output().ok().and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u64>().ok()).unwrap_or(0);
        eprintln!("DO mo_app_den_khi_co_so_lieu_ms={ms} quet_bo_ms={} so_bo={} ram_mb={}", self.snap.ms_quet_bo, self.snap.bo.len(), rss / 1024);
    }

    fn chup_man_hinh(&mut self, ctx: &egui::Context) {
        let Some(file) = self.args.chup.clone() else { return };
        if !self.chup_da_xin && self.snap.co_du_lieu && self.bat_dau.elapsed().as_secs_f32() > self.args.sau {
            self.chup_da_xin = true;
            let hop = Arc::clone(&self.da_chup);
            ctx.request_screenshot(move |a: Arc<egui::ColorImage>| {
                let loi = image::save_buffer(&file, a.as_raw(), a.width() as u32, a.height() as u32, image::ColorType::Rgba8).err().map(|e| e.to_string());
                if let Ok(mut g) = hop.lock() {
                    *g = Some(loi.unwrap_or_default());
                }
            });
        }
        if self.da_chup.lock().is_ok_and(|g| g.is_some()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.nhan_du_lieu(&ctx);
        egui::Panel::bottom("may").show_separator_line(false).frame(egui::Frame::new().fill(giao_dien::MAT_1).inner_margin(egui::Margin::symmetric(0, 6))).show(ui, |ui| self.thanh_may(ui));
        match &self.man {
            Man::Thu => self.man_thu_vien(ui),
            Man::Bo(p) => {
                let p = p.clone();
                self.man_bo(ui, &p);
            }
            Man::Chuong(..) => self.man_chuong(ui),
            Man::Timeline(_) => self.man_timeline(ui),
            Man::Xuat(_) => self.man_xuat(ui),
            Man::May => self.man_may(ui),
        }
        self.hop_thoai(&ctx);
        self.hien_thong_bao(&ctx);
        self.do_dac();
        self.chup_man_hinh(&ctx);
        // đồng hồ "3′ trước" + nhấp nháy: vẽ lại 2 lần/giây là đủ
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

impl App {
    fn hien_thong_bao(&mut self, ctx: &egui::Context) {
        if self.thong_bao.as_ref().is_some_and(|(_, t)| t.elapsed() > Duration::from_secs(5)) {
            self.thong_bao = None;
        }
        let Some((s, _)) = &self.thong_bao else { return };
        egui::Area::new(egui::Id::unique("tb")).anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -48.0]).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(s);
            });
        });
    }

    fn chu_dich_vu(&self) -> (String, egui::Color32) {
        use giao_dien::{CHU_MO, DO, XANH};
        let dv = &self.snap.dv;
        if !self.snap.co_du_lieu {
            return ("Đang đọc dữ liệu…".to_owned(), CHU_MO);
        }
        if dv.gio == 0.0 {
            return (format!("Dịch vụ nền chưa chạy ({} chưa có trang-thai.json)", self.duong.goc_dv.display()), DO);
        }
        if !dv.song {
            return (format!("Dịch vụ nền ĐÃ TẮT — trạng thái cuối {}", giao_dien::truoc(dv.gio)), DO);
        }
        let h = |k: &str| dv.hang.get(k).copied().unwrap_or(0);
        let ap = if dv.ap_luc == "thuong" || dv.ap_luc.is_empty() { String::new() } else { format!(" · máy {}", dv.ap_luc) };
        let tk = if dv.agy_tai_khoan.is_empty() { "?" } else { &dv.agy_tai_khoan };
        (format!("{} · dịch vụ chạy · {} chạy · {} chờ · {} chờ quota · {} lỗi · agy {tk}{ap}", self.may_dang_mo, h("chay"), h("cho"), h("cat_quota"), h("loi")), XANH)
    }

    /// Thanh dưới: máy này + dịch vụ nền + máy khác — số đo từ file trạng thái, không đoán.
    fn thanh_may(&mut self, ui: &mut egui::Ui) {
        use giao_dien::{CHU_MO, CHU_PHU, VANG, XANH};
        let (chu, mau) = self.chu_dich_vu();
        let may_khac: Vec<(String, bool, String)> = self.snap.dv.may_khac.iter().map(|m| (m.ten.clone(), m.truc_tuyen, m.mat_tu.map_or_else(String::new, du_lieu::gio_phut))).collect();
        let (quet_ms, quet_truoc) = (self.snap.ms_quet_bo, self.snap.gio_quet_bo);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.colored_label(mau, "●");
            ui.label(egui::RichText::new(chu).size(12.0).color(CHU_PHU));
            if ui.link("Máy & hàng việc").clicked() {
                self.mo_may();
            }
            for (ten, on, tu) in may_khac {
                ui.separator();
                ui.colored_label(if on { XANH } else { VANG }, "●");
                let t = if on { format!("{ten} trực tuyến") } else { format!("{ten} mất kết nối từ {tu}") };
                ui.label(egui::RichText::new(t).size(12.0).color(if on { CHU_PHU } else { VANG }));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(format!("quét bộ {quet_ms} ms · {}", giao_dien::truoc(quet_truoc))).size(11.0).color(CHU_MO));
            });
        });
    }
}

fn doc_args() -> Args {
    let mut a = Args { sau: 2.5, ..Args::default() };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        match k.as_str() {
            "--goc" => a.goc = it.next().map(PathBuf::from),
            "--dv" => a.dv = it.next().map(PathBuf::from),
            "--man" => a.man = it.next(),
            "--chup" => a.chup = it.next().map(PathBuf::from),
            "--sau" => a.sau = it.next().and_then(|s| s.parse().ok()).unwrap_or(2.5),
            "--buoc" => a.chon_buoc = it.next().and_then(|s| s.parse::<usize>().ok()).map(|n| n.saturating_sub(1)),
            "--do" => a.do_dac = true,
            _ => {}
        }
    }
    a
}

fn main() -> eframe::Result {
    let args = doc_args();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("Recap Studio").with_inner_size([1360.0, 860.0]).with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native("Recap Studio", opts, Box::new(move |cc| Ok(Box::new(App::moi(cc, args)))))
}
