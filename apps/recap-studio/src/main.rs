//! Recap Studio — lõi dựng chương: `recap-studio dung-chuong <thư mục bộ> chNNN [tuỳ chọn]`.
//! Đọc dữ liệu bộ ĐỊNH DẠNG CŨ (chỉ đọc), dựng sequence bằng lõi FilmCraft (MIT/Apache, xem ATTRIBUTION.md).
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod bo;
mod du_lieu;
mod dung;
mod ke_hoach;
mod phan;
mod xuat;

use std::path::PathBuf;

const HELP: &str = "recap-studio dung-chuong <thư mục bộ> <chNNN> [--ra video.mp4] [--du-an chuong.fcproj] [--bitrate-kbps N=3000] [--thu N] [--kieu K] [--doan từ,đến] [--cache dir] [--gpu off|auto] [--ma-hoa off|auto]
  Không có --ra = chỉ dựng + lưu dự án (không xuất). Mặc định --du-an = <thư mục tạm>/recap-studio/<chNNN>.fcproj.
  CHỈ ĐỌC thư mục bộ; mọi thứ ghi nằm ở --du-an/--ra/--cache.
recap-studio dung-bo <thư mục bộ> [--chuong A-B|chNNN] [--nhom viet|en] [--ep] [--thay-sua-tay]
  Dựng/cập nhật chương vào dự án bộ <bộ>/timeline/<bộ>.fcproj (dải phần phẳng, marker chNNN). Chương đã sửa tay GIỮ nguyên (trừ --thay-sua-tay).
recap-studio trang-thai <thư mục bộ>   JSON phần/chương của dự án bộ (Màn 5) · recap-studio mo-phan <bộ> --phan N   đặt phần mở sẵn
  (biến RECAP_TIMELINE_DIR = nơi đặt dự án nếu không phải <bộ>/timeline)
recap-studio gop-cu <thư mục bộ> [--nhom viet]   gộp timeline/chNNN.fcproj cũ vào dự án bộ (sao lưu timeline.truoc-t621)
recap-studio xuat-doan <du-an.fcproj> --seq ID --tu giây --den giây --ra out.mp4   (tiến trình con của xuat)
recap-studio xuat <thư mục bộ> (--phan N | --full | --chuong A-B [--nhan tên]) [--ra-dir D=~/recap-thu-moc2] [--bitrate-kbps N=3000]
                  [--song-song N=1] [--chia gio|arc|tay|full] [--phut N] [--mo-dau file.mp4]
  Chia theo `xuat.chia` của _du-an.json (ép bằng --chia). Mỗi chương dựng+xuất riêng rồi nối; ghi D/<bộ>/<bộ> - <nhãn>.mp4 + so-xuat.json.
  Mở đầu: ghép <bộ>/Xuat/<tên> - mo-dau.mp4 (hoặc --mo-dau) vào FULL và phần đầu; --chuong chỉ ghép khi có --mo-dau.";

fn xuat_lenh(a: &[String]) -> Result<(), String> {
    let goc = PathBuf::from(shellexpand(a.get(1).ok_or(HELP)?));
    let ra_dir = PathBuf::from(shellexpand(&opt(a, "--ra-dir").unwrap_or_else(|| "~/recap-thu-moc2".into())));
    let tuy = xuat::Tuy {
        bitrate_kbps: opt(a, "--bitrate-kbps").and_then(|v| v.parse().ok()).unwrap_or(3000),
        song_song: opt(a, "--song-song").and_then(|v| v.parse().ok()).unwrap_or(1),
        ra_dir,
        gpu: opt(a, "--gpu"),
        ma_hoa: opt(a, "--ma-hoa"),
        nhom: opt(a, "--nhom").unwrap_or_else(|| "viet".into()),
        mo_dau: opt(a, "--mo-dau").map(|p| PathBuf::from(shellexpand(&p))),
    };
    let v = if let Some((x, y)) = opt(a, "--chuong").and_then(|c| c.split_once('-').map(|(x, y)| (x.to_string(), y.to_string()))) {
        let (tu, den): (u32, u32) = (x.parse().map_err(|_| "--chuong A-B")?, y.parse().map_err(|_| "--chuong A-B")?);
        xuat::Video { nhan: opt(a, "--nhan").unwrap_or_else(|| format!("ch{tu:03}-{den:03}")), tu, den, mo_dau: tuy.mo_dau.is_some() }
    } else {
        let ds = xuat::danh_sach(&goc, opt(a, "--chia").as_deref(), opt(a, "--phut").and_then(|v| v.parse().ok()))?;
        let can = if a.iter().any(|x| x == "--full") { "FULL".to_string() } else { format!("phan{:02}", opt(a, "--phan").and_then(|v| v.parse::<u32>().ok()).ok_or("cần --phan N, --full hoặc --chuong A-B")?) };
        ds.into_iter().find(|v| v.nhan.starts_with(&can)).ok_or_else(|| format!("bộ không có video '{can}' (chia hiện tại không tạo ra nó — thử --chia gio|arc)"))?
    };
    let kq = xuat::xuat(&goc, &v, &tuy)?;
    println!("{kq}");
    Ok(())
}

