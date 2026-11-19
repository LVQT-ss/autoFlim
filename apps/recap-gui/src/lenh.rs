//! Mọi thao tác GHI của màn hình, đúng hợp đồng `DINH-DANG.md`: file lệnh vào `lenh/`, FILE MỚI vào `hang-viec/`
//! (ghi tạm + rename). Ngoài ra chỉ ghi công tắc của bộ vào `bo.json` (spec `00` mục C) và chuyển bộ vào Thùng rác.
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use crate::du_lieu::{Bo, SO_BUOC, Tt, bay_gio};

/// Ghi nguyên tử giống `ghi_json` của dịch vụ: file tạm cùng thư mục rồi rename.
pub fn ghi_nguyen_tu(p: &Path, v: &Value) -> std::io::Result<()> {
    if let Some(cha) = p.parent() {
        std::fs::create_dir_all(cha)?;
    }
    let ten = p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let tam = p.with_file_name(format!(".{ten}.{}.tmp", std::process::id()));
    std::fs::write(&tam, serde_json::to_vec_pretty(v)?)?;
    std::fs::rename(&tam, p)
}

fn ngau_nhien() -> String {
    format!("{:04x}", (bay_gio().to_bits() ^ u64::from(std::process::id()).rotate_left(16)) & 0xffff)
}

/// `{"lenh": dung|thu_lai|len_dau|huy, "id": …}` — dịch vụ áp dụng rồi xoá file.
pub fn gui_lenh(goc: &Path, lenh: &str, id: &str) -> std::io::Result<()> {
    let ten = format!("{}-{lenh}-{}.json", (bay_gio() * 1000.0) as u64, ngau_nhien());
    ghi_nguyen_tu(&goc.join("lenh").join(ten), &json!({"lenh": lenh, "id": id}))
}

/// Thêm 1 việc (file mới trong `hang-viec/`, đủ khoá như `them_viec` của dịch vụ).
pub fn them_viec(goc: &Path, loai: &str, bo: &Path, chuong: Option<&str>, tham_so: Value) -> std::io::Result<String> {
    let t = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let ten_bo = bo.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ten: String = format!("{loai}-{ten_bo}-{}", chuong.unwrap_or("bo")).chars().map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect();
    let id = format!("{t}-{}-{ten}", ngau_nhien());
    let bo_tuyet = std::fs::canonicalize(bo).unwrap_or_else(|_| bo.to_path_buf());
    let v = json!({"id": id, "loai": loai, "bo": bo_tuyet.to_string_lossy(), "chuong": chuong, "uu_tien": 5, "trang_thai": "cho",
        "lan_thu": 0, "lan_thoang_qua": 0, "ly_do": "", "gio_tao": bay_gio(), "gio_bat_dau": 0, "gio_xong": 0, "cho_toi": 0,
        "cho_tk": null, "pid": 0, "tien_do": null, "tham_so": tham_so, "ket_qua": null});
    ghi_nguyen_tu(&goc.join("hang-viec").join(format!("{id}.json")), &v)?;
    Ok(id)
}

