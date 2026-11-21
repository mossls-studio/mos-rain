// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 MOSSLS
//
// mos_rain —— 雨天玻璃：细密水雾、大小不一的圆水珠，以及会缓慢下滑的长条雨滴。
//
// 高度场是全分辨率的（雨滴只有几个像素，降采样就糊了）：
//   · 细雨层：更密的抖动网格，1–4px 的小水珠，高度低、密度高，像一层水雾；
//   · 主雨滴：抖动网格上的穹顶水珠，密度管数量、大小管半径；
//   · 长条雨滴：一部分雨滴被拉长，上方拖一条由细椭球串成的水痕，整体向下滑。
// 所有雨滴都用 (1-q²)(1-q⁸) 穹顶 + screen 叠加：边缘斜率归零、重叠融成水洼。
// 梯度 → 法线，折射、色散、高光、边缘光、磨砂、染色各自独立。

#![no_std]

mod kit;

use core::panic::PanicInfo;
use kit::*;

static MANIFEST: &[u8] = include_bytes!(env!("MOS_MANIFEST_FILE"));

#[panic_handler]
fn on_panic(_: &PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

#[no_mangle]
pub extern "C" fn ec_api_version() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn ec_manifest_ptr() -> i32 {
    MANIFEST.as_ptr() as i32
}

#[no_mangle]
pub extern "C" fn ec_manifest_len() -> i32 {
    MANIFEST.len() as i32
}

#[no_mangle]
pub extern "C" fn ec_alloc(n: i32) -> i32 {
    arena_reset(n.max(16) as usize)
}

// ---------------------------------------------------------------- 雨滴

/// 在高度场上叠一颗轴对齐椭圆水珠，同时记录这颗水珠的「局部尺度」。
///
/// 穹顶 `(1-q²)(1-q⁸)`：中心留住斜率（放大背景），边缘斜率收到 0，
/// 所以珠子边界不会把背景撕出一条缝。screen 叠加让重叠的珠子融成水洼。
/// 局部尺度用于把折射量按珠子自身半径缩放：2px 的雾滴不会把背景搅出
/// 一圈十几像素的涟漪，15px 的大水珠才有足够厚的“透镜”。
#[inline]
unsafe fn splat(
    hb: *mut f32,
    sb: *mut f32,
    gw: usize,
    gh: usize,
    cx: f32,
    cy: f32,
    ra: f32,
    rb: f32,
    vh: f32,
    sr: f32,
) {
    if ra < 0.45 || rb < 0.45 || vh <= 0.0 {
        return;
    }
    let ia = 1.0 / (ra * ra);
    let ib = 1.0 / (rb * rb);
    let x0f = floorf(cx - ra);
    let x1f = ceilf(cx + ra);
    let y0f = floorf(cy - rb);
    let y1f = ceilf(cy + rb);
    if x1f < 0.0 || y1f < 0.0 || x0f > (gw - 1) as f32 || y0f > (gh - 1) as f32 {
        return;
    }
    let x0 = maxf(0.0, x0f) as usize;
    let x1 = minf((gw - 1) as f32, x1f) as usize;
    let y0 = maxf(0.0, y0f) as usize;
    let y1 = minf((gh - 1) as f32, y1f) as usize;
    let mut py = y0;
    while py <= y1 {
        let dy = py as f32 - cy;
        let dy2 = dy * dy * ib;
        let row = py * gw;
        let mut px = x0;
        while px <= x1 {
            let dx = px as f32 - cx;
            let q2 = dx * dx * ia + dy2;
            if q2 < 1.0 {
                let q4 = q2 * q2;
                let v = (1.0 - q2) * (1.0 - q4 * q4) * vh;
                let slot = hb.add(row + px);
                let old = *slot;
                *slot = old + v - old * v;
                let sp = sb.add(row + px);
                if sr > *sp {
                    *sp = sr;
                }
            }
            px += 1;
        }
        py += 1;
    }
}

#[inline]
fn positive_mod(v: f32, p: f32) -> f32 {
    let m = v - floorf(v / p) * p;
    if m < 0.0 {
        m + p
    } else {
        m
    }
}

/// 拖尾：一条沿重力方向、慢慢变细变矮的连续水痕（不是一串珠子）。
///
/// 每一行取中心线和半宽，横截面用同一个 `(1-q²)(1-q⁸)` 穹顶，
/// 到尾部宽度和高度一起收 0。用解析的脊线而不是叠加椭球，
/// 梯度才会是一条连续的水膜，而不是一圈圈珠子边。
unsafe fn trail(
    hb: *mut f32,
    sb: *mut f32,
    gw: usize,
    gh: usize,
    cx: f32,
    cy: f32,
    r: f32,
    len: f32,
    sway: f32,
    phase: f32,
) {
    if len < 1.0 {
        return;
    }
    let root = cy - r * 0.55;
    let top = root - len;
    let y0 = maxf(0.0, floorf(top)) as usize;
    let y1 = minf((gh - 1) as f32, ceilf(root)) as usize;
    let mut py = y0;
    while py <= y1 {
        let t = clamp01((root - py as f32) / len);
        let taper = 1.0 - t;
        // 用 sqrt 让中段保持宽度/高度，只有接近尾端才真正收细。
        let soft = sqrtf(taper);
        // 浅而宽的湿痕：太窄太厚会变成一条黑线；浅脊只轻轻弯折背景，
        // 边缘再靠高光提亮，才是真实雨痕的读法。
        let lump = 1.0 + 0.28 * fsin(t * 9.0 + phase * 2.1);
        let w = r * (0.08 + 0.38 * soft) * lump;
        let hgt = 0.03 + 0.26 * soft;
        let xc = cx
            + fsin(phase + t * 3.2) * sway * (1.0 - 0.35 * t)
            + fsin(phase * 1.7 + t * 7.5) * sway * 0.45 * (1.0 - t);
        let ia = 1.0 / (w * w);
        let x0 = maxf(0.0, floorf(xc - w)) as usize;
        let x1 = minf((gw - 1) as f32, ceilf(xc + w)) as usize;
        let row = py * gw;
        let mut px = x0;
        while px <= x1 {
            let dx = px as f32 - xc;
            let q2 = dx * dx * ia;
            if q2 < 1.0 {
                let q4 = q2 * q2;
                let v = hgt * (1.0 - q2) * (1.0 - q4 * q4);
                let slot = hb.add(row + px);
                let old = *slot;
                *slot = old + v - old * v;
                let sp = sb.add(row + px);
                if w > *sp {
                    *sp = w;
                }
            }
            px += 1;
        }
        py += 1;
    }
}

/// 生成雨滴高度场，返回 (折射基准尺度, 法线增益, 标称半径)。
unsafe fn build_height(
    hb: *mut f32,
    sb: *mut f32,
    gw: usize,
    gh: usize,
    size01: f32,
    density01: f32,
    streak01: f32,
    trail01: f32,
    mist01: f32,
    speed: f32,
    time: f32,
) -> (f32, f32, f32) {
    let wf = gw as f32;
    let hf = gh as f32;
    let min_dim = minf(wf, hf);
    let n = gw * gh;
    let mut i = 0;
    while i < n {
        *hb.add(i) = 0.0;
        *sb.add(i) = 0.0;
        i += 1;
    }

    let cell = maxf(8.0, min_dim * 0.065);
    let rad_base = 0.18 + 0.50 * size01;
    // 0.82 的收紧系数：默认 55% 落在“中雨”的尺度上，不会铺成一整面气泡膜。
    let rad_nom = cell * rad_base * 0.82;
    let raw_thresh = 0.15 + 0.85 * density01;

    // ---- 细雨层：更密的网格、1–4px 的小水珠，铺在玻璃上像水雾。
    let cell_m = maxf(3.0, cell * 0.22);
    let inv_cm = 1.0 / cell_m;
    let gwm = ceilf(wf * inv_cm) as i32 + 1;
    let ghm = ceilf(hf * inv_cm) as i32 + 1;
    let mut gy = -1;
    while gy <= ghm {
        let mut gx = -1;
        while gx <= gwm {
            let seed = (gx as u32).wrapping_mul(0x9e37_79b9)
                ^ (gy as u32).wrapping_mul(0x85eb_ca6b)
                ^ 0x6a09_e667;
            let h0 = hash_u32(seed);
            if unit(h0) > mist01 {
                gx += 1;
                continue;
            }
            let h1 = hash_u32(h0 ^ 0xbb67_ae85);
            let h2 = hash_u32(h1.wrapping_add(0x3c6e_f372));
            let h3 = hash_u32(h2 ^ 0x510e_527f);
            let cx = (gx as f32 + 0.5 + (unit(h1) - 0.5) * 0.92) * cell_m;
            let cy = (gy as f32 + 0.5 + (unit(h2) - 0.5) * 0.92) * cell_m;
            let r = cell_m * (0.11 + 0.24 * unit(h3));
            let vh = 0.35 + 0.35 * unit(hash_u32(h3 ^ 0x1f83_d9ab));
            splat(hb, sb, gw, gh, cx, cy, r, r * 0.94, vh, r);
            gx += 1;
        }
        gy += 1;
    }

    // ---- 主雨滴：抖动网格，cell 定间距，density 定保留比例，size 定半径。
    let inv_cell = 1.0 / cell;
    let gwi = ceilf(wf * inv_cell) as i32 + 1;
    let ghi = ceilf(hf * inv_cell) as i32 + 1;
    gy = -1;
    while gy <= ghi {
        let mut gx = -1;
        while gx <= gwi {
            let seed =
                (gx as u32).wrapping_mul(0x9e37_79b9) ^ (gy as u32).wrapping_mul(0x85eb_ca6b);
            let h0 = hash_u32(seed);
            // 粗网格留白：真实雨天的玻璃总有几块是干的，不是整面气泡膜。
            let cgx = gx >> 2;
            let cgy = gy >> 2;
            let cm = unit(hash_u32(
                (cgx as u32).wrapping_mul(0x27d4_eb2d) ^ (cgy as u32).wrapping_mul(0x1656_67b1),
            ));
            if unit(h0) > raw_thresh || cm > 0.35 + 0.65 * density01 {
                gx += 1;
                continue;
            }
            let h1 = hash_u32(h0 ^ 0xa5a5_a5a5);
            let h2 = hash_u32(h1.wrapping_add(0x1234_5677));
            let h3 = hash_u32(h2 ^ 0x68e3_1da4);
            let h4 = hash_u32(h3.wrapping_add(0x7f4a_7c15));
            let h5 = hash_u32(h4 ^ 0x2545_f491);
            let h6 = hash_u32(h5.wrapping_add(0x6a09_e667));

            let jx = (unit(h1) - 0.5) * 1.02;
            let jy = (unit(h2) - 0.5) * 1.02;
            let cx = (gx as f32 + 0.5 + jx) * cell;
            let mut cy = (gy as f32 + 0.5 + jy) * cell;
            let r = rad_nom * (0.42 + 1.05 * unit(h3)) * (0.75 + 0.55 * cm);
            let is_streak = unit(h4) < streak01;

            if is_streak {
                // 长条雨滴：缓慢下滑，头部 + 上方拖尾一起下移，按 h+12r 循环。
                let slide = time * speed * 0.5 * (0.5 + unit(h5));
                let period = hf + 12.0 * r;
                // 滑到画面下方后再从上方 6r 处回来；time=0 时位置不动。
                cy += slide;
                if cy > hf + 6.0 * r {
                    cy = positive_mod(cy + 6.0 * r, period) - 6.0 * r;
                }

                let phase = unit(h6) * TAU;
                let len = r * (1.0 + trail01 * 18.0);
                let sway = r * (0.25 + 0.25 * unit(hash_u32(h6 ^ 0x1429_29a5)));
                // 头部：竖直方向略拉长，像被重力拽住的水珠。
                let ra = r * (0.85 + 0.20 * unit(h1));
                let rb = r * (1.15 + 0.25 * unit(h2));

                // 拖尾在上方，越往上越细、越矮，横向带一点 sin 摆动。
                trail(hb, sb, gw, gh, cx, cy, r, len, sway, phase);

                // 头最后画，压住拖尾根部。
                let hv = 0.92 + 0.08 * unit(h3);
                let hr = maxf(ra, rb);
                splat(hb, sb, gw, gh, cx, cy, ra, rb, hv, hr);
            } else {
                // 圆水珠：轻微椭圆，横向略宽，像贴在玻璃上的凝露。
                cy += 0.6 * fsin(time * 0.35 + unit(h5) * TAU);
                let ra = r * (0.96 + 0.20 * unit(h1));
                let rb = r * (0.87 + 0.16 * unit(h2));
                let vh = 0.86 + 0.14 * unit(h6);
                splat(hb, sb, gw, gh, cx, cy, ra, rb, vh, maxf(ra, rb));
            }
            gx += 1;
        }
        gy += 1;
    }

    // base_disp：中心约 1.6× 放大；normal_gain 让法线落在 −1..1 附近。
    (0.30 * rad_nom * rad_nom, 0.6 * rad_nom, rad_nom)
}

// ---------------------------------------------------------------- 渲染

#[no_mangle]
pub extern "C" fn ec_render(
    pixels: i32,
    width: i32,
    height: i32,
    params: i32,
    nparams: i32,
    time: f64,
    _scale: f64,
) -> i32 {
    if width <= 0 || height <= 0 || pixels == 0 || params == 0 {
        return 0;
    }
    if nparams < 20 {
        return 1;
    }
    let w = width as usize;
    let h = height as usize;
    let n = w * h;
    if n == 0 || n > 16_000_000 {
        return 1;
    }
    let wf = w as f32;
    let hf = h as f32;
    let min_dim = minf(wf, hf);

    let px = pixels as *mut f32;
    let prm = params as *const f64;
    let p = |i: usize| -> f32 {
        let v = unsafe { *prm.add(i) };
        if v.is_finite() {
            v as f32
        } else {
            0.0
        }
    };

    let amount = clampf(p(0), 0.0, 300.0);
    let size = clampf(p(1), 0.0, 100.0);
    let density = clampf(p(2), 0.0, 100.0);
    let streak = clampf(p(3), 0.0, 100.0);
    let trail = clampf(p(4), 0.0, 100.0);
    let mist = clampf(p(5), 0.0, 100.0);
    let speed = clampf(p(6), 0.0, 200.0);
    let dispersion = clamp01(p(7) * 0.01);
    let highlight = clampf(p(8), 0.0, 300.0) * 0.01;
    let rim = clampf(p(9), 0.0, 300.0) * 0.01;
    let light_angle = p(10);
    let frost = clamp01(p(11) * 0.01);
    let tint_r = clamp01(p(12));
    let tint_g = clamp01(p(13));
    let tint_b = clamp01(p(14));
    let tint_amount = clamp01(p(16) * 0.01);
    let sampling = p(17) as i32;
    let edge = p(18) as i32;
    let mix = clamp01(p(19) * 0.01);
    let t = if time.is_finite() {
        clampf(time as f32, 0.0, 1.0e6)
    } else {
        0.0
    };

    if mix <= 0.0
        || (amount <= 0.0 && frost <= 0.0 && tint_amount <= 0.0 && highlight <= 0.0 && rim <= 0.0)
    {
        return 0;
    }

    let wi = w as i32;
    let hi = h as i32;
    let n4 = n * 4;
    // 雨滴只有几个像素宽，高度场必须全分辨率。
    let inv_g = 1.0;
    let gw = w;
    let gh = h;
    let gn = n;

    let hbuf = arena_alloc(gn * 4) as *mut f32;
    let gbuf = arena_alloc(gn * 4) as *mut f32;
    let sbuf = arena_alloc(gn * 4) as *mut f32;
    let src = arena_alloc(n * 16) as *mut f32;
    let long = if w > h { w } else { h };
    let pre = arena_alloc((long + 1) * 16) as *mut f32;
    if hbuf.is_null() || gbuf.is_null() || sbuf.is_null() || src.is_null() || pre.is_null() {
        return 1;
    }
    let mut blur_buf: *mut f32 = core::ptr::null_mut();
    let mut bw = 0usize;
    let mut bh = 0usize;
    if frost > 0.001 {
        bw = maxf(2.0, floorf(wf * 0.5)) as usize;
        bh = maxf(2.0, floorf(hf * 0.5)) as usize;
        blur_buf = arena_alloc(bw * bh * 16) as *mut f32;
        if blur_buf.is_null() {
            return 1;
        }
    }

    unsafe {
        let mut i = 0;
        while i < n4 {
            *src.add(i) = *px.add(i);
            i += 1;
        }

        let (base_disp, normal_gain, rad_nom) = build_height(
            hbuf,
            sbuf,
            gw,
            gh,
            clamp01(size * 0.01),
            clamp01(density * 0.01),
            clamp01(streak * 0.01),
            clamp01(trail * 0.01),
            clamp01(mist * 0.01),
            speed,
            t,
        );
        make_gradients(hbuf, gbuf, gw, gh);

        if !blur_buf.is_null() {
            let mut by = 0;
            while by < bh {
                let sy0 = by * 2;
                let sy1 = minf((sy0 + 1) as f32, hf - 1.0) as usize;
                let mut bx = 0;
                while bx < bw {
                    let sx0 = bx * 2;
                    let sx1 = minf((sx0 + 1) as f32, wf - 1.0) as usize;
                    let o00 = (sy0 * w + sx0) * 4;
                    let o01 = (sy0 * w + sx1) * 4;
                    let o10 = (sy1 * w + sx0) * 4;
                    let o11 = (sy1 * w + sx1) * 4;
                    let d = (by * bw + bx) * 4;
                    let mut c = 0;
                    while c < 4 {
                        *blur_buf.add(d + c) = (*src.add(o00 + c)
                            + *src.add(o01 + c)
                            + *src.add(o10 + c)
                            + *src.add(o11 + c))
                            * 0.25;
                        c += 1;
                    }
                    bx += 1;
                }
                by += 1;
            }
            let radius = frost * 0.035 * min_dim * 0.5;
            blur_inplace(blur_buf, bw, bh, radius, pre);
        }

        let amt = amount * 0.01;
        let sharp_k = 1.0 - frost * 0.75;
        let soft_k = 1.0 - frost * 0.35;
        // 折射和法线都按每颗雨滴自己的尺度缩放：小球只搅动自己附近，
        // 大水珠才有厚透镜的位移量。s = 局部雨滴半径（像素）。
        let inv_rn = 1.0 / rad_nom;
        let disp_w = inv_g * base_disp * amt * soft_k * inv_rn * inv_rn;
        let normal_k = normal_gain * inv_rn;
        let ca = dispersion * 0.10;
        let la = light_angle * PI / 180.0;
        let lx = fcos(la);
        let ly = -fsin(la);
        let lz = 0.75;
        let inv_hl = inv_sqrt(lx * lx + ly * ly + (lz + 1.0) * (lz + 1.0));
        let hx = lx * inv_hl;
        let hy = ly * inv_hl;
        let hz = (lz + 1.0) * inv_hl;
        let tint_k = tint_amount;
        let tgx = hx / hz;
        let tgy = hy / hz;
        let two_phase = ca > 0.005;
        // 色散两路只差几个像素，双线性足够；双三次留给主采样。
        let chroma_method = if sampling == 1 { 0 } else { sampling };
        let gwi = gw as i32;
        let ghi = gh as i32;
        let gx_base = hbuf as *const f32;
        let gy_base = gbuf as *const f32;

        let mut y = 0;
        while y < h {
            let row = y * w;
            let gy_pos = y as f32 * inv_g;
            let mut x = 0;
            while x < w {
                let i = row + x;
                let (gx, gy) = grad_at(gx_base, gy_base, gwi, ghi, x as f32 * inv_g, gy_pos);
                let s = *sbuf.add(i);
                let gate = clamp01(s * 0.6);
                let dmax = s * 2.2;
                let disp_k = disp_w * s * s;
                let o = i * 4;
                let orr = *px.add(o);
                let org = *px.add(o + 1);
                let orb = *px.add(o + 2);
                let ora = *px.add(o + 3);

                let mut dx = gx * disp_k;
                let mut dy = gy * disp_k;
                if dx > dmax {
                    dx = dmax;
                } else if dx < -dmax {
                    dx = -dmax;
                }
                if dy > dmax {
                    dy = dmax;
                } else if dy < -dmax {
                    dy = -dmax;
                }

                let fxg = x as f32 + dx;
                let fyg = y as f32 + dy;

                let (mut r, mut g, mut b, mut a);
                if two_phase {
                    let sr = sample(
                        src,
                        wi,
                        hi,
                        edge,
                        chroma_method,
                        x as f32 + dx * (1.0 - ca),
                        y as f32 + dy * (1.0 - ca),
                    );
                    let sb = sample(
                        src,
                        wi,
                        hi,
                        edge,
                        chroma_method,
                        x as f32 + dx * (1.0 + ca),
                        y as f32 + dy * (1.0 + ca),
                    );
                    r = sr.r;
                    g = (sr.g + sb.g) * 0.5;
                    b = sb.b;
                    a = (sr.a + sb.a) * 0.5;
                } else {
                    let s = sample(src, wi, hi, edge, sampling, fxg, fyg);
                    r = s.r;
                    g = s.g;
                    b = s.b;
                    a = s.a;
                }

                if !blur_buf.is_null() {
                    let fk = frost * gate;
                    let soft = sample(
                        blur_buf,
                        bw as i32,
                        bh as i32,
                        edge,
                        sampling,
                        fxg * 0.5,
                        fyg * 0.5,
                    );
                    r += (soft.r - r) * fk;
                    g += (soft.g - g) * fk;
                    b += (soft.b - b) * fk;
                    a += (soft.a - a) * fk;
                }

                let tk = tint_k * gate;
                if tk > 0.0 {
                    r *= 1.0 - tk + tint_r * tk;
                    g *= 1.0 - tk + tint_g * tk;
                    b *= 1.0 - tk + tint_b * tk;
                }

                if highlight > 0.0 || rim > 0.0 {
                    let ng = normal_k * s;
                    let nx = -gx * ng * soft_k;
                    let ny = -gy * ng * soft_k;
                    let n2 = nx * nx + ny * ny;
                    let invn = inv_sqrt(n2 + 1.0);
                    let ndh = (nx * hx + ny * hy + hz) * invn;
                    let dir = clamp01(ndh);
                    // 宽而淡的湿润光泽。
                    let sheen = if ndh > 0.0 {
                        let nd2 = ndh * ndh;
                        let nd4 = nd2 * nd2;
                        nd4 * nd4
                    } else {
                        0.0
                    };
                    // 锐利的镜面反光：只在法线接近理想反射方向的区域出现，
                    // 是一块湿亮的高光，而不是给整颗珠子蒙一层灰。
                    let ddx = nx - tgx;
                    let ddy = ny - tgy;
                    let dd = sqrtf(ddx * ddx + ddy * ddy);
                    let tt = clampf(1.0 - dd * 8.0, 0.0, 1.0);
                    let glint = tt * tt * (3.0 - 2.0 * tt);
                    // 亮背景上的水珠只该提亮，不该整颗烧成纯白。
                    let lum = clamp01(r * 0.299 + g * 0.587 + b * 0.114);
                    let atten = 1.0 - 0.68 * lum;
                    // 很小的雾滴散射更强：给它们一点底光，暗背景上才不会消失。
                    let small = clamp01(1.0 - s * inv_rn * 1.8);
                    let scatter = small * 0.10;
                    let spec = (0.04 + scatter + sheen * 0.06 + glint * 0.95) * highlight * sharp_k;
                    // Schlick 菲涅耳：只有接近掠射的陡坡才反光。
                    let g1 = 1.0 - invn;
                    let g2 = g1 * g1;
                    let g4 = g2 * g2;
                    let fres = g4 * g1 * rim * (0.6 + 1.4 * dir) * 0.82 * sharp_k;
                    // 珠子边缘的一圈亮边，阈值 n²>0.85；只认迎光侧。
                    let e = clamp01((n2 - 0.85) * 1.25);
                    let e2 = e * e * (3.0 - 2.0 * e);
                    let lit = if dir > 0.02 { clamp01(dir * 2.2) } else { 0.0 };
                    // 背光侧的边缘压暗一点，水滴才有体积，不会是一整圈白泡泡。
                    // 迎光侧也留一点暗轮廓：亮背景上水滴靠这条暗边才看得出来。
                    let shade = e2 * (0.30 - 0.16 * lit);
                    r *= 1.0 - shade;
                    g *= 1.0 - shade;
                    b *= 1.0 - shade;
                    let ring = e2 * rim * lit * 0.38 * sharp_k;
                    let light_k = a * atten * gate;
                    r += (spec * 0.98 + fres * 0.72 + ring * 0.92) * light_k;
                    g += (spec * 0.99 + fres * 0.86 + ring * 0.96) * light_k;
                    b += (spec * 1.00 + fres * 1.00 + ring * 1.00) * light_k;
                }

                if mix < 1.0 {
                    r = orr + (r - orr) * mix;
                    g = org + (g - org) * mix;
                    b = orb + (b - orb) * mix;
                    a = ora + (a - ora) * mix;
                }

                if a >= 0.0 {
                    r = clampf(r, 0.0, a);
                    g = clampf(g, 0.0, a);
                    b = clampf(b, 0.0, a);
                } else {
                    r = maxf(0.0, r);
                    g = maxf(0.0, g);
                    b = maxf(0.0, b);
                    a = 0.0;
                }

                *px.add(o) = r;
                *px.add(o + 1) = g;
                *px.add(o + 2) = b;
                *px.add(o + 3) = a;

                x += 1;
            }
            y += 1;
        }
    }

    0
}
