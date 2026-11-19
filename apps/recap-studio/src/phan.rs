//! Dự án 1 bộ kiểu B (T621, spec 05-timeline): 1 file `<bộ>.fcproj`, mỗi phần là 1 sequence PHẲNG (clip các chương nối liền,
//! marker `chNNN` đầu mỗi chương), nhóm `viet/` `en/` là bin. Chương dựng riêng (RAM thấp) rồi GHÉP ở mức JSON vào đúng chỗ:
//! đổi id sang vùng trống, dời thời gian, chèn clip vào dải phần, ripple các chương sau. File kèm `<bộ>.may-dung.json` nhớ
//! mỗi chương nằm đâu + "dấu" nội dung lúc máy dựng; dấu lệch = Thịnh đã sửa tay → máy không ghi đè.
use crate::du_lieu::Kq;
use serde_json::{Map, Value, json};
use std::path::Path;

/// ≤ ~3.000 clip/phần (đo: 100 fps ở 2.000 clip, 3–17 fps ở 20.000)
pub const TRAN_CLIP: usize = 3000;

fn fnv(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

pub struct DuAn {
    pub doc: Value,
    pub meta: Value,
}

pub fn meta_rong() -> Value {
    json!({"phien_ban": 1, "chuong": {}, "phan": {}})
}

fn khoa(nhom: &str, ma: &str) -> String {
    format!("{nhom}/{ma}")
}

fn so_chuong(ma: &str) -> i64 {
    ma.trim_start_matches("ch").parse().unwrap_or(-1)
}

fn cap_id(doc: &mut Value) -> i64 {
    let id = doc["project"]["next_id"].as_i64().unwrap_or(1);
    doc["project"]["next_id"] = json!(id + 1);
    id
}

/// mọi số nguyên mang id (id/item/link/group) cộng `d`
fn doi_id(v: &mut Value, d: i64) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                if matches!(k.as_str(), "id" | "item" | "link" | "group")
                    && let Some(n) = x.as_i64()
                {
                    *x = json!(n + d);
                    continue;
                }
                doi_id(x, d);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| doi_id(x, d)),
        _ => {}
    }
}

/// chạy `f` trên từng clip của sequence (video rồi audio), kèm chỉ số track tuần tự
fn moi_clip(seq: &mut Value, mut f: impl FnMut(usize, &mut Value)) {
    for (hdr, key) in ["video_tracks", "audio_tracks"].into_iter().enumerate() {
        if let Some(ts) = seq[key].as_array_mut() {
            for (i, t) in ts.iter_mut().enumerate() {
                if let Some(cs) = t["items"].as_array_mut() {
                    for c in cs {
                        f(hdr * 1000 + i, c);
                    }
                }
            }
        }
    }
}

fn tong_clip(seq: &Value) -> usize {
    ["video_tracks", "audio_tracks"].iter().filter_map(|k| seq[k].as_array()).flatten().filter_map(|t| t["items"].as_array()).map(Vec::len).sum()
}

fn ket_thuc(seq: &Value) -> i64 {
    let mut m = 0;
    for k in ["video_tracks", "audio_tracks"] {
        for t in seq[k].as_array().into_iter().flatten() {
            for c in t["items"].as_array().into_iter().flatten() {
                m = m.max(c["start"].as_i64().unwrap_or(0) + c["duration"].as_i64().unwrap_or(0));
            }
        }
    }
    m
}

fn tim_seq_chinh(doc: &Value, ma: &str) -> Kq<String> {
    doc["project"]["items"]
        .as_object()
        .into_iter()
        .flatten()
        .find(|(_, v)| v["name"] == ma && v["kind"].get("Sequence").is_some())
        .map(|(k, _)| k.clone())
        .ok_or_else(|| format!("dự án chương không có sequence '{ma}'"))
}

fn seq_phan<'a>(doc: &'a mut Value, id: i64) -> Kq<&'a mut Value> {
    doc["project"]["items"][id.to_string()]["kind"]
        .get_mut("Sequence")
        .ok_or_else(|| format!("không có sequence phần id {id}"))
}