fn dung_bo_lenh(a: &[String]) -> Result<(), String> {
    let goc = PathBuf::from(shellexpand(a.get(1).ok_or(HELP)?));
    // --chuong A-B | chNNN-chMMM | chNNN (1 chương — dịch vụ nền truyền mã chương)
    let so = |s: &str| s.trim_start_matches("ch").parse::<u32>();
    let c = opt(a, "--chuong").map(|c| if c.contains('-') { c } else { format!("{c}-{c}") });
    let (tu, den) = match c.as_deref().and_then(|c| c.split_once('-').map(|(x, y)| (so(x), so(y)))) {
        Some((Ok(x), Ok(y))) => (x, y),
        Some(_) => return Err("--chuong A-B".into()),
        None => {
            let ar = xuat::arcs(&goc);
            (ar.first().map(|x| x.1).ok_or("bộ chưa có Script/arcNN-chA-B")?, ar.iter().map(|x| x.2).max().unwrap_or(0))
        }
    };
    let tuy = bo::Tuy {
        nhom: opt(a, "--nhom").unwrap_or_else(|| "viet".into()),
        ep: a.iter().any(|x| x == "--ep"),
        thay_sua_tay: a.iter().any(|x| x == "--thay-sua-tay"),
        cache: opt(a, "--cache").map_or_else(|| std::env::temp_dir().join("recap-studio").join("cache"), PathBuf::from),
    };
    let chs: Vec<u32> = (tu..=den).collect();
    println!("{}", bo::dung_bo(&goc, &chs, &tuy)?);
    Ok(())
}

fn gop_cu_lenh(a: &[String]) -> Result<(), String> {
    let goc = PathBuf::from(shellexpand(a.get(1).ok_or(HELP)?));
    println!("{}", bo::gop_cu(&goc, &opt(a, "--nhom").unwrap_or_else(|| "viet".into()))?);
    Ok(())
}

/// Bộ dựng khung GPU (filmcraft-gpu) sau móc của filmcraft-export; chỉ dùng khi `--gpu auto`, chỗ nào GPU không dựng được thì CPU dựng.
struct GpuFrameRenderer(filmcraft_gpu::ExportRenderer);

impl filmcraft_export::FrameRenderer for GpuFrameRenderer {
    fn render(
        &mut self,
        project: &filmcraft_project::Project,
        seq: filmcraft_project::ItemId,
        t: filmcraft_time::Tick,
        opts: filmcraft_render::RenderOptions,
        sources: &dyn filmcraft_render::SourceProvider,
    ) -> Option<filmcraft_render::Image> {
        self.0.render(project, seq, t, opts, sources)
    }

    fn caps(&self) -> filmcraft_render::plan::DeviceCaps {
        self.0.caps()
    }
}

fn xuat_doan_lenh(a: &[String]) -> Result<(), String> {
    filmcraft_export::register_frame_renderer(|| {
        filmcraft_gpu::ExportRenderer::new().map(|r| Box::new(GpuFrameRenderer(r)) as Box<dyn filmcraft_export::FrameRenderer>)
    });
    if std::env::var_os("RECAP_STUDIO_CON").is_some() {
        std::thread::spawn(|| {
            let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
            std::process::exit(130);
        });
    }
    let (du_an, ra) = (PathBuf::from(shellexpand(a.get(1).ok_or(HELP)?)), PathBuf::from(opt(a, "--ra").ok_or("thiếu --ra")?));
    let num = |k: &str| opt(a, k).and_then(|v| v.parse::<f64>().ok()).ok_or(format!("thiếu {k}"));
    let kq = bo::xuat_doan(&bo::Doan {
        du_an: &du_an,
        seq: opt(a, "--seq").and_then(|v| v.parse().ok()).ok_or("thiếu --seq")?,
        tu: num("--tu")?,
        den: num("--den")?,
        ra: &ra,
        bitrate_kbps: opt(a, "--bitrate-kbps").and_then(|v| v.parse().ok()).unwrap_or(3000),
        gpu: opt(a, "--gpu").as_deref(),
        ma_hoa: opt(a, "--ma-hoa").as_deref(),
    })?;
    let g = filmcraft_export::gpu_render_stats();
    if g.frames + g.fallbacks > 0 {
        eprintln!("gpu khung {} lùi CPU {}", g.frames, g.fallbacks);
    }
    println!("{kq}");
    Ok(())
}

