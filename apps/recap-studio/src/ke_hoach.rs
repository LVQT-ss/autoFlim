//! Kế hoạch trượt nhanh do engine cũ sinh (`truot-nhanh.py --ke-hoach`); app cũ và app mới đọc CHÍNH file này.
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, Copy)]
pub struct Cam {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct KhungNen {
    pub tu: f64,
    pub nen: String,
    #[serde(default)]
    pub tron: Option<f64>,
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub struct Nhoe {
    pub t: f64,
    pub doc: f64,
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub struct Lia {
    pub t: f64,
    pub dai: f64,
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub struct Lap {
    pub tu: f64,
    pub dai: f64,
    pub ho: f64,
    pub buoc: f64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TrangSong {
    pub khoa: String,
    pub anh: String,
    pub nen: String,
    #[serde(default)]
    pub khung: Vec<KhungNen>,
    pub dai_anh: [f64; 2],
    pub tu: f64,
    pub den: f64,
    pub cam: Vec<Cam>,
    #[serde(default)]
    pub nhoe: Vec<Nhoe>,
    pub lia: Option<Lia>,
    pub lap: Option<Lap>,
}

#[derive(Deserialize, Debug)]
pub struct KeHoach {
    #[serde(rename = "W")]
    pub w: f64,
    #[serde(rename = "H")]
    pub h: f64,
    pub trang: Vec<TrangSong>,
}

/// camera nội suy tuyến tính theo thời gian (như `camTai` của truot-song.ts)
pub fn cam_tai(cam: &[Cam], t: f64) -> Cam {
    let (Some(a0), Some(z)) = (cam.first(), cam.last()) else { return Cam { t, x: 0.0, y: 0.0, zoom: 1.0 } };
    if t <= a0.t {
        return *a0;
    }
    if t >= z.t {
        return *z;
    }
    let i = cam.iter().position(|c| c.t >= t).unwrap_or(cam.len() - 1).max(1);
    let (a, b) = (cam[i - 1], cam[i]);
    let q = if b.t > a.t { (t - a.t) / (b.t - a.t) } else { 0.0 };
    Cam { t, x: a.x + (b.x - a.x) * q, y: a.y + (b.y - a.y) * q, zoom: a.zoom + (b.zoom - a.zoom) * q }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cam_noi_suy_va_kep_dau_cuoi() {
        let c = [Cam { t: 1.0, x: 0.0, y: 0.0, zoom: 1.0 }, Cam { t: 3.0, x: 1.0, y: 2.0, zoom: 2.0 }];
        assert_eq!(cam_tai(&c, 0.0).x, 0.0);
        assert_eq!(cam_tai(&c, 9.0).y, 2.0);
        let m = cam_tai(&c, 2.0);
        assert!((m.x - 0.5).abs() < 1e-9 && (m.zoom - 1.5).abs() < 1e-9);
    }
}
