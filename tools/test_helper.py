"""mos_雨滴 v5.0.0 外部辅助程序协议、数值与视觉样张自测。

用法：
  python tools/test_helper.py [effectcraft-helper-mos-rain.exe 路径]
"""

import array
import json
import math
import os
import queue
import struct
import subprocess
import sys
import tempfile
import threading
import time
import zlib

PROTO = 1
EFFECT = "mos.rain"
HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT_DIR = os.path.join(HERE, "tests", "out")


class Helper:
    def __init__(self, exe, env_extra=None):
        env = dict(os.environ)
        if env_extra:
            env.update(env_extra)
        self.proc = subprocess.Popen(
            [exe, "--effectcraft-helper"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            bufsize=0,
            env=env,
        )
        self.q = queue.Queue()
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for raw in self.proc.stdout:
            line = raw.decode("utf-8", "replace").strip()
            if not line:
                continue
            if line.startswith("@ec-helper "):
                line = line[len("@ec-helper "):]
            try:
                self.q.put(json.loads(line))
            except ValueError:
                self.q.put({"op": "@@raw", "line": line})

    def send(self, obj):
        self.proc.stdin.write((json.dumps(obj) + "\n").encode("utf-8"))
        self.proc.stdin.flush()

    def recv(self, timeout=15.0):
        return self.q.get(timeout=timeout)

    def drain(self):
        while True:
            try:
                self.q.get_nowait()
            except queue.Empty:
                return

    def close(self):
        try:
            self.proc.stdin.close()
        except OSError:
            pass
        try:
            self.proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.proc.kill()


def make_params(**overrides):
    defaults = [
        -1.0, 0.0, 0.0, 0.0, 18.0, 16.0, 100.0,   # custom 0..6
        55.0, 55.0, 0.0, 100.0,                    # big 7..10
        65.0, 50.0, 0.0, 100.0,                    # small 11..14
        55.0, 55.0, 0.0, 100.0,                    # streak 15..18
        60.0, 100.0, 22.0, 85.0, 120.0, 18.0,      # flow & optics 19..24
        0.0, 0.0, 0.0, 100.0,                      # output 25..28
    ]
    idx = {
        "custom_layer": 0, "custom_mode": 1, "custom_channel": 2, "custom_invert": 3,
        "custom_threshold": 4, "custom_bevel": 5, "custom_disp": 6,
        "big_density": 7, "big_size": 8, "big_phase": 9, "big_disp": 10,
        "small_density": 11, "small_size": 12, "small_phase": 13, "small_disp": 14,
        "streak_density": 15, "streak_size": 16, "streak_phase": 17, "streak_disp": 18,
        "flow_speed": 19, "displacement": 20, "dispersion": 21,
        "highlight": 22, "light_angle": 23, "bg_blur": 24,
        "seed": 25, "sampling": 26, "edge_mode": 27, "mix": 28,
    }
    p = list(defaults)
    for k, v in overrides.items():
        p[idx[k]] = float(v)
    return p


def write_frame(fh, offset, floats):
    arr = array.array("f", floats)
    if sys.byteorder != "little":
        arr.byteswap()
    fh.seek(offset)
    fh.write(arr.tobytes())
    fh.flush()


def read_frame(fh, offset, n_floats):
    fh.seek(offset)
    raw = fh.read(n_floats * 4)
    arr = array.array("f")
    arr.frombytes(raw)
    if sys.byteorder != "little":
        arr.byteswap()
    return arr


def save_png(path, w, h, floats):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    raw_rows = bytearray()
    stride = w * 4
    for y in range(h):
        raw_rows.append(0)  # filter type 0
        row_start = y * stride
        row = bytearray(stride)
        for x in range(w):
            o = row_start + x * 4
            r = floats[o]
            g = floats[o + 1]
            b = floats[o + 2]
            a = floats[o + 3]
            if a > 1e-4:
                inv = 1.0 / a
                rr = int(max(0.0, min(1.0, r * inv)) * 255.0 + 0.5)
                gg = int(max(0.0, min(1.0, g * inv)) * 255.0 + 0.5)
                bb = int(max(0.0, min(1.0, b * inv)) * 255.0 + 0.5)
            else:
                rr = gg = bb = 0
            aa = int(max(0.0, min(1.0, a)) * 255.0 + 0.5)
            xo = x * 4
            row[xo] = rr
            row[xo + 1] = gg
            row[xo + 2] = bb
            row[xo + 3] = aa
        raw_rows.extend(row)

    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    idat = zlib.compress(bytes(raw_rows), 3)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


def crop_zoom(floats, w, h, x0, y0, cw, ch, zoom=3):
    zw = cw * zoom
    zh = ch * zoom
    out = array.array("f", [0.0] * (zw * zh * 4))
    for sy in range(ch):
        src_y = min(h - 1, max(0, y0 + sy))
        for sx in range(cw):
            src_x = min(w - 1, max(0, x0 + sx))
            so = (src_y * w + src_x) * 4
            r, g, b, a = floats[so], floats[so + 1], floats[so + 2], floats[so + 3]
            for zy in range(zoom):
                dy = sy * zoom + zy
                row_off = (dy * zw + sx * zoom) * 4
                for zx in range(zoom):
                    do = row_off + zx * 4
                    out[do] = r
                    out[do + 1] = g
                    out[do + 2] = b
                    out[do + 3] = a
    return zw, zh, out


def make_test_card(w, h):
    out = array.array("f", [0.0] * (w * h * 4))
    for y in range(h):
        ny = y / max(1, h - 1)
        row = y * w * 4
        for x in range(w):
            nx = x / max(1, w - 1)
            # Crisp colored grid + twilight gradient + vibrant circular bokeh highlights
            r = 0.10 + 0.55 * nx * (1.0 - 0.35 * ny)
            g = 0.14 + 0.52 * (1.0 - ny) + 0.18 * math.sin(nx * 10.0)
            b = 0.24 + 0.65 * ny + 0.18 * math.cos(nx * 7.0)
            if (x % 48) < 2 or (y % 48) < 2:
                r = r * 0.45 + 0.50
                g = g * 0.45 + 0.55
                b = b * 0.45 + 0.60
            # Diagonal stripes in lower-left for crisp refraction visibility
            if ((x + y) // 24) % 2 == 0 and x < w * 0.42 and y > h * 0.45:
                r = min(1.0, r + 0.28)
                g = max(0.0, g - 0.10)
            # Warm city-light bokeh spots
            for bx, by, br, col in [
                (0.25 * w, 0.28 * h, 0.08 * h, (0.98, 0.62, 0.22)),
                (0.68 * w, 0.36 * h, 0.10 * h, (0.28, 0.88, 0.96)),
                (0.52 * w, 0.72 * h, 0.09 * h, (0.95, 0.32, 0.65)),
            ]:
                d2 = (x - bx) ** 2 + (y - by) ** 2
                if d2 < br * br:
                    k = 1.0 - d2 / (br * br)
                    k = k * k
                    r = r + (col[0] - r) * k * 0.85
                    g = g + (col[1] - g) * k * 0.85
                    b = b + (col[2] - b) * k * 0.85
            o = row + x * 4
            out[o] = max(0.0, min(1.0, r))
            out[o + 1] = max(0.0, min(1.0, g))
            out[o + 2] = max(0.0, min(1.0, b))
            out[o + 3] = 1.0
    return out


def make_alpha_map(w, h):
    """Sharp geometric droplets, organic blobs, and thick block bars on transparent alpha."""
    out = array.array("f", [0.0] * (w * h * 4))
    cx1, cy1, r1 = w * 0.28, h * 0.42, h * 0.17
    cx2, cy2, r2 = w * 0.56, h * 0.36, h * 0.11
    cx3, cy3, r3 = w * 0.64, h * 0.62, h * 0.14
    for y in range(h):
        row = y * w * 4
        for x in range(w):
            a = 0.0
            # Circle 1
            d1 = math.hypot(x - cx1, y - cy1) - r1
            a = max(a, max(0.0, min(1.0, 0.5 - d1)))
            # Teardrop / egg 2 + bridged blob 3
            d2 = math.hypot(x - cx2, (y - cy2) * 0.78) - r2
            a = max(a, max(0.0, min(1.0, 0.5 - d2)))
            d3 = math.hypot(x - cx3, y - cy3) - r3
            a = max(a, max(0.0, min(1.0, 0.5 - d3)))
            # Crisp L-shaped letter-like stroke on the right
            if w * 0.77 <= x <= w * 0.81 and h * 0.22 <= y <= h * 0.75:
                a = 1.0
            if w * 0.77 <= x <= w * 0.90 and h * 0.67 <= y <= h * 0.75:
                a = 1.0
            if a > 0.0:
                o = row + x * 4
                out[o] = 0.92 * a
                out[o + 1] = 0.96 * a
                out[o + 2] = 1.0 * a
                out[o + 3] = a
    return out


def make_luma_map(w, h):
    """Shaded 3D lit sphere + smooth glowing stream + dark border on opaque black."""
    out = array.array("f", [0.0] * (w * h * 4))
    sx, sy, sr = w * 0.36, h * 0.48, h * 0.24
    lx, ly, lz = -0.55, -0.60, 0.58
    for y in range(h):
        row = y * w * 4
        for x in range(w):
            v = 0.0
            dx = (x - sx) / sr
            dy = (y - sy) / sr
            r2 = dx * dx + dy * dy
            if r2 < 1.0:
                nz = math.sqrt(1.0 - r2)
                ndotl = max(0.0, dx * lx + dy * ly + nz * lz)
                v = 0.25 + 0.75 * ndotl
                # Soft AA near silhouette
                edge = max(0.0, min(1.0, (1.0 - math.sqrt(r2)) * sr * 0.35))
                v *= edge
            # Second glowing wavy water ribbon on right side
            rx = w * 0.68 + math.sin(y * 0.03) * 28.0
            dist_r = abs(x - rx)
            if y > h * 0.14 and y < h * 0.86:
                g_val = math.exp(-(dist_r * dist_r) / (2.0 * 18.0 * 18.0))
                if g_val > v:
                    v = g_val
            o = row + x * 4
            out[o] = v
            out[o + 1] = v
            out[o + 2] = v
            out[o + 3] = 1.0
    return out


results = []


def check(name, ok, detail=""):
    results.append((name, ok, detail))
    print(("  PASS  " if ok else "  FAIL  ") + name + (("  <- " + detail) if detail else ""))


def diff_stats(a, b, n_floats):
    max_d = 0.0
    changed_px = 0
    for i in range(0, n_floats, 4):
        d = max(abs(a[i] - b[i]), abs(a[i + 1] - b[i + 1]), abs(a[i + 2] - b[i + 2]), abs(a[i + 3] - b[i + 3]))
        if d > max_d:
            max_d = d
        if d > 0.004:
            changed_px += 1
    return max_d, changed_px / (n_floats // 4)


def sanity_check_buffer(buf):
    for i in range(0, len(buf), 4):
        r, g, b, a = buf[i], buf[i + 1], buf[i + 2], buf[i + 3]
        if not (math.isfinite(r) and math.isfinite(g) and math.isfinite(b) and math.isfinite(a)):
            return False, "NaN/Inf detected"
        if r < -1e-5 or g < -1e-5 or b < -1e-5 or a < -1e-5 or a > 1.0001:
            return False, f"Out of range ({r},{g},{b},{a})"
        if r > a + 1e-4 or g > a + 1e-4 or b > a + 1e-4:
            return False, f"Premultiplied bound violated ({r},{g},{b}) > {a}"
    return True, "ok"


def render_once(h, fh, src_floats, w, ht, params, layer_map=None, time_s=0.0, scale=1.0):
    n = w * ht * 4
    layer_offset = n * 4
    write_frame(fh, 0, src_floats)
    if layer_map is not None:
        lw, lh, lfloats = layer_map
        write_frame(fh, layer_offset, lfloats)
    h.drain()
    t0 = time.perf_counter()
    h.send({
        "op": "render",
        "effect": EFFECT,
        "instance": 1,
        "time": time_s,
        "frame": {"offset": 0, "width": w, "height": ht, "origin": [0.0, 0.0], "scale": scale},
        "layer_size": [float(w), float(ht)],
        "flat": params,
    })
    layer_reqs = 0
    while True:
        m = h.recv(timeout=15.0)
        op = m.get("op")
        if op == "layer_input":
            layer_reqs += 1
            if layer_map is not None:
                lw, lh, _ = layer_map
                h.send({
                    "op": "image",
                    "image": {"offset": layer_offset, "width": lw, "height": lh, "origin": [0.0, 0.0], "scale": scale},
                })
            else:
                h.send({"op": "image", "image": None})
        elif op == "done":
            dt_ms = (time.perf_counter() - t0) * 1000.0
            return read_frame(fh, 0, n), dt_ms, layer_reqs
        elif op == "error":
            raise RuntimeError(f"render error: {m}")


def main():
    exe = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        os.environ.get("APPDATA", ""), "EffectCraft", "helpers", "effectcraft-helper-mos-rain.exe"
    )
    if not os.path.isfile(exe):
        print("找不到 helper：" + exe)
        return 2
    print("helper:", exe)

    W, H = 960, 540
    tmpdir = tempfile.mkdtemp(prefix="mos-rain-check-")
    frames_path = os.path.join(tmpdir, "frames.bin")
    with open(frames_path, "wb") as fh:
        fh.truncate(W * H * 16 * 3)

    # 1. Localization check (en-en)
    h_en = Helper(exe, {"MOS_NAMES_LANG": "en", "MOS_PARAMS_LANG": "en"})
    try:
        h_en.send({"op": "hello", "version": PROTO, "frames": frames_path})
        m_hello = h_en.recv()
        h_en.send({"op": "describe"})
        m_desc = h_en.recv()
        eff = (m_desc.get("effects") or [{}])[0]
        man = eff.get("manifest", {})
        check(
            "en-en 本地化 manifest 正确",
            m_hello.get("name") == "mos_rain"
            and man.get("name") == "mos_rain"
            and man.get("category") == "MOS Deform"
            and len(man.get("params", [])) == 29,
            f"name={man.get('name')} cat={man.get('category')} params={len(man.get('params', []))}",
        )
        h_en.send({"op": "shutdown"})
    finally:
        h_en.close()

    # 2. Protocol & functional tests (zh-zh, short timeout for timeout check)
    h = Helper(exe, {
        "MOS_NAMES_LANG": "zh",
        "MOS_PARAMS_LANG": "zh",
        "MOS_RAIN_LAYER_TIMEOUT": "2",
        "MOS_RAIN_LAYER_CACHE_SECS": "5",
    })
    try:
        h.send({"op": "hello", "version": PROTO, "frames": frames_path})
        m = h.recv()
        check("hello 应答正常", m.get("op") == "hello" and m.get("name") == "mos_雨滴", str(m))

        h.send({"op": "describe"})
        m = h.recv()
        effects = m.get("effects") or []
        man = effects[0].get("manifest", {}) if effects else {}
        params_meta = man.get("params", [])
        check(
            "describe 返回 mos.rain / 29 参数 / param[0].type=layer / padding=false",
            bool(effects)
            and man.get("id") == EFFECT
            and effects[0].get("padding") is False
            and len(params_meta) == 29
            and params_meta[0].get("type") == "layer"
            and params_meta[0].get("id") == "custom_layer",
            f"len={len(params_meta)} p0={params_meta[0] if params_meta else None}",
        )

        h.send({"op": "padding", "effect": EFFECT, "instance": 1, "time": 0.0, "scale": 1.0,
                "layer_size": [float(W), float(H)], "flat": make_params()})
        m = h.recv()
        check("padding 请求返回 0", m.get("op") == "padding" and m.get("pixels") == 0, str(m))

        src = make_test_card(W, H)
        alpha_map = make_alpha_map(W, H)
        luma_map = make_luma_map(W, H)
        save_png(os.path.join(OUT_DIR, "00-source.png"), W, H, src)
        save_png(os.path.join(OUT_DIR, "02-custom-alpha-map.png"), W, H, alpha_map)
        save_png(os.path.join(OUT_DIR, "03-custom-luma-map.png"), W, H, luma_map)

        with open(frames_path, "r+b") as fh:
            # Procedural default (no custom layer -> zero layer_input)
            out_proc, ms_proc, reqs_proc = render_once(h, fh, src, W, H, make_params(), time_s=0.0)
            ok_s, msg_s = sanity_check_buffer(out_proc)
            _, chg_proc = diff_stats(src, out_proc, len(src))
            save_png(os.path.join(OUT_DIR, "01-proc-default.png"), W, H, out_proc)
            check(
                "默认纯程序雨滴：不请求 layer_input 且效果正常",
                reqs_proc == 0 and ok_s and chg_proc > 0.15,
                f"{ms_proc:.1f}ms, changed={chg_proc*100:.1f}%, {msg_s}",
            )

            # custom_mode = 2 (仅程序雨滴) with custom_layer = 5 -> zero layer_input
            out_mode2, ms_m2, reqs_m2 = render_once(
                h, fh, src, W, H, make_params(custom_layer=5, custom_mode=2), time_s=0.0
            )
            max_d_m2, _ = diff_stats(out_proc, out_mode2, len(src))
            check(
                "来源模式=仅程序雨滴：即使选了图层也不请求 layer_input 且结果与默认一致",
                reqs_m2 == 0 and max_d_m2 < 1e-6,
                f"reqs={reqs_m2} max_diff={max_d_m2:.2e}",
            )

            # Custom Alpha extraction (仅自定义贴图, bg_blur=0)
            out_alpha, ms_alpha, reqs_alpha = render_once(
                h, fh, src, W, H,
                make_params(custom_layer=10, custom_mode=0, custom_channel=1, bg_blur=0, dispersion=48),
                layer_map=(W, H, alpha_map), time_s=0.0,
            )
            ok_a, msg_a = sanity_check_buffer(out_alpha)
            _, chg_alpha = diff_stats(src, out_alpha, len(src))
            save_png(os.path.join(OUT_DIR, "02-custom-alpha-out.png"), W, H, out_alpha)
            zw, zh, z_buf = crop_zoom(out_alpha, W, H, int(W * 0.20), int(H * 0.25), 220, 180, 3)
            save_png(os.path.join(OUT_DIR, "02-custom-alpha-zoom.png"), zw, zh, z_buf)

            # Verify exterior far from custom shapes (top-left corner 60x60) is strictly untouched!
            ext_diff = 0.0
            for yy in range(0, 60):
                for xx in range(0, 60):
                    o = (yy * W + xx) * 4
                    ext_diff = max(
                        ext_diff,
                        abs(out_alpha[o] - src[o]),
                        abs(out_alpha[o + 1] - src[o + 1]),
                        abs(out_alpha[o + 2] - src[o + 2]),
                    )
            check(
                "自定义 Alpha 提取（仅自定义贴图）：有效区域折射清晰、外部零泄露",
                reqs_alpha == 1 and ok_a and 0.05 < chg_alpha < 0.45 and ext_diff < 1e-5,
                f"{ms_alpha:.1f}ms, changed={chg_alpha*100:.1f}%, ext_diff={ext_diff:.2e}, {msg_a}",
            )

            # Cache hit check at same (param, layer_id=10, time=0.0)
            out_alpha_cached, _, reqs_cached = render_once(
                h, fh, src, W, H,
                make_params(custom_layer=10, custom_mode=0, custom_channel=1, bg_blur=0, dispersion=48),
                layer_map=(W, H, alpha_map), time_s=0.0,
            )
            check("图层缓存命中：同层同时间不重复请求 layer_input", reqs_cached == 0, f"reqs={reqs_cached}")

            # Cache miss check when layer_id changes to 11 at SAME time=0.0 (Luminance map!)
            out_luma, ms_luma, reqs_luma = render_once(
                h, fh, src, W, H,
                make_params(custom_layer=11, custom_mode=0, custom_channel=0, bg_blur=0, dispersion=36),
                layer_map=(W, H, luma_map), time_s=0.0,
            )
            ok_l, msg_l = sanity_check_buffer(out_luma)
            _, chg_luma = diff_stats(src, out_luma, len(src))
            diff_al, _ = diff_stats(out_alpha, out_luma, len(src))
            save_png(os.path.join(OUT_DIR, "03-custom-luma-out.png"), W, H, out_luma)
            zw, zh, z_buf = crop_zoom(out_luma, W, H, int(W * 0.22), int(H * 0.22), 260, 210, 3)
            save_png(os.path.join(OUT_DIR, "03-custom-luma-zoom.png"), zw, zh, z_buf)
            check(
                "切换图层 id 后缓存正确失效，亮度提取（光照球/水带）渲染正常",
                reqs_luma == 1 and ok_l and chg_luma > 0.05 and diff_al > 0.05,
                f"{ms_luma:.1f}ms, changed={chg_luma*100:.1f}%, {msg_l}",
            )

            # Threshold change check (15% vs 60% on luma_map)
            out_t_lo, _, _ = render_once(
                h, fh, src, W, H,
                make_params(custom_layer=11, custom_mode=0, custom_channel=0, custom_threshold=15, bg_blur=0),
                layer_map=(W, H, luma_map), time_s=0.0,
            )
            out_t_hi, _, _ = render_once(
                h, fh, src, W, H,
                make_params(custom_layer=11, custom_mode=0, custom_channel=0, custom_threshold=60, bg_blur=0),
                layer_map=(W, H, luma_map), time_s=0.0,
            )
            _, chg_tlo = diff_stats(src, out_t_lo, len(src))
            _, chg_thi = diff_stats(src, out_t_hi, len(src))
            check(
                "边缘阈值越高提取面积越小（15% vs 60%）",
                chg_tlo > chg_thi + 0.015,
                f"t15={chg_tlo*100:.1f}% vs t60={chg_thi*100:.1f}%",
            )

            # Invert check
            out_inv, _, _ = render_once(
                h, fh, src, W, H,
                make_params(custom_layer=11, custom_mode=0, custom_channel=0, custom_invert=1, bg_blur=0),
                layer_map=(W, H, luma_map), time_s=0.0,
            )
            diff_inv, _ = diff_stats(out_luma, out_inv, len(src))
            check("反转提取明显翻转干湿区", diff_inv > 0.10, f"max_diff={diff_inv:.3f}")

            # Bevel tight vs lush
            out_bt, _, _ = render_once(
                h, fh, src, W, H,
                make_params(custom_layer=10, custom_mode=0, custom_channel=1, custom_bevel=5, bg_blur=0),
                layer_map=(W, H, alpha_map), time_s=0.0,
            )
            out_bl, _, _ = render_once(
                h, fh, src, W, H,
                make_params(custom_layer=10, custom_mode=0, custom_channel=1, custom_bevel=30, bg_blur=0),
                layer_map=(W, H, alpha_map), time_s=0.0,
            )
            save_png(os.path.join(OUT_DIR, "04-custom-bevel-tight.png"), W, H, out_bt)
            save_png(os.path.join(OUT_DIR, "05-custom-bevel-lush.png"), W, H, out_bl)
            _, chg_bevel = diff_stats(out_bt, out_bl, len(src))
            check("边缘曲率 px=5 与 px=30 有清晰差异", chg_bevel > 0.05, f"diff_area={chg_bevel*100:.1f}%")

            # Blend with procedural mode (custom_mode = 1)
            out_blend, ms_blend, _ = render_once(
                h, fh, src, W, H,
                make_params(custom_layer=10, custom_mode=1, custom_channel=1, bg_blur=18),
                layer_map=(W, H, alpha_map), time_s=0.0,
            )
            ok_bl, msg_bl = sanity_check_buffer(out_blend)
            _, diff_ba = diff_stats(out_alpha, out_blend, len(src))
            _, diff_bp = diff_stats(out_proc, out_blend, len(src))
            save_png(os.path.join(OUT_DIR, "06-custom-blend-proc.png"), W, H, out_blend)
            check(
                "融合模式：自定义边缘与程序雨滴共同呈现",
                ok_bl and diff_ba > 0.15 and diff_bp > 0.08,
                f"{ms_blend:.1f}ms, vs_custom={diff_ba*100:.1f}%, vs_proc={diff_bp*100:.1f}%, {msg_bl}",
            )

            # Mix = 0 passthrough
            out_mix0, _, reqs_mix0 = render_once(
                h, fh, src, W, H,
                make_params(custom_layer=10, custom_mode=1, mix=0),
                layer_map=(W, H, alpha_map), time_s=0.0,
            )
            max_d_mix0, _ = diff_stats(src, out_mix0, len(src))
            save_png(os.path.join(OUT_DIR, "07-mix-zero.png"), W, H, out_mix0)
            check("混合=0% 完全直通原图", max_d_mix0 == 0.0 and reqs_mix0 == 0, f"max_diff={max_d_mix0}")

            # Scale preview consistency check (scale=0.5 at 480x270 vs scale=1.0 at 960x540)
            W_half, H_half = W // 2, H // 2
            src_half = make_test_card(W_half, H_half)
            alpha_half = make_alpha_map(W_half, H_half)
            out_half, ms_half, _ = render_once(
                h, fh, src_half, W_half, H_half,
                make_params(custom_layer=10, custom_mode=0, custom_channel=1, bg_blur=0, dispersion=48),
                layer_map=(W_half, H_half, alpha_half), time_s=0.0, scale=0.5,
            )
            ok_h, msg_h = sanity_check_buffer(out_half)
            _, chg_half = diff_stats(src_half, out_half, len(src_half))
            check(
                "预览半分辨率 scale=0.5 与 full-res 一致",
                ok_h and abs(chg_half - chg_alpha) < 0.03,
                f"{ms_half:.1f}ms, half={chg_half*100:.1f}% vs full={chg_alpha*100:.1f}%, {msg_h}",
            )

            # 1080p performance check (best-of-2 to exclude initial OS 100MB page-fault spike)
            W2, H2 = 1920, 1080
            with open(frames_path, "r+b") as fh2:
                fh2.truncate(W2 * H2 * 16 * 3)
            src2 = make_test_card(W2, H2)
            map2 = make_alpha_map(W2, H2)
            with open(frames_path, "r+b") as fh2:
                _, ms_1080_proc, _ = min(
                    render_once(h, fh2, src2, W2, H2, make_params(), time_s=1.0),
                    render_once(h, fh2, src2, W2, H2, make_params(), time_s=1.5),
                    key=lambda x: x[1],
                )
                _, ms_1080_cust, _ = min(
                    render_once(
                        h, fh2, src2, W2, H2,
                        make_params(custom_layer=12, custom_mode=0, custom_channel=1),
                        layer_map=(W2, H2, map2), time_s=1.0,
                    ),
                    render_once(
                        h, fh2, src2, W2, H2,
                        make_params(custom_layer=12, custom_mode=0, custom_channel=1),
                        layer_map=(W2, H2, map2), time_s=2.0,
                    ),
                    key=lambda x: x[1],
                )
            check(
                "1920x1080 原生多线程渲染速度达标",
                ms_1080_proc < 320.0 and ms_1080_cust < 320.0,
                f"1080p proc={ms_1080_proc:.1f}ms, custom={ms_1080_cust:.1f}ms",
            )

        # Timeout + padding-while-waiting + survive test
        with open(frames_path, "r+b") as fh:
            write_frame(fh, 0, src)
        h.drain()
        t0 = time.time()
        h.send({
            "op": "render", "effect": EFFECT, "instance": 9, "time": 9.0,
            "frame": {"offset": 0, "width": W, "height": H, "origin": [0.0, 0.0], "scale": 1.0},
            "layer_size": [float(W), float(H)],
            "flat": make_params(custom_layer=99, custom_mode=0),
        })
        first = h.recv(timeout=5)
        h.send({
            "op": "padding", "effect": EFFECT, "instance": 9, "time": 9.0, "scale": 1.0,
            "layer_size": [float(W), float(H)], "flat": make_params(),
        })
        pad_reply = None
        err_reply = None
        for _ in range(2):
            m = h.recv(timeout=6)
            if m.get("op") == "padding":
                pad_reply = m
            elif m.get("op") == "error":
                err_reply = m
        waited = time.time() - t0
        h.send({"op": "ping"})
        pong = h.recv(timeout=3)
        check(
            "等不到图层时等待期间仍应答 padding、按时超时报 error 且进程存活",
            first.get("op") == "layer_input"
            and pad_reply is not None and pad_reply.get("pixels") == 0
            and err_reply is not None and waited < 5.0
            and pong.get("op") == "pong",
            f"waited={waited:.2f}s err={err_reply}",
        )

        h.send({"op": "shutdown"})
    finally:
        h.close()

    bad = [r for r in results if not r[1]]
    print(f"\n{len(results) - len(bad)}/{len(results)} 通过")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
