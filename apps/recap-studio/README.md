# recap-studio

Lõi dựng chương manga cho app Recap Studio (spec: `specs/app-moi/05-timeline.md`, `08-engine.md` ở repo YTB_AUTOMATION).
Dựa trên FilmCraft (MIT OR Apache-2.0, xem `ATTRIBUTION.md`); crate này chỉ gọi lệnh `timeline.*`/`effects.*`/`file.*` của lõi.

```
recap-studio dung-chuong <thư mục bộ> chNNN [--ra video.mp4] [--du-an chuong.fcproj]
                         [--bitrate-kbps N] [--doan từ,đến] [--thu N] [--kieu K] [--cache dir] [--gpu off|auto] [--ma-hoa off|auto]
recap-studio xuat <thư mục bộ> (--phan N | --full | --chuong A-B [--nhan tên]) [--ra-dir ~/recap-thu-moc2] [--bitrate-kbps 3000]
                  [--song-song N] [--chia gio|arc|tay|full] [--phut N] [--mo-dau file.mp4]
```

## `xuat` (mốc 2, T614)
- Danh sách video theo `xuat.chia` của `_du-an.json` (`gio` + `phut`, `arc` = mỗi `Script/arcNN-chA-B`, `tay` + `phan`, thiếu = chỉ FULL; `--chia` ép).
  Luôn có `FULL`. `--chuong A-B` = khoảng tự chọn (không ghép mở đầu trừ khi có `--mo-dau`).
- Mỗi chương dựng + xuất bằng **tiến trình con** `dung-chuong` (`.tam/chNNN.mp4`; RAM chỉ ~1 chương, đoạn đã đạt và mới hơn mốc câu được dùng lại khi chạy lại),
  rồi nối `ffmpeg -c copy`. Mở đầu `<bộ>/Xuat/<tên> - mo-dau.mp4` (FULL + phần đầu; `tay`: mọi phần) được mã lại đúng thông số đoạn chương rồi nối.
- Tự kiểm như `kiem_video`: có hình + tiếng · độ dài ±1 s so với tổng mốc câu (+ mở đầu) · tiếng không hụt > 2 s · 5 khung rải đều không đen.
  Lỗi → file đổi tên `….loi.mp4`, sổ ghi lý do, **không đè** bản cũ đạt; bản mới đạt thì bản cũ vào `.thung-rac/`.
- Sổ `<ra-dir>/<bộ>/so-xuat.json`: file, nhãn, khoảng chương, giây, MB, MB/phút, lúc, kiểm, mở đầu, `moc_chuong` (giây đã cộng mở đầu), giây xuất từng chương.
- Dừng giữa chừng: mọi file đang ghi mang đuôi `.dang*.mp4` (đổi tên khi xong, lần chạy sau tự dọn); đoạn chương đã xong được giữ. Xuất treo > 4× độ dài (≥ 10 phút) bị dừng + báo.
- Không ghi gì vào thư mục bộ (CHỈ ĐỌC).

- Đọc dữ liệu bộ **định dạng cũ, CHỈ ĐỌC** (`Video/chNNN/_technical/moc-neo-*.json`, `chapter-plan.json`, `truot-song/ke-hoach-*.json`,
  `panel-maps/`, `Manga/chNNN - */`, `Script/<arc>/voice-*/chNNN.mp3`, `_ten-rieng.json`, `_du-an.json`).
  Ảnh trong `Manga/` đã xoá chữ (định dạng cũ ghi đè); mảnh vá `che-chu/` của định dạng mới chưa có ở mốc này.
- Ghi: `--du-an` (.fcproj, chỉ ghi đường dẫn media — không chép ảnh), `--ra` (mp4), `--cache` (nền mờ nhỏ, xoá được).
- Kiểu dựng theo trang (`hieu_ung_trang` trong `chapter-plan.json`, mặc định `kieu_dung` của bộ): `truot-song` (Trượt nhanh, port
  `truot-song.ts`: camera, nhoè dọc, hình lia, 2 bản lặp, nền theo khung), `tinh` (Cả trang), `tinh-phong`, `quet-doc` (Trượt dọc),
  `camera-path` (Theo khung, từ `panel-maps`). `--kieu K` ép 1 kiểu cho mọi trang (để thử).
- Dải trang đi vào **sequence lồng 1080p** để Directional Blur chạy trên khung hình, không trên ảnh dải cao tới 7.000 px
  (xuất ch107 nhanh ~3×, hình khác ≤ MSE 6/255²).
- `tools/so_khung.py`: so khung video dựng với khung app cũ.

