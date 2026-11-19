//! Lệnh cấp bộ của kiểu B (T621): `dung-bo` (dựng/cập nhật chương vào dự án bộ), `gop-cu` (gộp chNNN.fcproj cũ), `xuat-doan`
//! (xuất 1 đoạn của dải phần — tiến trình con của `xuat`, KHÔNG dựng lại từ dữ liệu).
use crate::du_lieu::{Kq, doc_json, tim_moc};
use crate::phan::{self, DuAn};
use filmcraft_engine::Session;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// tên dự án = tên thư mục bộ (`<bộ>/timeline/<bộ>.fcproj`)
pub fn ten_du_an(goc: &Path) -> String {
    goc.file_name().map_or_else(|| "bo".into(), |n| n.to_string_lossy().into_owned())
}

/// `RECAP_TIMELINE_DIR` = nơi đặt dự án khi dữ liệu dựng nằm ở thư mục khác (bộ đã chuyển từ định dạng cũ — recap-gui đặt biến này)
pub fn thu_muc(goc: &Path) -> PathBuf {
    std::env::var_os("RECAP_TIMELINE_DIR").map_or_else(|| goc.join("timeline"), PathBuf::from)
}

fn tem_nguon(goc: &Path, ma: &str) -> String {
    tim_moc(&goc.join("Video").join(ma).join("_technical"))
        .and_then(|f| f.metadata().ok())
        .map_or_else(|| "?".into(), |m| format!("{}:{}", m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs()), m.len()))
}

pub struct Tuy {
    pub nhom: String,
    /// dựng lại cả chương chưa sửa tay dù dữ liệu không đổi
    pub ep: bool,
    /// ghi đè cả chương đã sửa tay (mặc định GIỮ)
    pub thay_sua_tay: bool,
    pub cache: PathBuf,
}

/// dựng từng chương bằng tiến trình con `dung-chuong` (RAM chỉ ~1 chương) rồi ghép; lưu sau MỖI chương.
pub fn dung_bo(goc: &Path, chs: &[u32], t: &Tuy) -> Kq<Value> {
    let tam = std::env::temp_dir().join(format!("recap-studio-bo-{}", std::process::id()));
    std::fs::create_dir_all(&tam).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let (ten, dir) = (ten_du_an(goc), thu_muc(goc));
    let mut du: Option<DuAn> = phan::doc(&dir, &ten)?;
    let (mut moi, mut thay, mut giu, mut khong_doi, mut thieu) = (vec![], vec![], vec![], vec![], vec![]);
    for n in chs {
        let ma = format!("ch{n:03}");
        let key = format!("{}/{ma}", t.nhom);
        if tim_moc(&goc.join("Video").join(&ma).join("_technical")).is_none() {
            thieu.push(ma);
            continue;
        }
        let nguon = tem_nguon(goc, &ma);
        let co = du.as_ref().is_some_and(|d| d.meta["chuong"].get(&key).is_some());
        if let (true, Some(d)) = (co, du.as_ref()) {
            if phan::da_sua_tay(d, &key) && !t.thay_sua_tay {
                giu.push(ma);
                continue;
            }
            if !t.ep && d.meta["chuong"][&key]["nguon"] == nguon.as_str() && !phan::da_sua_tay(d, &key) {
                khong_doi.push(ma);
                continue;
            }
        }
        let f = tam.join(format!("{ma}.fcproj"));
        let mut c = Command::new(&exe)
            .args(["dung-chuong", &goc.to_string_lossy(), &ma, "--du-an", &f.to_string_lossy(), "--cache", &t.cache.to_string_lossy()])
            .env("RECAP_STUDIO_CON", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut e = String::new();
        if let Some(mut s) = c.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut s, &mut e);
        }
        if !c.wait().map_err(|e| e.to_string())?.success() {
            return Err(format!("{ma}: dung-chuong lỗi: {}", e.trim()));
        }
        let ch = doc_json(&f)?;
        let _ = std::fs::remove_file(&f);
        let d = match du.take() {
            Some(d) => d,
            None => phan::moi(&ten, &ch),
        };
        let mut d = d;
        phan::ghep(&mut d, ch, &ma, &t.nhom, &nguon)?;
        phan::luu(&dir, &ten, &d)?;
        du = Some(d);
        if co { thay.push(ma) } else { moi.push(ma) }
    }
    let _ = std::fs::remove_dir_all(&tam);
    if let Some(mut d) = du {
        phan::lam_full(&mut d, &t.nhom)?;
        phan::luu(&dir, &ten, &d)?;
    }
    Ok(json!({"moi": moi, "thay": thay, "giu_sua_tay": giu, "khong_doi": khong_doi, "thieu_moc": thieu, "du_an": dir.join(format!("{ten}.fcproj"))}))
}

