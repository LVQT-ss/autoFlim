//! `recap-studio xuat`: xuất 1 video của bộ (Phần N / FULL) — dựng+xuất TỪNG CHƯƠNG (tiến trình con `dung-chuong`, RAM chỉ ~1 chương),
//! rồi nối `ffmpeg -c copy` (+ mở đầu mã lại cho khớp thông số). Tự kiểm như `kiem_video` của lam-bo.py, ghi sổ xuất + mốc chương.
//! Ghi ra `<ra-dir>/<bộ>/` (không đụng thư mục bộ). Dừng giữa chừng: mọi file đang ghi mang đuôi `.dang*.mp4`, chỉ đổi tên khi xong.
use crate::du_lieu::{Kq, doc_json, tim_moc};
use crate::{bo, phan};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub struct Video {
    pub nhan: String,
    pub tu: u32,
    pub den: u32,
    /// ghép `<bộ> - mo-dau*.mp4` lên đầu
    pub mo_dau: bool,
}

pub struct Tuy {
    pub bitrate_kbps: u32,
    pub song_song: usize,
    pub ra_dir: PathBuf,
    pub mo_dau: Option<PathBuf>,
    pub gpu: Option<String>,
    pub ma_hoa: Option<String>,
    /// nhóm trong dự án bộ: viet | en
    pub nhom: String,
}

pub fn ten_bo(goc: &Path) -> String {
    doc_json(&goc.join("_du-an.json"))
        .ok()
        .and_then(|d| d["ten_hien_thi"].as_str().map(String::from))
        .unwrap_or_else(|| goc.file_name().map_or_else(|| "bo".into(), |n| n.to_string_lossy().into_owned()))
}

/// `Script/arcNN-chA-B` → [(NN, A, B)] theo A (RE_ARC của lam-bo.py).
pub fn arcs(goc: &Path) -> Vec<(u32, u32, u32)> {
    let mut v: Vec<(u32, u32, u32)> = std::fs::read_dir(goc.join("Script"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            let p: Vec<&str> = n.strip_prefix("arc")?.split('-').collect();
            let [nn, a, b] = p[..] else { return None };
            Some((nn.parse().ok()?, a.strip_prefix("ch")?.parse().ok()?, b.parse().ok()?))
        })
        .collect();
    v.sort_by_key(|x| x.1);
    v
}

pub fn ma(n: u32) -> String {
    format!("ch{n:03}")
}

/// độ dài chương (giây) theo mốc câu cuối; None = chương chưa có mốc.
pub fn thoi_luong(goc: &Path, n: u32) -> Option<f64> {
    let f = tim_moc(&goc.join("Video").join(ma(n)).join("_technical"))?;
    let m = doc_json(&f).ok()?;
    let l = m["moc"].as_array()?.last()?.clone();
    Some(l["bat_dau"].as_f64()? + l["dai"].as_f64()?)
}

/// chia theo giờ: cắt ở ranh giới chương gần mốc `phut` nhất; phần cuối < nửa mốc dồn vào phần trước (chia_phan của lam-bo.py).
pub fn chia_gio(chs: &[(u32, f64)], phut: u32) -> Vec<(u32, u32)> {
    let muc = f64::from(phut) * 60.0;
    let mut cum = vec![0.0];
    for c in chs {
        cum.push(cum[cum.len() - 1] + c.1);
    }
    let (mut ra, mut i) = (vec![], 0usize);
    while i < chs.len() {
        let t0 = cum[i];
        let mut j = i;
        while j + 1 < chs.len() && cum[j + 1] - t0 < muc {
            j += 1;
        }
        if j > i && ((cum[j] - t0) - muc).abs() < ((cum[j + 1] - t0) - muc).abs() {
            j -= 1;
        }
        ra.push((chs[i].0, chs[j].0, t0, cum[j + 1]));
        i = j + 1;
    }
    if ra.len() > 1 && ra[ra.len() - 1].3 - ra[ra.len() - 1].2 < muc / 2.0 {
        let l = ra.pop().map(|x| (x.1, x.3));
        if let (Some(l), Some(p)) = (l, ra.last_mut()) {
            p.1 = l.0;
            p.3 = l.1;
        }
    }
    ra.into_iter().map(|x| (x.0, x.1)).collect()
}