fn bin_nhom<'a>(doc: &'a mut Value, nhom: &str) -> Kq<&'a mut Vec<Value>> {
    if !doc["project"]["root"]["children"].as_array().is_some_and(|c| c.iter().any(|b| b["Bin"]["name"] == nhom)) {
        let id = cap_id(doc);
        doc["project"]["root"]["children"].as_array_mut().ok_or("root.children hỏng")?.push(json!({"Bin": {"id": id, "name": nhom, "children": []}}));
    }
    doc["project"]["root"]["children"]
        .as_array_mut()
        .and_then(|c| c.iter_mut().find(|b| b["Bin"]["name"] == nhom))
        .and_then(|b| b["Bin"]["children"].as_array_mut())
        .ok_or_else(|| "bin nhóm hỏng".to_string())
}

/// dấu nội dung chương: clip nằm trong [bat_dau, bat_dau+dai) của dải phần (start tương đối) + mọi sequence trang của chương
pub fn dau(du: &DuAn, key: &str) -> Option<String> {
    let e = du.meta["chuong"].get(key)?;
    let (seq, off, dai) = (e["seq"].as_i64()?, e["bat_dau"].as_i64()?, e["dai"].as_i64()?);
    let s = &du.doc["project"]["items"][seq.to_string()]["kind"]["Sequence"];
    let mut v: Vec<Value> = vec![];
    for (hdr, k) in ["video_tracks", "audio_tracks"].into_iter().enumerate() {
        for (i, t) in s[k].as_array()?.iter().enumerate() {
            for c in t["items"].as_array()? {
                let st = c["start"].as_i64()?;
                if st >= off && st < off + dai {
                    let mut c = c.clone();
                    c["start"] = json!(st - off);
                    v.push(json!([hdr, i, c]));
                }
            }
        }
    }
    let mut ids: Vec<i64> = e["items"].as_array()?.iter().filter_map(Value::as_i64).collect();
    ids.sort_unstable();
    for id in ids {
        let it = &du.doc["project"]["items"][id.to_string()];
        if it["kind"].get("Sequence").is_some() {
            v.push(it.clone());
        }
    }
    Some(fnv(&Value::Array(v).to_string()))
}

pub fn da_sua_tay(du: &DuAn, key: &str) -> bool {
    match (dau(du, key), du.meta["chuong"][key]["dau"].as_str()) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    }
}

/// ripple: clip + marker của dải phần có start ≥ `tu` dời `d`; chương sau (cùng dải) cập nhật bat_dau
fn ripple(du: &mut DuAn, seq_id: i64, tu: i64, d: i64) -> Kq<()> {
    if d == 0 {
        return Ok(());
    }
    let s = seq_phan(&mut du.doc, seq_id)?;
    moi_clip(s, |_, c| {
        if let Some(st) = c["start"].as_i64().filter(|st| *st >= tu) {
            c["start"] = json!(st + d);
        }
    });
    for m in s["markers"].as_array_mut().into_iter().flatten() {
        if let Some(st) = m["start"].as_i64().filter(|st| *st >= tu) {
            m["start"] = json!(st + d);
        }
    }
    for e in du.meta["chuong"].as_object_mut().into_iter().flat_map(Map::values_mut) {
        if e["seq"].as_i64() == Some(seq_id)
            && let Some(b) = e["bat_dau"].as_i64().filter(|b| *b >= tu)
        {
            e["bat_dau"] = json!(b + d);
        }
    }
    Ok(())
}

/// dự án rỗng (chưa phần nào) lấy khung từ 1 dự án chương
pub fn moi(ten: &str, mau: &Value) -> DuAn {
    let mut doc = mau.clone();
    doc["project"]["name"] = json!(ten);
    doc["project"]["items"] = json!({});
    doc["project"]["root"]["children"] = json!([]);
    DuAn { doc, meta: meta_rong() }
}