/// gộp `timeline/chNNN.fcproj` cũ thành dự án bộ. Sao lưu cả thư mục trước (`timeline.truoc-t621`), chỉ rồi mới gỡ file rời.
pub fn gop_cu(goc: &Path, nhom: &str) -> Kq<Value> {
    let dir = thu_muc(goc);
    let mut fs: Vec<(u32, PathBuf)> = std::fs::read_dir(&dir)
        .map_err(|e| format!("đọc {}: {e}", dir.display()))?
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            Some((n.strip_prefix("ch")?.strip_suffix(".fcproj")?.parse().ok()?, e.path()))
        })
        .collect();
    fs.sort();
    if fs.is_empty() {
        return Err(format!("{}: không có chNNN.fcproj để gộp", dir.display()));
    }
    let sao = goc.join("timeline.truoc-t621");
    if !sao.exists() {
        let o = Command::new("cp").arg("-R").arg(&dir).arg(&sao).output().map_err(|e| e.to_string())?;
        if !o.status.success() {
            return Err(format!("sao lưu timeline: {}", String::from_utf8_lossy(&o.stderr)));
        }
    }
    let ten = ten_du_an(goc);
    let mut du: Option<DuAn> = phan::doc(&dir, &ten)?;
    for (n, f) in &fs {
        let ch = doc_json(f)?;
        let ma = format!("ch{n:03}");
        let mut d = match du.take() {
            Some(d) => d,
            None => phan::moi(&ten, &ch),
        };
        phan::ghep(&mut d, ch, &ma, nhom, &tem_nguon(goc, &ma))?;
        du = Some(d);
    }
    let mut d = du.ok_or("không có chương nào")?;
    phan::lam_full(&mut d, nhom)?;
    phan::luu(&dir, &ten, &d)?;
    for (_, f) in &fs {
        let _ = std::fs::remove_file(f); // bản gốc đã nằm trong timeline.truoc-t621
    }
    Ok(json!({"gop": fs.len(), "du_an": dir.join(format!("{ten}.fcproj")), "sao_luu": sao}))
}

pub struct Doan<'a> {
    pub du_an: &'a Path,
    pub seq: i64,
    pub tu: f64,
    pub den: f64,
    pub ra: &'a Path,
    pub bitrate_kbps: u32,
    pub gpu: Option<&'a str>,
    pub ma_hoa: Option<&'a str>,
}

/// mở dự án bộ, mở sequence phần, xuất khoảng [tu, den) giây — đúng cái Thịnh thấy trên timeline (kể cả sửa tay)
pub fn xuat_doan(d: &Doan) -> Kq<Value> {
    let mut s = Session::default();
    let mut lenh = |id: &str, p: Value| s.execute(id, p).map_err(|e| format!("{id}: {e}"));
    lenh("file.open", json!({"path": d.du_an}))?;
    lenh("sequence.open", json!({"item": d.seq}))?;
    let mut e = json!({"path": d.ra, "format": "h264", "audio": true, "wait": true, "bitrateKbps": d.bitrate_kbps, "bitrateMode": "vbr1Pass",
        "range": "custom", "startSeconds": d.tu, "endSeconds": d.den});
    if let Some(o) = e.as_object_mut() {
        if let Some(g) = d.gpu {
            o.insert("gpuRendering".into(), json!(g));
        }
        if let Some(m) = d.ma_hoa {
            o.insert("hardwareEncoding".into(), json!(m));
        }
    }
    let t = std::time::Instant::now();
    lenh("file.exportMedia", e)?;
    let ms = t.elapsed().as_millis();
    let stats = lenh("perf.stats", json!({})).map(|v| v["export"].clone()).unwrap_or(Value::Null);
    Ok(json!({"xuat_ms": ms, "khau": stats}))
}

/// trạng thái cho recap-gui Màn 5: phần + chương (vị trí, độ dài, đã sửa tay?) — GUI không phải tự biết cách tính dấu.
pub fn trang_thai(goc: &Path) -> Kq<Value> {
    let Some(du) = phan::doc(&thu_muc(goc), &ten_du_an(goc))? else { return Ok(json!({"co": false})) };
    let t = |x: i64| x as f64 / 254_016_000_000.0;
    let mut chuong = vec![];
    for (k, e) in du.meta["chuong"].as_object().into_iter().flatten() {
        let Some((nhom, ma)) = k.split_once('/') else { continue };
        let sid = e["seq"].as_i64().unwrap_or(-1);
        let so_phan = du.meta["phan"][nhom].as_array().and_then(|p| p.iter().find(|p| p["seq"].as_i64() == Some(sid))).and_then(|p| p["so"].as_u64()).unwrap_or(0);
        chuong.push(json!({"nhom": nhom, "ma": ma, "seq": sid, "phan": so_phan, "bat_dau": t(e["bat_dau"].as_i64().unwrap_or(0)), "dai": t(e["dai"].as_i64().unwrap_or(0)),
            "da_sua_tay": phan::da_sua_tay(&du, k), "luc_s": e["luc_s"], "nguon": e["nguon"]}));
    }
    Ok(json!({"co": true, "du_an": thu_muc(goc).join(format!("{}.fcproj", ten_du_an(goc))), "chuong": chuong}))
}

/// đặt phần `so` làm sequence đang mở khi bật dự án (FilmCraft mở theo `view.active_sequence`)
pub fn mo_phan(goc: &Path, nhom: &str, so: u64) -> Kq<Value> {
    let (dir, ten) = (thu_muc(goc), ten_du_an(goc));
    let mut du = phan::doc(&dir, &ten)?.ok_or("chưa có dự án bộ")?;
    let sid = du.meta["phan"][nhom].as_array().and_then(|p| p.iter().find(|p| p["so"].as_u64() == Some(so))).and_then(|p| p["seq"].as_i64()).ok_or_else(|| format!("không có phần {so}"))?;
    du.doc["view"]["active_sequence"] = json!(sid);
    let mut ds = vec![json!(sid)];
    ds.extend(du.doc["view"]["open_sequences"].as_array().into_iter().flatten().filter(|x| x.as_i64() != Some(sid)).cloned());
    du.doc["view"]["open_sequences"] = Value::Array(ds);
    phan::luu(&dir, &ten, &du)?;
    Ok(json!({"du_an": dir.join(format!("{ten}.fcproj")), "seq": sid}))
}