/// Mọi video của bộ theo `xuat.chia` của `_du-an.json` (gio | arc | tay | full), `chia`/`phut` ép từ dòng lệnh nếu có.
pub fn danh_sach(goc: &Path, chia: Option<&str>, phut: Option<u32>) -> Kq<Vec<Video>> {
    let du_an = doc_json(&goc.join("_du-an.json")).unwrap_or(Value::Null);
    let x = &du_an["xuat"];
    let chia = chia.map(String::from).or_else(|| x["chia"].as_str().map(String::from)).unwrap_or_else(|| "full".into());
    let phut = phut.or_else(|| x["phut"].as_u64().map(|p| p.max(10) as u32)).unwrap_or(60);
    let ar = arcs(goc);
    let (dau, cuoi) = (ar.first().map(|a| a.1).ok_or("bộ chưa có Script/arcNN-chA-B")?, ar.iter().map(|a| a.2).max().unwrap_or(0));
    let co: Vec<u32> = (dau..=cuoi).filter(|n| ar.iter().any(|a| (a.1..=a.2).contains(n))).collect();
    let mut ra = vec![];
    match chia.as_str() {
        "arc" => ra.extend(ar.iter().map(|a| Video { nhan: format!("phan{:02} ch{:03}-{:03}", a.0, a.1, a.2), tu: a.1, den: a.2, mo_dau: a.1 == dau })),
        "gio" => {
            let mut chs = vec![];
            for n in &co {
                match thoi_luong(goc, *n) {
                    Some(d) => chs.push((*n, d)),
                    None => break, // chương thiếu: dừng, không chia "nhảy qua"
                }
            }
            for (k, (a, b)) in chia_gio(&chs, phut).into_iter().enumerate() {
                ra.push(Video { nhan: format!("phan{:02} ch{a:03}-{b:03}", k + 1), tu: a, den: b, mo_dau: k == 0 });
            }
        }
        "tay" => {
            for (k, c) in x["phan"].as_array().into_iter().flatten().enumerate() {
                if let (Some(a), Some(b)) = (c[0].as_u64(), c[1].as_u64()) {
                    ra.push(Video { nhan: format!("phan{:02} ch{a:03}-{b:03}", k + 1), tu: a as u32, den: b as u32, mo_dau: true });
                }
            }
        }
        "full" => {}
        k => return Err(format!("xuat.chia '{k}' lạ (gio|arc|tay|full)")),
    }
    ra.push(Video { nhan: "FULL".into(), tu: dau, den: cuoi, mo_dau: true });
    Ok(ra)
}

fn chay(cmd: &str, args: &[&str]) -> Kq<std::process::Output> {
    Command::new(cmd).args(args).output().map_err(|e| format!("{cmd}: {e}"))
}

/// ffprobe: (độ dài format, độ dài hình, độ dài tiếng) hoặc lỗi
pub fn do_dai(f: &Path) -> Kq<(f64, Option<f64>, Option<f64>)> {
    let o = chay("ffprobe", &["-v", "error", "-show_entries", "stream=codec_type,duration:format=duration", "-of", "json", &f.to_string_lossy()])?;
    let j: Value = serde_json::from_slice(&o.stdout).map_err(|e| format!("ffprobe {}: {e}", f.display()))?;
    let d = |t: &str| j["streams"].as_array().and_then(|s| s.iter().find(|x| x["codec_type"] == t)).map(|x| x["duration"].as_str().and_then(|v| v.parse().ok()).unwrap_or(0.0));
    Ok((j["format"]["duration"].as_str().and_then(|v| v.parse().ok()).unwrap_or(0.0), d("video"), d("audio")))
}

/// Tự kiểm như `kiem_video`: hình+tiếng, độ dài ±`dung_sai` s, tiếng không hụt >2 s, 5 khung rải đều không đen. [] = đạt.
pub fn kiem(f: &Path, giay: f64, dung_sai: f64) -> Vec<String> {
    let (tong, v, a) = match do_dai(f) {
        Ok(x) => x,
        Err(e) => return vec![format!("không kiểm được: {e}")],
    };
    let mut loi = vec![];
    if v.is_none() {
        loi.push("không có hình".into());
    }
    if a.is_none() {
        loi.push("không có tiếng".into());
    }
    if (tong - giay).abs() > dung_sai {
        loi.push(format!("dài {tong:.1}s, mong {giay:.1}s"));
    }
    if let (Some(v), Some(a)) = (v, a)
        && a < v - 2.0
    {
        loi.push(format!("tiếng hụt {:.0}s", v - a));
    }
    let den = (1..=5)
        .filter(|i| {
            let ss = format!("{:.2}", tong * f64::from(*i) / 6.0);
            chay("ffmpeg", &["-v", "error", "-ss", &ss, "-i", &f.to_string_lossy(), "-frames:v", "1", "-vf", "scale=64:36,format=gray", "-f", "rawvideo", "-"])
                .is_ok_and(|o| !o.stdout.is_empty() && o.stdout.iter().map(|b| u64::from(*b)).sum::<u64>() / (o.stdout.len() as u64) < 6)
        })
        .count();
    if den >= 3 {
        loi.push(format!("{den}/5 khung đen"));
    }
    loi
}