/// Sửa vài khoá của `bo.json`, giữ nguyên mọi khoá khác.
pub fn sua_bo_json(bo: &Path, sua: impl FnOnce(&mut Value)) -> std::io::Result<()> {
    let p = bo.join("bo.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(&p)?)?;
    sua(&mut v);
    ghi_nguyen_tu(&p, &v)
}

fn tu_dong_obj(v: &mut Value) -> &mut Value {
    if !v["tu_dong"].is_object() {
        v["tu_dong"] = json!({"ca_bo": true, "buoc": vec![false; SO_BUOC]});
    }
    &mut v["tu_dong"]
}

pub fn dat_tu_dong_ca_bo(bo: &Path, bat: bool) -> std::io::Result<()> {
    sua_bo_json(bo, |v| tu_dong_obj(v)["ca_bo"] = json!(bat))
}

pub fn dat_tu_dong_buoc(bo: &Path, i: usize, bat: bool) -> std::io::Result<()> {
    sua_bo_json(bo, |v| {
        let t = tu_dong_obj(v);
        if !t["buoc"].is_array() {
            t["buoc"] = json!(vec![false; SO_BUOC]);
        }
        t["buoc"][i] = json!(bat);
    })
}

pub fn dat_co(bo: &Path, khoa: &'static str, gia_tri: bool) -> std::io::Result<()> {
    sua_bo_json(bo, |v| v[khoa] = json!(gia_tri))
}

/// Mở thư mục bằng trình quản lý file của hệ điều hành.
pub fn mo_thu_muc(p: &Path) {
    let lenh = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(lenh).arg(p).spawn();
}

/// Đưa bộ vào Thùng rác: đổi tên thư mục sang `<gốc>/.thung-rac/<tên>-<giờ>` (cùng ổ, khôi phục = đổi tên lại).
pub fn vao_thung_rac(bo: &Path) -> std::io::Result<PathBuf> {
    let goc = bo.parent().unwrap_or(bo);
    let ten = bo.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let dich = goc.join(".thung-rac").join(format!("{ten}-{}", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    std::fs::create_dir_all(goc.join(".thung-rac"))?;
    std::fs::rename(bo, &dich)?;
    Ok(dich)
}

pub struct BoMoi {
    pub ten: String,
    pub loai: String,
    pub may: String,
    pub nguon: String,
    pub noi_tiep_anime: bool,
    pub tu_dong: bool,
}

/// Tạo thư mục bộ + `bo.json`, rồi xếp việc `tai` (bước 1). Trả thư mục bộ.
pub fn tao_bo(goc_bo: &Path, goc_dv: &Path, b: &BoMoi) -> std::io::Result<PathBuf> {
    let ten: String = b.ten.trim().chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')).collect();
    if ten.is_empty() || ten.starts_with('.') {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "tên bộ trống hoặc không hợp lệ"));
    }
    let dir = goc_bo.join(&ten);
    if dir.exists() {
        return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "đã có bộ tên này"));
    }
    std::fs::create_dir_all(dir.join("nguon"))?;
    let v = json!({"ten": ten, "loai": b.loai, "may": b.may, "noi_tiep_anime": b.noi_tiep_anime, "da_dang": false,
        "nguon_tai": b.nguon, "tu_dong": {"ca_bo": b.tu_dong, "buoc": vec![false; SO_BUOC]}});
    ghi_nguyen_tu(&dir.join("bo.json"), &v)?;
    them_viec(goc_dv, "tai", &dir, None, json!({"nguon": b.nguon}))?;
    Ok(dir)
}

/// Việc nào Chạy được từ màn hình: bước có loại việc máy; bước agy cần mẫu đề (chưa có ở mốc này).
pub fn chay_buoc(goc_dv: &Path, bo: &Bo, i: usize) -> std::io::Result<usize> {
    let Some(loai) = crate::du_lieu::LOAI_BUOC[i] else { return Ok(0) };
    if loai == "agy" {
        return Ok(0);
    }
    if loai == "tai" || loai == "xuat" {
        them_viec(goc_dv, loai, &bo.thu_muc, None, json!({}))?;
        return Ok(1);
    }
    // timeline dựng từ chương đã có giọng; các bước còn lại làm chương chưa xong. Chương đã có việc xếp hàng/đang chạy thì bỏ qua.
    let nguon = if i == 9 { &bo.buoc[8] } else { &bo.buoc[i] };
    let co_viec: std::collections::BTreeSet<&str> = bo.buoc[i].dong.iter().filter(|d| d.viec.is_some() && matches!(d.tt, Tt::Chay | Tt::Chua | Tt::Cho)).map(|d| d.ten.as_str()).collect();
    let can: Vec<&str> = nguon.dong.iter().filter(|d| (i == 9) == d.xong && (i == 9 || !d.xong) && !co_viec.contains(d.ten.as_str())).map(|d| d.ten.as_str()).collect();
    for c in &can {
        them_viec(goc_dv, loai, &bo.thu_muc, Some(c), json!({}))?;
    }
    Ok(can.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chạy tay với dịch vụ thật/thử: `RECAP_DV_GOC=… RECAP_TEST_BO=… RECAP_TEST_ID=<id việc> cargo test -p recap-gui -- --ignored lenh_that`.
    #[test]
    #[ignore]
    fn lenh_that() {
        let goc = PathBuf::from(std::env::var("RECAP_DV_GOC").unwrap());
        let bo = PathBuf::from(std::env::var("RECAP_TEST_BO").unwrap());
        gui_lenh(&goc, &std::env::var("RECAP_TEST_LENH").unwrap(), &std::env::var("RECAP_TEST_ID").unwrap()).unwrap();
        them_viec(&goc, "do_khung", &bo, Some("ch139"), json!({})).unwrap();
    }

    #[test]
    fn ghi_cong_tac_giu_khoa_khac() {
        let d = std::env::temp_dir().join(format!("recap-gui-lenh-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("bo.json"), r#"{"ten":"X","giong":["a"]}"#).unwrap();
        dat_tu_dong_buoc(&d, 3, true).unwrap();
        dat_tu_dong_ca_bo(&d, false).unwrap();
        let v: Value = serde_json::from_slice(&std::fs::read(d.join("bo.json")).unwrap()).unwrap();
        assert_eq!(v["ten"], "X");
        assert_eq!(v["tu_dong"]["buoc"][3], true);
        assert_eq!(v["tu_dong"]["ca_bo"], false);
        let _ = std::fs::remove_dir_all(&d);
    }
}