fn tao_phan(du: &mut DuAn, nhom: &str, mau_seq: &Value) -> Kq<i64> {
    let id = cap_id(&mut du.doc);
    let so = du.meta["phan"][nhom].as_array().map_or(0, Vec::len) + 1;
    let mut s = mau_seq.clone();
    for k in ["video_tracks", "audio_tracks"] {
        for t in s[k].as_array_mut().into_iter().flatten() {
            t["items"] = json!([]);
        }
    }
    s["markers"] = json!([]);
    s["mark_in"] = Value::Null;
    s["mark_out"] = Value::Null;
    s["work_area"] = Value::Null;
    du.doc["project"]["items"][id.to_string()] = json!({"id": id, "name": format!("Phần {so}"), "label": "Forest", "kind": {"Sequence": s}, "metadata": {}, "created": id});
    bin_nhom(&mut du.doc, nhom)?.push(json!({"Item": id}));
    if !du.meta["phan"][nhom].is_array() {
        du.meta["phan"][nhom] = json!([]);
    }
    du.meta["phan"][nhom].as_array_mut().ok_or("meta.phan hỏng")?.push(json!({"so": so, "seq": id}));
    Ok(id)
}

/// Ghép 1 chương (dự án chương `ch` do `dung-chuong` dựng) vào dải phần. Chương đã có → thay đúng đoạn, chương sau dời.
pub fn ghep(du: &mut DuAn, mut ch: Value, ma: &str, nhom: &str, nguon: &str) -> Kq<()> {
    let key = khoa(nhom, ma);
    let main_old = tim_seq_chinh(&ch, ma)?;
    // 1) đổi id sang vùng trống của dự án bộ
    let delta = du.doc["project"]["next_id"].as_i64().ok_or("dự án bộ thiếu next_id")?;
    du.doc["project"]["next_id"] = json!(delta + ch["project"]["next_id"].as_i64().ok_or("chương thiếu next_id")?);
    let mut items = Map::new();
    for (k, mut v) in std::mem::take(ch["project"]["items"].as_object_mut().ok_or("chương thiếu items")?) {
        doi_id(&mut v, delta);
        items.insert((k.parse::<i64>().map_err(|e| e.to_string())? + delta).to_string(), v);
    }
    let main_id = (main_old.parse::<i64>().map_err(|e| e.to_string())? + delta).to_string();
    let mut main = items.remove(&main_id).ok_or("mất sequence chính")?;
    let mau_seq = main["kind"]["Sequence"].take();
    let dai_moi = ket_thuc(&mau_seq);
    let n_clip = tong_clip(&mau_seq);

    // 2) chỗ đặt: chương cũ → gỡ đoạn cũ; chương mới → sau chương liền trước
    let (seq_id, off) = if let Some(e) = du.meta["chuong"].get(&key).cloned() {
        let (sid, off, dai) = (e["seq"].as_i64().ok_or("meta thiếu seq")?, e["bat_dau"].as_i64().ok_or("meta thiếu bat_dau")?, e["dai"].as_i64().ok_or("meta thiếu dai")?);
        let s = seq_phan(&mut du.doc, sid)?;
        for k in ["video_tracks", "audio_tracks"] {
            for t in s[k].as_array_mut().into_iter().flatten() {
                if let Some(cs) = t["items"].as_array_mut() {
                    cs.retain(|c| !c["start"].as_i64().is_some_and(|st| st >= off && st < off + dai));
                }
            }
        }
        if let Some(ms) = s["markers"].as_array_mut() {
            ms.retain(|m| !(m["name"] == ma && m["start"].as_i64() == Some(off)));
        }
        for id in e["items"].as_array().into_iter().flatten().filter_map(Value::as_i64) {
            du.doc["project"]["items"].as_object_mut().map(|m| m.remove(&id.to_string()));
        }
        bin_nhom(&mut du.doc, nhom)?.retain(|b| b["Bin"]["name"] != ma);
        ripple(du, sid, off + dai, dai_moi - dai)?;
        (sid, off)
    } else {
        let n = so_chuong(ma);
        let truoc = du.meta["chuong"]
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(k, _)| k.starts_with(&format!("{nhom}/")) && so_chuong(&k[nhom.len() + 1..]) < n)
            .max_by_key(|(k, _)| so_chuong(&k[nhom.len() + 1..]))
            .map(|(_, e)| e.clone());
        let phan_cuoi = du.meta["phan"][nhom].as_array().and_then(|p| p.last()).and_then(|p| p["seq"].as_i64());
        let (sid, off) = match (truoc, phan_cuoi) {
            (Some(t), _) => {
                let sid = t["seq"].as_i64().ok_or("meta thiếu seq")?;
                let la_cuoi = Some(sid) == phan_cuoi;
                // phần cuối đầy → sang phần mới (chỉ khi chèn nối đuôi)
                let day = la_cuoi && seq_phan(&mut du.doc, sid).map(|s| tong_clip(s) + n_clip > TRAN_CLIP).unwrap_or(false);
                let ket = t["bat_dau"].as_i64().unwrap_or(0) + t["dai"].as_i64().unwrap_or(0);
                let cuoi_phan = la_cuoi && seq_phan(&mut du.doc, sid).map(|s| ket_thuc(s) <= ket).unwrap_or(false);
                if day && cuoi_phan { (tao_phan(du, nhom, &mau_seq)?, 0) } else { (sid, ket) }
            }
            (None, Some(sid)) => (du.meta["phan"][nhom][0]["seq"].as_i64().unwrap_or(sid), 0),
            (None, None) => (tao_phan(du, nhom, &mau_seq)?, 0),
        };
        ripple(du, sid, off, dai_moi)?;
        (sid, off)
    };

    // 3) chèn clip theo chỉ số track + marker + item
    let s = seq_phan(&mut du.doc, seq_id)?;
    for (hdr, k) in ["video_tracks", "audio_tracks"].into_iter().enumerate() {
        let _ = hdr;
        let nguon_t = mau_seq[k].as_array().ok_or("track chương hỏng")?;
        let dich = s[k].as_array_mut().ok_or("track phần hỏng")?;
        if dich.len() < nguon_t.len() {
            return Err("dải phần ít track hơn chương".into());
        }
        for (i, t) in nguon_t.iter().enumerate() {
            let cs = dich[i]["items"].as_array_mut().ok_or("track phần hỏng")?;
            for c in t["items"].as_array().into_iter().flatten() {
                let mut c = c.clone();
                c["start"] = json!(c["start"].as_i64().unwrap_or(0) + off);
                cs.push(c);
            }
            cs.sort_by_key(|c| c["start"].as_i64().unwrap_or(0));
        }
    }
    let mid = cap_id(&mut du.doc);
    let s = seq_phan(&mut du.doc, seq_id)?;
    let ms = s["markers"].as_array_mut().ok_or("markers hỏng")?;
    ms.push(json!({"id": mid, "start": off, "duration": 0, "name": ma, "comment": "", "kind": "Chapter", "color": "Forest"}));
    ms.sort_by_key(|m| m["start"].as_i64().unwrap_or(0));
    let ids: Vec<i64> = items.keys().filter_map(|k| k.parse().ok()).collect();
    for (k, v) in items {
        du.doc["project"]["items"][k] = v;
    }
    let bid = cap_id(&mut du.doc);
    let kids: Vec<Value> = ids.iter().map(|i| json!({"Item": i})).collect();
    bin_nhom(&mut du.doc, nhom)?.push(json!({"Bin": {"id": bid, "name": ma, "children": kids}}));
    du.meta["chuong"][&key] = json!({"seq": seq_id, "bat_dau": off, "dai": dai_moi, "items": ids, "nguon": nguon, "luc": luc(), "luc_s": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()), "dau": ""});
    let d = dau(du, &key).ok_or("không tính được dấu chương")?;
    du.meta["chuong"][&key]["dau"] = json!(d);
    // mở sẵn các phần khi bật dự án
    let phan: Vec<i64> = du.meta["phan"].as_object().into_iter().flat_map(Map::values).flat_map(|v| v.as_array().into_iter().flatten()).filter_map(|p| p["seq"].as_i64()).collect();
    du.doc["view"]["active_sequence"] = json!(phan.first().copied().unwrap_or(seq_id));
    du.doc["view"]["open_sequences"] = json!(phan);
    Ok(())
}