fn doc_so(p: &Path) -> Vec<Value> {
    std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn ghi_so(p: &Path, so: &[Value]) -> Kq<()> {
    let t = p.with_extension("json.dang");
    std::fs::write(&t, serde_json::to_string_pretty(so).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&t, p).map_err(|e| e.to_string())
}

fn don_dang(dir: &Path) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if n.contains(".dang") {
            let _ = std::fs::remove_file(e.path()); // chỉ file `.dang*` do chính lệnh này tạo
        }
    }
}

/// Xuất 1 chương bằng tiến trình con `xuat-doan`: mở DỰ ÁN BỘ (kể cả sửa tay) và xuất đoạn giữa marker chương này và chương kế —
/// KHÔNG dựng lại từ dữ liệu (T621; trước đó xuất bỏ qua sửa tay). Trả (độ dài đoạn, ms xuất).
fn xuat_chuong(du: &phan::DuAn, du_an: &Path, n: u32, tam: &Path, t: &Tuy, dung_sai: f64) -> Kq<(f64, f64)> {
    let m = ma(n);
    let (seq, a, b) = phan::doan(du, &t.nhom, &m)?;
    let mong = b - a;
    let dich = tam.join(format!("{m}.mp4"));
    // dùng lại đoạn đã xuất nếu dấu nội dung chương không đổi và còn đạt
    let dau = phan::dau(du, &format!("{}/{m}", t.nhom)).unwrap_or_default();
    let f_dau = tam.join(format!("{m}.dau"));
    if dich.is_file() && std::fs::read_to_string(&f_dau).ok().as_deref() == Some(dau.as_str()) && kiem(&dich, mong, dung_sai).is_empty() {
        return Ok((mong, 0.0));
    }
    let dang = tam.join(format!("{m}.{}.dang.mp4", std::process::id()));
    let t0 = std::time::Instant::now();
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut c = Command::new(exe)
        .args(["xuat-doan", &du_an.to_string_lossy(), "--seq", &seq.to_string(), "--tu", &a.to_string(), "--den", &b.to_string(), "--ra", &dang.to_string_lossy()])
        .args(["--bitrate-kbps", &t.bitrate_kbps.to_string()])
        .args(t.gpu.iter().flat_map(|g| ["--gpu", g.as_str()]))
        .args(t.ma_hoa.iter().flat_map(|m| ["--ma-hoa", m.as_str()]))
        .env("RECAP_STUDIO_CON", "1")
        .env("RECAP_H264_PRESET", std::env::var("RECAP_H264_PRESET").unwrap_or_else(|_| "speed".into()))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    // xuất treo thật: quá 4× độ dài (ít nhất 10 phút) thì dừng + báo lỗi, giữ file dở để xem
    let han = std::time::Duration::from_secs_f64((mong * 4.0).max(600.0));
    let kq = loop {
        if let Some(s) = c.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        if t0.elapsed() > han {
            let _ = c.kill();
            return Err(format!("{m}: xuất quá {:.0}s — dừng (file dở: {})", han.as_secs_f64(), dang.display()));
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    };
    if !kq.success() {
        let mut e = String::new();
        if let Some(mut s) = c.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut s, &mut e);
        }
        let _ = std::fs::remove_file(&dang);
        return Err(format!("{m}: xuat-doan lỗi: {}", e.trim()));
    }
    let loi = kiem(&dang, mong, dung_sai);
    if !loi.is_empty() {
        return Err(format!("{m}: đoạn xuất hỏng ({}) — file dở {}", loi.join("; "), dang.display()));
    }
    std::fs::rename(&dang, &dich).map_err(|e| e.to_string())?;
    std::fs::write(&f_dau, dau).map_err(|e| e.to_string())?;
    Ok((mong, t0.elapsed().as_secs_f64() * 1000.0))
}

fn gio(s: f64) -> String {
    let s = s.round() as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{:02}:{:02}", s / 60, s % 60) }
}

