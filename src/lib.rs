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

/// 一滴水：有限斜率的抛物面穹顶，边缘保留斜率，才有硬接触线和锐高光。
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
                let v = (1.0 - q2) * vh;
                *hb.add(row + px) += v;
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

/// 细雾专用：屏幕叠加，不参与等值面。雾滴是表面细节，
/// 混进场里会在水珠边缘切出一圈锯齿。
#[inline]
unsafe fn splat_mist(
    hb: *mut f32,
    sb: *mut f32,
    gw: usize,
    gh: usize,
    cx: f32,
    cy: f32,
    r: f32,
    vh: f32,
) {
    if r < 0.4 || vh <= 0.0 {
        return;
    }
    let x0 = maxf(0.0, floorf(cx - r)) as usize;
    let x1 = minf((gw - 1) as f32, ceilf(cx + r)) as usize;
    let y0 = maxf(0.0, floorf(cy - r)) as usize;
    let y1 = minf((gh - 1) as f32, ceilf(cy + r)) as usize;
    let inv = 1.0 / (r * r);
    let mut py = y0;
    while py <= y1 {
        let dy = py as f32 - cy;
        let row = py * gw;
        let mut px = x0;
        while px <= x1 {
            let dx = px as f32 - cx;
            let q2 = (dx * dx + dy * dy) * inv;
            if q2 < 1.0 {
                let slot = hb.add(row + px);
                let sp = sb.add(row + px);
                if *slot > 0.40 || *sp > 3.0 {
                    px += 1;
                    continue;
                }
                let v = (1.0 - q2) * vh;
                let old = *slot;
                *slot = old + v - old * v;
                if r > *sp {
                    *sp = r;
                }
            }
            px += 1;
        }
        py += 1;
    }
}

#[inline]
fn fract01(v: f32) -> f32 {
    let f = v - floorf(v);
    if f < 0.0 {
        f + 1.0
    } else {
        f
    }
}

#[inline]
fn fabsf(v: f32) -> f32 {
    if v < 0.0 {
        -v
    } else {
        v
    }
}

const MAX_STREAMS: usize = 20;

/// 一条下雨水流的静态描述。头在往下走，轨迹相对头固定，
/// 所以整条水痕会真正向下平移，而不是只在原地摆动。
#[derive(Clone, Copy)]
struct StreamDesc {
    x0: f32,
    phase: f32,
    amp: f32,
    width: f32,
    head: f32,
    len: f32,
    grow: f32,
}

#[inline]
fn stream_dx(d: f32, phase: f32, amp: f32) -> f32 {
    amp * (0.62 * fsin(d * 0.028 + phase) + 0.38 * fsin(d * 0.009 + phase * 1.7))
}

#[inline]
fn stream_w(d: f32, len: f32, width: f32) -> f32 {
    let t = clamp01(d / maxf(len, 1.0));
    // 头颈细、中段略鼓、尾端收细：水痕是细线，头是一颗珠。
    width * (0.35 + 0.20 * (1.0 - t))
}