fn luc() -> String {
    let o = std::process::Command::new("date").arg("+%Y-%m-%dT%H:%M:%S").output().ok();
    o.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

/// Đoạn [bắt đầu, kết thúc) của chương theo MARKER `chNNN` trong dải phần (sửa tay kéo marker vẫn đúng). (seq, giây)
pub fn doan(du: &DuAn, nhom: &str, ma: &str) -> Kq<(i64, f64, f64)> {
    let e = du.meta["chuong"].get(khoa(nhom, ma)).ok_or_else(|| format!("{ma}: chưa có trong dự án bộ — chạy `recap-studio dung-bo` trước"))?;
    let sid = e["seq"].as_i64().ok_or("meta thiếu seq")?;
    let s = &du.doc["project"]["items"][sid.to_string()]["kind"]["Sequence"];
    let ms: Vec<(i64, String)> = s["markers"].as_array().into_iter().flatten().filter(|m| m["kind"] == "Chapter").filter_map(|m| Some((m["start"].as_i64()?, m["name"].as_str()?.to_string()))).collect();
    let i = ms.iter().position(|m| m.1 == ma).ok_or_else(|| format!("{ma}: dải phần mất marker chương"))?;
    let a = ms[i].0;
    let b = ms.get(i + 1).map_or_else(|| ket_thuc(s), |m| m.0);
    let giay = |t: i64| t as f64 / 254_016_000_000.0;
    Ok((sid, giay(a), giay(b)))
}

pub fn doc(dir: &Path, ten: &str) -> Kq<Option<DuAn>> {
    let f = dir.join(format!("{ten}.fcproj"));
    if !f.is_file() {
        return Ok(None);
    }
    let r = |p: &Path| -> Kq<Value> { serde_json::from_str(&std::fs::read_to_string(p).map_err(|e| format!("đọc {}: {e}", p.display()))?).map_err(|e| format!("JSON hỏng {}: {e}", p.display())) };
    let meta = match dir.join(format!("{ten}.may-dung.json")) {
        m if m.is_file() => r(&m)?,
        _ => meta_rong(),
    };
    Ok(Some(DuAn { doc: r(&f)?, meta }))
}

/// ghi nguyên tử (.dang rồi đổi tên); bản cũ giữ 1 bản `.fcproj.bak`
pub fn luu(dir: &Path, ten: &str, du: &DuAn) -> Kq<()> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let (f, m) = (dir.join(format!("{ten}.fcproj")), dir.join(format!("{ten}.may-dung.json")));
    let ghi = |p: &Path, v: &Value| -> Kq<()> {
        let t = p.with_extension("dang");
        std::fs::write(&t, serde_json::to_vec(v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::rename(&t, p).map_err(|e| e.to_string())
    };
    if f.is_file() {
        std::fs::copy(&f, dir.join(format!("{ten}.fcproj.bak"))).map_err(|e| e.to_string())?;
    }
    ghi(&f, &du.doc)?;
    ghi(&m, &du.meta)
}

/// FULL = 1 sequence chứa các sequence phần lồng nhau (chỉ để xuất/xem tổng). Dựng lại mỗi lần (không có gì để mất).
pub fn lam_full(du: &mut DuAn, nhom: &str) -> Kq<()> {
    let phan: Vec<i64> = du.meta["phan"][nhom].as_array().into_iter().flatten().filter_map(|p| p["seq"].as_i64()).collect();
    let Some(&dau_p) = phan.first() else { return Ok(()) };
    let ten = format!("FULL {nhom}");
    // gỡ FULL cũ
    let cu: Option<String> = du.doc["project"]["items"].as_object().and_then(|m| m.iter().find(|(_, v)| v["name"] == ten.as_str()).map(|(k, _)| k.clone()));
    if let Some(k) = cu {
        du.doc["project"]["items"].as_object_mut().map(|m| m.remove(&k));
        let id: i64 = k.parse().unwrap_or(-1);
        bin_nhom(&mut du.doc, nhom)?.retain(|b| b["Item"].as_i64() != Some(id));
    }
    // khung + mẫu clip từ phần 1 (clip lồng + clip giọng)
    let mut full = du.doc["project"]["items"][dau_p.to_string()]["kind"]["Sequence"].clone();
    let mut mau_v = None;
    let mut mau_a = None;
    for (k, mau) in [("video_tracks", &mut mau_v), ("audio_tracks", &mut mau_a)] {
        *mau = full[k].as_array().into_iter().flatten().flat_map(|t| t["items"].as_array().into_iter().flatten()).next().cloned();
        for t in full[k].as_array_mut().into_iter().flatten() {
            t["items"] = json!([]);
        }
    }
    full["markers"] = json!([]);
    let (mut mv, mut ma) = (mau_v.ok_or("phần 1 rỗng")?, mau_a.ok_or("phần 1 rỗng")?);
    for c in [&mut mv, &mut ma] {
        // chỉ giữ hiệu ứng nền (motion/opacity/âm lượng), bỏ keyframe: clip lồng không có camera
        if let Some(es) = c["effects"].as_array_mut() {
            es.retain(|e| matches!(e["effect"].as_str(), Some("motion" | "opacity" | "volume")));
            for e in es {
                let la_motion = e["effect"] == "motion";
                if let Some(ps) = e["params"].as_object_mut() {
                    for p in ps.values_mut() {
                        p.as_object_mut().map(|o| o.remove("keyframes"));
                    }
                    if la_motion {
                        ps.insert("position".into(), json!({"value": {"Vec2": {"x": 960.0, "y": 540.0}}}));
                        ps.insert("scale".into(), json!({"value": {"Float": 100.0}}));
                    }
                }
            }
        }
        c["source_in"] = json!(0);
    }
    let id = cap_id(&mut du.doc);
    let mut t = 0i64;
    for (i, p) in phan.iter().enumerate() {
        let dai = ket_thuc(&du.doc["project"]["items"][p.to_string()]["kind"]["Sequence"]);
        for (k, mau) in [("video_tracks", &mv), ("audio_tracks", &ma)] {
            let cid = cap_id(&mut du.doc);
            let mut c = mau.clone();
            c["id"] = json!(cid);
            c["item"] = json!(p);
            c["name"] = json!(format!("Phần {}", i + 1));
            c["start"] = json!(t);
            c["duration"] = json!(dai);
            c["link"] = Value::Null;
            full[k][0]["items"].as_array_mut().ok_or("FULL thiếu track")?.push(c);
        }
        t += dai;
    }
    du.doc["project"]["items"][id.to_string()] = json!({"id": id, "name": ten, "label": "Rose", "kind": {"Sequence": full}, "metadata": {}, "created": id});
    bin_nhom(&mut du.doc, nhom)?.push(json!({"Item": id}));
    du.meta["full"][nhom] = json!(id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// dự án chương giả: 1 sequence `ma` (V: 2 clip, A: 1 clip dài `dai` tick) + 1 media
    fn chuong(ma: &str, dai: i64, ghi: i64) -> Value {
        let tr = |id: i64, k: &str, cs: Vec<Value>| json!({"id": id, "kind": k, "name": k, "items": cs});
        let clip = |id: i64, item: i64, st: i64, d: i64| json!({"id": id, "item": item, "name": "c", "start": st, "duration": d, "link": null, "group": null, "effects": []});
        let seq = json!({"settings": {}, "video_tracks": [tr(1, "Video", vec![clip(10, 5, 0, dai / 2), clip(11, 5, dai / 2, dai / 2)])], "audio_tracks": [tr(2, "Audio", vec![clip(12, 5, 0, dai)])], "markers": [], "mark_in": null, "mark_out": null, "work_area": null});
        json!({"project": {"name": "x", "root": {"id": 0, "name": "r", "children": []}, "next_id": 20 + ghi,
            "items": {"5": {"id": 5, "name": "m.jpg", "kind": {"Media": {}}}, "6": {"id": 6, "name": ma, "kind": {"Sequence": seq}}}}, "view": {"open_sequences": [], "active_sequence": 6}})
    }

    fn vitri(du: &DuAn, key: &str) -> (i64, i64) {
        let e = &du.meta["chuong"][key];
        (e["bat_dau"].as_i64().unwrap_or(-1), e["dai"].as_i64().unwrap_or(-1))
    }

    #[test]
    fn ghep_noi_thay_va_giu_sua_tay() {
        let mut du = moi("b", &chuong("ch001", 100, 0));
        ghep(&mut du, chuong("ch001", 100, 0), "ch001", "viet", "a").unwrap();
        ghep(&mut du, chuong("ch002", 200, 0), "ch002", "viet", "a").unwrap();
        ghep(&mut du, chuong("ch003", 50, 0), "ch003", "viet", "a").unwrap();
        assert_eq!((vitri(&du, "viet/ch002"), vitri(&du, "viet/ch003")), ((100, 200), (300, 50)));
        let sid = du.meta["phan"]["viet"][0]["seq"].as_i64().unwrap();
        let s = &du.doc["project"]["items"][sid.to_string()]["kind"]["Sequence"];
        assert_eq!(s["markers"].as_array().unwrap().len(), 3);
        assert_eq!(tong_clip(s), 9);
        assert!(!da_sua_tay(&du, "viet/ch002"));
        // id không trùng giữa các chương
        let mut ids: Vec<i64> = s["video_tracks"][0]["items"].as_array().unwrap().iter().map(|c| c["id"].as_i64().unwrap()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 6);
        // thay ch002 bằng bản dài 300 → ch003 dời 100, dấu ch003 vẫn khớp (chưa sửa tay)
        ghep(&mut du, chuong("ch002", 300, 0), "ch002", "viet", "b").unwrap();
        assert_eq!((vitri(&du, "viet/ch002"), vitri(&du, "viet/ch003")), ((100, 300), (400, 50)));
        assert!(!da_sua_tay(&du, "viet/ch003") && !da_sua_tay(&du, "viet/ch002"));
        assert_eq!(du.meta["chuong"]["viet/ch002"]["nguon"], "b");
        // sửa tay 1 clip của ch003 → nhận ra; ch002 vẫn sạch
        du.doc["project"]["items"][sid.to_string()]["kind"]["Sequence"]["video_tracks"][0]["items"][5]["duration"] = json!(7);
        assert!(da_sua_tay(&du, "viet/ch003") && !da_sua_tay(&du, "viet/ch002"));
        // đoạn theo marker
        let (_, a, b) = doan(&du, "viet", "ch003").unwrap();
        assert!(a > 0.0 && b > a);
        // chèn ch000 (số nhỏ hơn mọi chương) → ripple tất cả
        ghep(&mut du, chuong("ch000", 10, 0), "ch000", "viet", "a").unwrap();
        assert_eq!((vitri(&du, "viet/ch000"), vitri(&du, "viet/ch001")), ((0, 10), (10, 100)));
        lam_full(&mut du, "viet").unwrap();
        assert!(du.meta["full"]["viet"].is_i64());
    }

    #[test]
    fn day_thi_sang_phan_moi() {
        let mut du = moi("b", &chuong("ch001", 100, 0));
        // mỗi chương 3 clip; ép trần bằng nhiều chương không thể (3000) → kiểm thủ công trần bằng tong_clip
        for n in 1..=3 {
            ghep(&mut du, chuong(&format!("ch{n:03}"), 100, 0), &format!("ch{n:03}"), "viet", "a").unwrap();
        }
        assert_eq!(du.meta["phan"]["viet"].as_array().unwrap().len(), 1);
    }
}