/// mở đầu mã lại ĐÚNG thông số của đoạn chương (cỡ, fps, pix_fmt, timescale, âm thanh) để nối `-c copy` không lệch.
fn chuan_mo_dau(md: &Path, mau: &Path, ra: &Path) -> Kq<()> {
    let o = chay("ffprobe", &["-v", "error", "-show_entries", "stream=codec_type,width,height,r_frame_rate,pix_fmt,sample_rate,channels,time_base", "-of", "json", &mau.to_string_lossy()])?;
    let j: Value = serde_json::from_slice(&o.stdout).map_err(|e| e.to_string())?;
    let st = |t: &str| j["streams"].as_array().and_then(|s| s.iter().find(|x| x["codec_type"] == t)).cloned().unwrap_or(Value::Null);
    let (v, a) = (st("video"), st("audio"));
    let g = |x: &Value, k: &str| x[k].as_str().map(String::from).or_else(|| x[k].as_u64().map(|n| n.to_string())).unwrap_or_default();
    let vf = format!("scale={}:{}:flags=lanczos,fps={},format={}", g(&v, "width"), g(&v, "height"), g(&v, "r_frame_rate"), g(&v, "pix_fmt"));
    let ts = g(&v, "time_base").split('/').nth(1).unwrap_or("15360").to_string();
    let o = Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-i"])
        .arg(md)
        .args(["-vf", &vf, "-c:v", "libx264", "-crf", "18", "-bf", "0", "-video_track_timescale", &ts, "-c:a", "aac", "-b:a", "192k", "-ar", &g(&a, "sample_rate"), "-ac", &g(&a, "channels")])
        .arg(ra)
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() { Ok(()) } else { Err(format!("mã lại mở đầu: {}", String::from_utf8_lossy(&o.stderr))) }
}