/// 生成式雨滴场。
///
/// 每颗水珠/每条水流都有出生相位和寿命：小水珠瞬间出现，停一会、偶尔往下
/// 滑一截，最后淡出；水流是一颗带头的大水珠持续下落，身后拖着它走过的轨迹，
/// 轨迹上的节点水珠跟着头一起走。靠近的水在场上相加，再取硬等值面，融成一片。
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
    seed: f32,
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

    let salt = (seed * 1000.0) as u32;
    let cell = maxf(8.0, min_dim * 0.058);
    let rad_base = 0.20 + 0.55 * size01;
    let rad_nom = cell * rad_base * 0.82;
    let raw_thresh = 0.10 + 0.90 * density01;
    let flow_speed = 35.0 + speed * 2.2;

    // ---- 先算好水流：水珠要靠它判断有没有被“带走”。
    let mut descs = [StreamDesc {
        x0: 0.0,
        phase: 0.0,
        amp: 0.0,
        width: 0.0,
        head: 0.0,
        len: 0.0,
        grow: 0.0,
    }; MAX_STREAMS];
    let count = minf(2.0 + streak01 * 12.0, MAX_STREAMS as f32) as usize;
    let mut s = 0;
    while s < count {
        let h0 = hash_u32(0x51ed_2701u32.wrapping_add((s as u32).wrapping_mul(0x9e37_79b9)) ^ salt);
        let h1 = hash_u32(h0 ^ 0xbb67_ae85);
        let h2 = hash_u32(h1.wrapping_add(0x3c6e_f372));
        let h3 = hash_u32(h2 ^ 0x510e_527f);
        let h4 = hash_u32(h3.wrapping_add(0x1f83_d9ab));
        let h5 = hash_u32(h4 ^ 0x68e3_1da4);

        // 周期 = 走完画面 + 头尾余量所需时间；相位错开，雨才连续。
        let cycle = 4.5 + 3.5 * unit(h4);
        let birth = unit(h5) * 3.0;
        let age = time - birth;
        if age < 0.0 {
            s += 1;
            continue;
        }
        let ph = fract01(age / cycle);
        let margin = 40.0 + rad_nom * 4.0;
        let head = -margin + ph * (hf + 2.0 * margin);
        // 出生初期还没走远，尾巴从 0 长出来；走久后尾巴封顶。
        let age = ph * cycle;
        let max_len = hf * (0.30 + 0.50 * trail01);
        let len = minf(max_len, age * flow_speed);

        // 出生时是一颗小水珠，吸饱水才长大、开始拖痕。
        let grow = smoothstep01(clamp01(ph * 9.0));
        descs[s] = StreamDesc {
            x0: unit(h0) * wf,
            phase: unit(h1) * TAU,
            amp: rad_nom * (0.45 + 1.05 * unit(h2)),
            width: rad_nom * (0.45 + 0.35 * unit(h3)),
            head,
            len,
            grow,
        };
        s += 1;
    }

    // ---- 中号水珠：瞬间出现，停一会，偶尔向下滑一截，最后淡出。
    let inv_cell = 1.0 / cell;
    let gwi = ceilf(wf * inv_cell) as i32 + 1;
    let ghi = ceilf(hf * inv_cell) as i32 + 1;
    let mut gy = -1;
    while gy <= ghi {
        let mut gx = -1;
        while gx <= gwi {
            let seed0 = (gx as u32).wrapping_mul(0x9e37_79b9)
                ^ (gy as u32).wrapping_mul(0x85eb_ca6b)
                ^ salt;
            let h0 = hash_u32(seed0);
            let cgx = gx >> 2;
            let cgy = gy >> 2;
            let cm = unit(hash_u32(
                (cgx as u32).wrapping_mul(0x27d4_eb2d)
                    ^ (cgy as u32).wrapping_mul(0x1656_67b1)
                    ^ salt,
            ));
            if unit(h0) > raw_thresh || cm > 0.30 + 0.70 * density01 {
                gx += 1;
                continue;
            }
            let h1 = hash_u32(h0 ^ 0xa5a5_a5a5);
            let h2 = hash_u32(h1.wrapping_add(0x1234_5677));
            let h3 = hash_u32(h2 ^ 0x68e3_1da4);
            let h4 = hash_u32(h3.wrapping_add(0x7f4a_7c15));
            let h5 = hash_u32(h4 ^ 0x2545_f491);
            let h6 = hash_u32(h5.wrapping_add(0x6a09_e667));

            let jx = (unit(h1) - 0.5) * 1.05;
            let jy = (unit(h2) - 0.5) * 1.05;
            let bx = (gx as f32 + 0.5 + jx) * cell;
            let by = (gy as f32 + 0.5 + jy) * cell;
            let r = rad_nom * (0.38 + 0.95 * unit(h3)) * (0.72 + 0.55 * cm);

            // 生命周期：0 处瞬间出现，0.55 开始下滑，0.85 后淡出。
            let life = 2.4 + 3.6 * unit(h4);
            let birth = unit(h5) * 3.0;
            let age = time - birth;
            if age < 0.0 {
                gx += 1;
                continue;
            }
            let ph = fract01(age / life);
            let vis = 1.0 - smoothstep01(clamp01((ph - 0.82) * 5.5));
            if vis <= 0.02 {
                gx += 1;
                continue;
            }
            let slide = smoothstep01(clamp01((ph - 0.55) * 3.3));
            let cy = by + r * (0.35 + 0.95 * unit(h6)) * slide;

            // 水流经过的水珠会被带走：落在水流带里就跳过。
            let mut taken = false;
            let mut si = 0;
            while si < count {
                let d = descs[si].head - cy;
                if d > 0.0 && d < descs[si].len {
                    let xc = descs[si].x0 + stream_dx(d, descs[si].phase, descs[si].amp);
                    let w = stream_w(d, descs[si].len, descs[si].width);
                    if fabsf(bx - xc) < w * 1.7 + 2.0 {
                        taken = true;
                        break;
                    }
                }
                si += 1;
            }
            if taken {
                gx += 1;
                continue;
            }

            let teardrop = slide > 0.05;
            let ra = r * (0.92 + 0.14 * unit(h1));
            let rb = if teardrop {
                r * (1.22 + 0.22 * unit(h2))
            } else {
                r * (0.92 + 0.14 * unit(h2))
            };
            let vh = 0.92 * vis;
            splat(hb, sb, gw, gh, bx, cy, ra, rb, vh, maxf(ra, rb));
            gx += 1;
        }
        gy += 1;
    }

    // ---- 长水流：头不停往下走，身后是它走过的轨迹。
    let mut s = 0;
    while s < count {
        let dsc = &descs[s];
        let y0 = maxf(0.0, floorf(dsc.head - dsc.len)) as usize;
        let y1 = minf((gh - 1) as f32, ceilf(dsc.head)) as usize;
        let mut py = y0;
        while py <= y1 {
            let yf = py as f32;
            let d = dsc.head - yf;
            let t = clamp01(d / maxf(dsc.len, 1.0));
            let xc = dsc.x0 + stream_dx(d, dsc.phase, dsc.amp);
            let w = stream_w(d, dsc.len, dsc.width);
            let tail = smoothstep01(clamp01((1.0 - t) * 6.0));
            let hgt = 1.30 * tail * dsc.grow;
            let ia = 1.0 / (w * w);
            let x0i = maxf(0.0, floorf(xc - w)) as usize;
            let x1i = minf((gw - 1) as f32, ceilf(xc + w)) as usize;
            let row = py * gw;
            let mut px = x0i;
            while px <= x1i {
                let dx = px as f32 - xc;
                let q2 = dx * dx * ia;
                if q2 < 1.0 {
                    *hb.add(row + px) += (1.0 - q2) * hgt;
                    let sp = sb.add(row + px);
                    if w * 1.5 > *sp {
                        *sp = w * 1.5;
                    }
                }
                px += 1;
            }
            py += 1;
        }

        // 头：一颗被重力拽成泪滴的大水珠，位置随时间向下。
        let hx = dsc.x0;
        let hr = dsc.width * 1.70 * dsc.grow;
        splat(hb, sb, gw, gh, hx, dsc.head, hr * 0.88, hr * 1.14, 1.0, hr);

        // 轨迹上的节点水珠：跟着头一起走，所以看起来是整条水在往下流。
        let mut k = 1;
        while k <= 3 {
            let d = k as f32 * (48.0 + dsc.width * 2.0);
            if d < dsc.len {
                let x = dsc.x0 + stream_dx(d, dsc.phase, dsc.amp);
                let w = stream_w(d, dsc.len, dsc.width);
                let r = w * (1.25 + 0.25 * unit(hash_u32(0x1234_5677 ^ (s as u32)))) * dsc.grow;
                let y = dsc.head - d;
                splat(hb, sb, gw, gh, x, y, r * 0.92, r * 1.10, 0.96, r);
            }
            k += 1;
        }
        s += 1;
    }

    // ---- 2–5 颗孤立大水珠
    let big = 2 + (unit(hash_u32(0x9e37_79b9 ^ salt)) * 4.0) as i32;
    let mut k = 0;
    while k < big {
        let hk = hash_u32(0x7f4a_7c15u32.wrapping_add((k as u32).wrapping_mul(0x85eb_ca6b)) ^ salt);
        let hk2 = hash_u32(hk ^ 0x2545_f491);
        if time < unit(hk2) * 3.0 {
            k += 1;
            continue;
        }
        let x = unit(hk) * wf;
        let y = unit(hk2) * hf;
        let r = rad_nom * (1.4 + 1.1 * unit(hash_u32(hk2 ^ 0x68e3_1da4)));
        splat(hb, sb, gw, gh, x, y, r * 0.92, r * 1.18, 1.0, r);
        k += 1;
    }

    // 场直接就是高度：穹顶从中心到接触线一直是弯的，
    // 边缘保留有限斜率，所以是一条干净的硬边，而不是“软坡 + 一刀切”。
    // 重叠区的高度相加，两块水自然鼓成一个鞍形，融成一片。
    let mut i2 = 0;
    while i2 < n {
        let f = *hb.add(i2);
        *hb.add(i2) = minf(f, 1.15);
        i2 += 1;
    }

    // ---- 细雾：最后叠。出生瞬间就有，之后慢慢降低不透明度。
    let cell_m = maxf(3.0, cell * 0.16);
    let inv_cm = 1.0 / cell_m;
    let gwm = ceilf(wf * inv_cm) as i32 + 1;
    let ghm = ceilf(hf * inv_cm) as i32 + 1;
    gy = -1;
    while gy <= ghm {
        let mut gx = -1;
        while gx <= gwm {
            let h0 = hash_u32(
                (gx as u32).wrapping_mul(0x9e37_79b9)
                    ^ (gy as u32).wrapping_mul(0x85eb_ca6b)
                    ^ 0x6a09_e667
                    ^ salt,
            );
            if unit(h0) > mist01 {
                gx += 1;
                continue;
            }
            let h1 = hash_u32(h0 ^ 0xbb67_ae85);
            let h2 = hash_u32(h1.wrapping_add(0x3c6e_f372));
            let h3 = hash_u32(h2 ^ 0x510e_527f);
            let h4 = hash_u32(h3.wrapping_add(0x1f83_d9ab));
            let life = 1.6 + 3.0 * unit(h1);
            let birth = unit(h2) * 2.5;
            let age = time - birth;
            if age < 0.0 {
                gx += 1;
                continue;
            }
            let ph = fract01(age / life);
            let op = 1.0 - ph;
            if op < 0.08 {
                gx += 1;
                continue;
            }
            let cx = (gx as f32 + 0.5 + (unit(h1) - 0.5) * 0.95) * cell_m;
            let cy = (gy as f32 + 0.5 + (unit(h2) - 0.5) * 0.95) * cell_m;
            let r = cell_m * (0.09 + 0.25 * unit(h3));
            let vh = (0.45 + 0.35 * unit(h4)) * op;
            splat_mist(hb, sb, gw, gh, cx, cy, r, vh);
            gx += 1;
        }
        gy += 1;
    }

    (0.30 * rad_nom * rad_nom, 0.60 * rad_nom, rad_nom)
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
    if nparams < 22 {
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
    let seed = clampf(p(20), 0.0, 100.0);
    let delay = clampf(p(21), 0.0, 20.0);
    let t = if time.is_finite() {
        clampf(time as f32 + delay, 0.0, 1.0e6)
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
    let pre1 = arena_alloc((long + 1) * 4) as *mut f32;
    if hbuf.is_null()
        || gbuf.is_null()
        || sbuf.is_null()
        || src.is_null()
        || pre.is_null()
        || pre1.is_null()
    {
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
            seed,
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
                    let lum = clamp01(r * 0.299 + g * 0.587 + b * 0.114);
                    let atten = 1.0 - 0.60 * lum;

                    // 高光：^8 的湿润底光 + ^32 的锐利白边。
                    // 不用坡度距离，避免只在某个方向留一个针尖白点。
                    let p8 = if ndh > 0.0 {
                        let d2 = ndh * ndh;
                        let d4 = d2 * d2;
                        d4 * d4
                    } else {
                        0.0
                    };
                    let p16 = p8 * p8;
                    let p32 = p16 * p16;
                    let spec = (p16 * 1.25 + p8 * 0.05) * highlight * sharp_k;

                    // 暗轮廓：陡法线压暗，水珠才有黑边和体积，而不是一圈白泡泡。
                    let e = clamp01((n2 - 0.90) * 1.00);
                    let e2 = e * e * (3.0 - 2.0 * e);
                    let shade = e2 * (0.18 - 0.08 * dir) * (0.35 + 0.65 * gate);
                    r *= 1.0 - shade;
                    g *= 1.0 - shade;
                    b *= 1.0 - shade;

                    // 只留一点方向性菲涅耳，避免整圈白箍。
                    let g1 = 1.0 - invn;
                    let g2 = g1 * g1;
                    let g4 = g2 * g2;
                    let fres = g4 * g1 * rim * (0.4 + 1.2 * dir) * 0.70 * sharp_k;

                    // 迎光侧的边缘亮线：圆柱形水流没有 y 法线，纯 Blinn 高光
                    // 永远打不亮；这一项专门给陡坡的迎光侧描一条湿边。
                    let el = clamp01((n2 - 1.60) * 0.90);
                    let el2 = el * el * (3.0 - 2.0 * el);
                    let lit = clamp01(dir * 2.2);
                    let edge_light = el2 * lit * rim * 0.65;

                    let light_k = a * atten * gate;
                    r += (spec * 0.98 + fres * 0.20 + edge_light * 0.85) * light_k;
                    g += (spec * 0.99 + fres * 0.28 + edge_light * 0.92) * light_k;
                    b += (spec * 1.00 + fres * 0.35 + edge_light * 1.00) * light_k;
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