fn opt(a: &[String], k: &str) -> Option<String> {
    a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned()
}

fn chay() -> Result<(), String> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("xuat") => return xuat_lenh(&a),
        Some("dung-bo") => return dung_bo_lenh(&a),
        Some("gop-cu") => return gop_cu_lenh(&a),
        Some("trang-thai") => {
            println!("{}", bo::trang_thai(&PathBuf::from(shellexpand(a.get(1).ok_or(HELP)?)))?);
            return Ok(());
        }
        Some("mo-phan") => {
            let so = opt(&a, "--phan").and_then(|v| v.parse().ok()).ok_or("thiếu --phan N")?;
            println!("{}", bo::mo_phan(&PathBuf::from(shellexpand(a.get(1).ok_or(HELP)?)), &opt(&a, "--nhom").unwrap_or_else(|| "viet".into()), so)?);
            return Ok(());
        }
        Some("xuat-doan") => return xuat_doan_lenh(&a),
        _ => {}
    }
    if a.first().map(String::as_str) != Some("dung-chuong") || a.len() < 3 {
        return Err(HELP.into());
    }
    if std::env::var_os("RECAP_STUDIO_CON").is_some() {
        // tiến trình con của `xuat`: cha chết (Ctrl-C/kill) thì stdin đóng → dừng ngay, không để xuất mồ côi chạy tiếp
        std::thread::spawn(|| {
            let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
            std::process::exit(130);
        });
    }
    let goc = PathBuf::from(shellexpand(&a[1]));
    let ma = a[2].clone();
    let tmp = std::env::temp_dir().join("recap-studio");
    let tuy = dung::Tuy {
        cache: opt(&a, "--cache").map_or_else(|| tmp.join("cache"), PathBuf::from),
        thu: opt(&a, "--thu").and_then(|v| v.parse().ok()).unwrap_or(0),
        // 1080p manga: 3.000 kbps ≈ app cũ (~3,1 Mbps); mặc định 18 Mbps của FilmCraft = ~6× dung lượng (PSNR 40 dB so với 6 Mbps)
        bitrate_kbps: Some(opt(&a, "--bitrate-kbps").and_then(|v| v.parse().ok()).unwrap_or(3000)),
        gpu: opt(&a, "--gpu"),
        ma_hoa: opt(&a, "--ma-hoa"),
        doan: opt(&a, "--doan").and_then(|v| v.split_once(',').and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?)))),
    };
    let fcproj = opt(&a, "--du-an").map_or_else(|| tmp.join(format!("{ma}.fcproj")), PathBuf::from);
    let mp4 = opt(&a, "--ra").map(PathBuf::from);
    if let Some(d) = fcproj.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let mut ch = du_lieu::doc_chuong(&goc, &ma)?;
    if let Some(k) = opt(&a, "--kieu") {
        // ép 1 kiểu cho mọi trang (để thử/so từng kiểu): truot-song | camera-path | quet-doc | tinh | tinh-phong
        ch.trang.iter_mut().for_each(|t| t.kieu.clone_from(&k));
    }
    let kq = dung::Dung::new().dung_chuong(&ch, &tuy, &fcproj, mp4.as_deref())?;
    println!(
        "{}",
        serde_json::json!({"chuong": ma, "trang": kq.trang, "ten_rieng": kq.ten_rieng, "giay_video": kq.giay, "dung_ms": kq.lenh_ms, "xuat_ms": kq.xuat_ms, "du_an": fcproj, "ra": mp4})
    );
    Ok(())
}

fn shellexpand(p: &str) -> String {
    match (p.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(r), Ok(h)) => format!("{h}/{r}"),
        _ => p.into(),
    }
}

fn main() {
    if let Err(e) = chay() {
        eprintln!("recap-studio: {e}");
        std::process::exit(1);
    }
}