pub fn xuat(goc: &Path, v: &Video, t: &Tuy) -> Kq<Value> {
    let ten = ten_bo(goc);
    let dir = t.ra_dir.join(&ten);
    let tam = dir.join(".tam");
    std::fs::create_dir_all(tam.join("cache")).map_err(|e| e.to_string())?;
    don_dang(&dir);
    don_dang(&tam);
    let chs: Vec<u32> = (v.tu..=v.den).filter(|n| thoi_luong(goc, *n).is_some()).collect();
    let du_an = bo::thu_muc(goc).join(format!("{}.fcproj", bo::ten_du_an(goc)));
    let du = phan::doc(&bo::thu_muc(goc), &bo::ten_du_an(goc))?.ok_or_else(|| format!("chưa có {} — chạy `recap-studio dung-bo` (hoặc `gop-cu`) trước", du_an.display()))?;
    if chs.is_empty() {
        return Err(format!("{}: không chương nào có mốc câu trong ch{:03}-{:03}", v.nhan, v.tu, v.den));
    }
    let t0 = std::time::Instant::now();
    log(&format!("{ten} - {}: {} chương, song song {}", v.nhan, chs.len(), t.song_song));

    // 1) từng chương, t.song_song tiến trình con cùng lúc; lỗi đầu tiên dừng phần còn lại (đoạn đã xong giữ để chạy lại)
    let (kq, loi, dung_lai) = (Mutex::new(vec![(0.0_f64, 0.0_f64); chs.len()]), Mutex::new(None::<String>), AtomicBool::new(false));
    let tiep = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..t.song_song.max(1) {
            s.spawn(|| {
                while !dung_lai.load(Ordering::SeqCst) {
                    let i = tiep.fetch_add(1, Ordering::SeqCst);
                    let Some(n) = chs.get(i) else { break };
                    match xuat_chuong(&du, &du_an, *n, &tam, t, 1.0) {
                        Ok(r) => {
                            log(&format!("  {} xong ({:.0}s chương, xuất {:.0}s)", ma(*n), r.0, r.1 / 1000.0));
                            if let Ok(mut k) = kq.lock() {
                                k[i] = r;
                            }
                        }
                        Err(e) => {
                            dung_lai.store(true, Ordering::SeqCst);
                            if let Ok(mut l) = loi.lock() {
                                l.get_or_insert(e);
                            }
                        }
                    }
                }
            });
        }
    });
    if let Some(e) = loi.lock().map_err(|e| e.to_string())?.take() {
        return Err(e);
    }
    let kq = kq.into_inner().map_err(|e| e.to_string())?;

    // 2) nối (+ mở đầu)
    let ten_file = format!("{ten} - {}.mp4", v.nhan);
    let dich = dir.join(&ten_file);
    let dang = dir.join(format!("{ten_file}.dang-xuat.mp4"));
    let mut ds = String::new();
    let md = if v.mo_dau { t.mo_dau.clone().or_else(|| Some(goc.join("Xuat").join(format!("{ten} - mo-dau.mp4"))).filter(|p| p.is_file())) } else { None };
    let mut md_giay = 0.0;
    let q = |p: &Path| format!("file '{}'\n", p.to_string_lossy().replace('\'', "'\\''"));
    if let Some(m) = &md {
        let chuan = tam.join("mo-dau.dang.mp4");
        chuan_mo_dau(m, &tam.join(format!("{}.mp4", ma(chs[0]))), &chuan)?;
        md_giay = do_dai(&chuan)?.0;
        ds.push_str(&q(&chuan));
    }
    let mut moc = vec![];
    let mut cur = md_giay;
    for (i, n) in chs.iter().enumerate() {
        let f = tam.join(format!("{}.mp4", ma(*n)));
        moc.push(json!({"chuong": ma(*n), "giay": (cur * 100.0).round() / 100.0, "gio": gio(cur)}));
        cur += do_dai(&f)?.0;
        ds.push_str(&q(&f));
        let _ = i;
    }
    let (list, mong) = (tam.join("ds.dang.txt"), md_giay + kq.iter().map(|x| x.0).sum::<f64>());
    std::fs::write(&list, ds).map_err(|e| e.to_string())?;
    let o = Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list)
        .args(["-c", "copy", "-movflags", "+faststart", "-f", "mp4"])
        .arg(&dang)
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        let _ = std::fs::remove_file(&dang);
        return Err(format!("nối đoạn: {}", String::from_utf8_lossy(&o.stderr)));
    }
    let _ = std::fs::remove_file(&list);
    let _ = std::fs::remove_file(tam.join("mo-dau.dang.mp4"));

    // 3) tự kiểm + sổ; lỗi thì không đè bản cũ đạt
    let van_de = kiem(&dang, mong, 1.0);
    let mb = dang.metadata().map_or(0.0, |m| m.len() as f64 / 1e6);
    let thung = dir.join(".thung-rac");
    let dat = van_de.is_empty();
    let cuoi = if dat { dich.clone() } else { dir.join(format!("{ten} - {}.loi.mp4", v.nhan)) };
    if dat && dich.is_file() {
        std::fs::create_dir_all(&thung).map_err(|e| e.to_string())?;
        let luc = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        std::fs::rename(&dich, thung.join(format!("{luc}-{ten_file}"))).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&dang, &cuoi).map_err(|e| e.to_string())?;
    let ghi = json!({
        "file": cuoi.file_name().map(|n| n.to_string_lossy()), "nhan": v.nhan, "tu": ma(v.tu), "den": ma(v.den), "giay": (mong * 100.0).round() / 100.0,
        "mb": (mb * 10.0).round() / 10.0, "mb_moi_phut": (mb / (mong / 60.0) * 10.0).round() / 10.0, "luc": luc_iso(),
        "kiem": if dat { json!("đạt") } else { json!(van_de) }, "mo_dau": md_giay, "bitrate_kbps": t.bitrate_kbps,
        "xuat_giay": t0.elapsed().as_secs_f64().round(), "moc_chuong": moc,
        "xuat_chuong_giay": chs.iter().zip(&kq).map(|(n, k)| json!({ma(*n): (k.1 / 1000.0).round()})).collect::<Vec<_>>(),
    });
    let sf = dir.join("so-xuat.json");
    let mut so = doc_so(&sf);
    so.retain(|x| x["file"] != ghi["file"]);
    so.push(ghi.clone());
    ghi_so(&sf, &so)?;
    if dat { Ok(ghi) } else { Err(format!("{}: tự kiểm LỖI: {} (file {})", v.nhan, van_de.join("; "), cuoi.display())) }
}

fn luc_iso() -> String {
    let o = Command::new("date").arg("+%Y-%m-%dT%H:%M:%S").output().ok();
    o.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

fn log(s: &str) {
    eprintln!("[{}] {s}", luc_iso());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chia_gio_cat_o_ranh_gioi_chuong() {
        // 10 chương × 10 phút, mốc 30 phút → 3 phần (3+3+4: phần cuối 10 < 15 phút dồn nếu lẻ)
        let chs: Vec<(u32, f64)> = (1..=10).map(|n| (n, 600.0)).collect();
        assert_eq!(chia_gio(&chs, 30), vec![(1, 3), (4, 6), (7, 10)]);
        // phần cuối nhỏ hơn nửa mốc dồn vào phần trước
        let chs: Vec<(u32, f64)> = (1..=7).map(|n| (n, 600.0)).collect();
        assert_eq!(chia_gio(&chs, 30), vec![(1, 3), (4, 7)]);
    }

    #[test]
    fn gio_dinh_dang() {
        assert_eq!(gio(65.0), "01:05");
        assert_eq!(gio(3725.0), "1:02:05");
    }
}
