#!/usr/bin/env python3
"""So khung video dựng bằng recap-studio với khung của app cũ (FreeCut).
Dùng: so_khung.py <video.mp4> <ra-dir> <ảnh-so.jpg>...
Mỗi ảnh-so là cặp 1930×540 (trái = khung app cũ 960×540, phải = bản thử mốc 0 cùng thời điểm).
Thời điểm = nơi video khớp nhất với nửa PHẢI (cùng đường dựng nên khớp gần như tuyệt đối; thô 0,2 s rồi tinh từng khung);
ghi cặp mới (trái app | phải recap-studio) + MSE so với app (thang 0..255, xám 192×108)."""
import os, subprocess, sys
import numpy as np
from PIL import Image

W, H = 192, 108



def main():
    video, out, *refs = sys.argv[1:]
    os.makedirs(out, exist_ok=True)
    dur = float(subprocess.check_output(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", video]))
    step = 0.2
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", video, "-vf", f"fps=1/{step},scale={W}:{H},format=gray", "-f", "rawvideo", "-"], capture_output=True, check=True).stdout
    fr = np.frombuffer(raw, np.uint8).reshape(-1, H, W).astype(np.float32)
    for r in refs:
        im = Image.open(r).convert("RGB")
        app = im.crop((0, 0, 960, 540))
        a = np.asarray(app.convert("L").resize((W, H), Image.BILINEAR), np.float32)
        old = np.asarray(im.crop((970, 0, 1930, 540)).convert("L").resize((W, H), Image.BILINEAR), np.float32)
        t = int(((fr - old) ** 2).mean(axis=(1, 2)).argmin()) * step
        # tinh: các khung quanh t, bước 1/30 s
        fine = subprocess.run(["ffmpeg", "-v", "error", "-ss", f"{max(0, t - 0.3):.3f}", "-t", "0.6", "-i", video, "-vf", f"fps=30,scale={W}:{H},format=gray", "-f", "rawvideo", "-"], capture_output=True, check=True).stdout
        ff = np.frombuffer(fine, np.uint8).reshape(-1, H, W).astype(np.float32)
        j = int(((ff - old) ** 2).mean(axis=(1, 2)).argmin())
        t = max(0, t - 0.3) + j / 30
        mse = [((ff[j] - a) ** 2).mean()]
        i = 0
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", f"{t:.2f}", "-i", video, "-frames:v", "1", "-vf", "scale=960:540", f"{out}/_f.png"], check=True)
        new = Image.open(f"{out}/_f.png").convert("RGB")
        pair = Image.new("RGB", (1930, 540), "black")
        pair.paste(app, (0, 0)); pair.paste(new, (970, 0))
        name = os.path.basename(r).replace("so-truot-nhanh-", "so-moc1-")
        pair.save(f"{out}/{name}", quality=88)
        print(f"{name}: t={t:.2f}s  MSE moi={mse[i]:.1f}  (mốc 0 vs app={((old - a) ** 2).mean():.1f})")
    os.remove(f"{out}/_f.png")


if __name__ == "__main__":
    main()