## Số đo (Mac M-series 16 GB, 19/11, `--bitrate-kbps 3000`)
| Chương | Dựng lệnh | Xuất | RAM đỉnh | Dung lượng |
|---|---|---|---|---|
| Nisekoi ch107 (288 s, 18 trang, trượt nhanh) | 2,7 s | 291 s (mốc 0: 1.519 s) | 4,0 GB | 119,6 MB = 24,9 MB/phút |
| Beelzebub ch143 (277 s, arc02, chưa lên FreeCut) | 2,8 s | 281 s | 4,5 GB | 114,9 MB |
| Beelzebub ch139 (273 s, arc01) | 2,1 s | 237 s | 4,1 GB | 113,4 MB |

### `xuat` (Mac M-series 16 GB, 19/11, 3.000 kbps, song song 1 trừ ghi chú)
| Video | Thời lượng | Xuất | RAM đỉnh (1 tiến trình con) | Dung lượng | Tự kiểm |
|---|---|---|---|---|---|
| Beelzebub ch143–155 + mở đầu 45 s (`--song-song 2`) | 63 ph 29 s | 4.275 s (1,12×) | 5,6 GB ×2 tiến trình | 1,58 GB = 24,9 MB/phút | đạt (3.808,7 s, mong 3.808,5 s) |
| Nisekoi Phần 1 ch107–132 + mở đầu 46 s (`--chia gio --phut 120`) | 2 giờ 02 ph | ~6.790 s (0,93×; ghi 6.241 s vì ch107–108 dùng lại) | 5,8 GB | 3,05 GB = 24,9 MB/phút | đạt (7.323,9 s, mong 7.323,6 s) |

Tăng tốc (60 s ch143, 100–160 s, 1 tiến trình): mặc định/`--gpu off --ma-hoa off` 69 s · `--gpu auto` 64 s (≈ cùng, file y hệt từng byte) · `--ma-hoa auto` (VideoToolbox) **107 s, chậm hơn**, RAM 3,9 GB · cả hai auto 69 s.
Xuất đã dùng ~650 % CPU nên 2 chương song song KHÔNG nhanh hơn (mỗi chương ~2,2×, tổng vẫn ~1,1×) mà tốn gấp đôi RAM → mặc định `--song-song 1`.
FULL Nisekoi (~10 giờ, ~30 GB đoạn tạm + file) chưa chạy; ước ~9–10 giờ theo số trên.

Bitrate (trang 9 ch107, 16 s, PSNR so với bản 6 Mbps): 1.500 kbps = 35,3 dB · 2.000 = 37,5 dB · 3.000 = 40,5 dB; chọn 3.000 (≈ app cũ 3,1 Mbps).
`so-khung/`: 8 cặp khung (trái = app cũ FreeCut, phải = recap-studio, cùng thời điểm với ảnh so của mốc 0), `so_khung.py` in MSE từng cặp.

## Chưa làm / khác app cũ
- Slide + Fade từng ô: đã có (mỗi khung của `panel-maps` = 1 clip trang crop tĩnh, chia đều thời gian theo số khung, hiện 0,5 s; Slide trượt từ phải) — nhưng
  khác app cũ: app cũ lấy mốc hiện ô từ `project-path-*.json` (chưa bộ nào có), Slide của app cũ còn crop theo ô 88% khi trượt (ở đây ô trượt tràn ra ngoài ô trên nền mờ), easing tuyến tính. Trang không có khung / > 7 khung: Tĩnh.
- Mảnh vá che chữ (`che-chu/chNNN.json`): định dạng mới chưa có; đọc ảnh `Manga/` đã xoá chữ của định dạng cũ.
- Khung nổi: trang tĩnh thu 88% + bóng đen mờ (alpha 0,85, ảnh 192×108 phóng ×10, mờ σ≈40 px) đúng chỗ trang; quét dọc / theo khung / tĩnh phóng co camera về ô 88% và crop phần tràn (mốc crop mỗi 3 khung khi cỡ đổi). Bóng cố định cả trang, không bám khung động (giống app cũ). Kiểu Trượt nhanh chưa thu 88% (app cũ cũng tách riêng).
- Theo khung: chia thời gian đều theo số khung (chưa đọc đường camera dò sẵn); không easing smoothstep (tuyến tính).
- Nhoè chuyển động là hộp Directional Blur (xấp xỉ), mờ chuyển nền tuyến tính (app: ease-in-out), hạt chống sọc bỏ qua; trang 1 của ch107 app dùng kiểu khác kế hoạch (xem ADR-0005).
- RAM xuất ~4–5 GB mỗi tiến trình (spec ≤ 3 GB cho bước dựng — bước dựng chỉ ~vài trăm MB, RAM nằm ở bước xuất).
- Mốc chương của `xuat` theo tổng mốc câu (cộng mở đầu); chưa sinh `Dang-YouTube/_moc-*.txt` (việc của Màn 6 gói đăng).
