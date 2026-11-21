// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 MOSSLS
//
// 三个液态插件共用的内核：线性内存竞技场、无 std 数学、采样、模糊、梯度。
// 这份文件在三个项目里各存一份，保持独立仓库、互不依赖。

#![allow(dead_code)]

use core::arch::wasm32;

pub const PAGE: usize = 65536;
pub const EPS: f32 = 1.0e-6;
pub const PI: f32 = 3.141_592_7;
pub const HALF_PI: f32 = 1.570_796_3;
pub const FRAC_2_PI: f32 = 0.636_619_8;
pub const TAU: f32 = 6.283_185_5;

// ---------------------------------------------------------------- 线性内存

static mut ARENA_BASE: usize = 0;
static mut ARENA_CUR: usize = 0;

#[inline]
pub fn mem_bytes() -> usize {
    wasm32::memory_size(0) * PAGE
}

pub fn grow_to(end: usize) -> bool {
    let size = mem_bytes();
    if size >= end {
        return true;
    }
    let pages = (end - size + PAGE - 1) / PAGE;
    if wasm32::memory_grow(0, pages) == usize::MAX {
        return false;
    }
    mem_bytes() >= end
}

/// 竞技场起点：首次调用时取「初始线性内存的末端」。
pub fn arena_base() -> usize {
    unsafe {
        if ARENA_BASE == 0 {
            let top = mem_bytes();
            ARENA_BASE = top;
            ARENA_CUR = top;
        }
        ARENA_BASE
    }
}

pub fn arena_alloc(bytes: usize) -> *mut u8 {
    if bytes == 0 {
        return core::ptr::null_mut();
    }
    let start = (unsafe { ARENA_CUR } + 15) & !15usize;
    let end = start + bytes;
    if !grow_to(end) {
        return core::ptr::null_mut();
    }
    unsafe { ARENA_CUR = end };
    start as *mut u8
}

/// 宿主每帧调用一次：`ec_alloc(nparams*8 + w*h*16)`。
pub fn arena_reset(bytes: usize) -> i32 {
    let base = arena_base();
    let want = base + align16(bytes.max(16));
    if !grow_to(want) {
        return 0;
    }
    unsafe { ARENA_CUR = want };
    base as i32
}

// ---------------------------------------------------------------- 小工具

#[inline]
pub fn align16(v: usize) -> usize {
    (v + 15) & !15usize
}

#[inline]
pub fn minf(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}

#[inline]
pub fn maxf(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
}

#[inline]
pub fn clampf(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
pub fn clamp01(v: f32) -> f32 {
    clampf(v, 0.0, 1.0)
}

#[inline]
pub fn smoothstep01(t: f32) -> f32 {
    let t = clamp01(t);
    t * t * (3.0 - 2.0 * t)
}

/// 1/sqrt：位技巧取初值 + 两次牛顿迭代，没有除法。
#[inline]
pub fn inv_sqrt(v: f32) -> f32 {
    if v <= 0.0 {
        return 0.0;
    }
    let h = 0.5 * v;
    let mut y = f32::from_bits(0x5f37_59df - (v.to_bits() >> 1));
    y = y * (1.5 - h * y * y);
    y = y * (1.5 - h * y * y);
    y
}

#[inline]
pub fn sqrtf(v: f32) -> f32 {
    if v <= 0.0 {
        0.0
    } else {
        v * inv_sqrt(v)
    }
}

#[inline]
pub fn floorf(x: f32) -> f32 {
    let t = x as i32 as f32;
    if t > x {
        t - 1.0
    } else {
        t
    }
}

#[inline]
pub fn ceilf(x: f32) -> f32 {
    let t = x as i32 as f32;
    if t < x {
        t + 1.0
    } else {
        t
    }
}

#[inline]
pub fn roundf(x: f32) -> f32 {
    let t = x as i32 as f32;
    let d = x - t;
    if d >= 0.5 {
        t + 1.0
    } else if d <= -0.5 {
        t - 1.0
    } else {
        t
    }
}

// ---------------------------------------------------------------- 三角函数
//
// 把参数归约到 [-π/2, π/2]，再用 11 次泰勒多项式；误差 < 1e-7。

#[inline]
fn poly_sin(r: f32) -> f32 {
    let r2 = r * r;
    r * (1.0
        + r2 * (-1.666_666_7e-1
            + r2 * (8.333_333e-3
                + r2 * (-1.984_127e-4 + r2 * (2.755_731_9e-6 + r2 * (-2.505_210_7e-8))))))
}

#[inline]
fn poly_cos(r: f32) -> f32 {
    let r2 = r * r;
    1.0 + r2
        * (-0.5
            + r2 * (4.166_667e-2
                + r2 * (-1.388_888_9e-3
                    + r2 * (2.480_158_7e-5 + r2 * (-2.755_731_9e-7 + r2 * 2.087_675_8e-9)))))
}

#[inline]
pub fn fsin(x: f32) -> f32 {
    let k = roundf(x * FRAC_2_PI) as i32;
    let r = x - (k as f32) * HALF_PI;
    let s = poly_sin(r);
    let c = poly_cos(r);
    match k & 3 {
        0 => s,
        1 => c,
        2 => -s,
        _ => -c,
    }
}

#[inline]
pub fn fcos(x: f32) -> f32 {
    fsin(x + HALF_PI)
}

// ---------------------------------------------------------------- 哈希

#[inline]
pub fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

#[inline]
pub fn unit(v: u32) -> f32 {
    (v >> 8) as f32 * (1.0 / 16_777_216.0)
}

// ---------------------------------------------------------------- 采样

#[derive(Clone, Copy, Default)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[inline]
pub fn wrap1(i: i32, n: i32, edge: i32) -> (usize, bool) {
    match edge {
        1 => ((i.rem_euclid(n)) as usize, true),
        2 => {
            let p = n * 2;
            let m = i.rem_euclid(p);
            ((if m < n { m } else { p - 1 - m }) as usize, true)
        }
        3 => {
            if i < 0 || i >= n {
                (0, false)
            } else {
                (i as usize, true)
            }
        }
        _ => (i.clamp(0, n - 1) as usize, true),
    }
}

#[inline]
pub fn cubic_weight(t: f32) -> f32 {
    let a = if t < 0.0 { -t } else { t };
    if a <= 1.0 {
        (1.5 * a - 2.5) * a * a + 1.0
    } else if a < 2.0 {
        ((-0.5 * a + 2.5) * a - 4.0) * a + 2.0
    } else {
        0.0
    }
}

#[inline]
pub unsafe fn sample_nearest(src: *const f32, w: i32, h: i32, edge: i32, fx: f32, fy: f32) -> Rgba {
    let xi = roundf(fx) as i32;
    let yi = roundf(fy) as i32;
    let (xx, okx) = wrap1(xi, w, edge);
    let (yy, oky) = wrap1(yi, h, edge);
    if !okx || !oky {
        return Rgba::default();
    }
    let o = (yy * w as usize + xx) * 4;
    Rgba {
        r: *src.add(o),
        g: *src.add(o + 1),
        b: *src.add(o + 2),
        a: *src.add(o + 3),
    }
}

#[inline]
pub unsafe fn sample_bilinear(
    src: *const f32,
    w: i32,
    h: i32,
    edge: i32,
    fx: f32,
    fy: f32,
) -> Rgba {
    // 绝大多数像素都落在画面内部：走直取路径，省掉四次边界判断。
    if fx >= 0.0 && fy >= 0.0 && fx < (w - 1) as f32 && fy < (h - 1) as f32 {
        let x0f = floorf(fx);
        let y0f = floorf(fy);
        let x0 = x0f as usize;
        let y0 = y0f as usize;
        let tx = fx - x0f;
        let ty = fy - y0f;
        let wi = w as usize;
        let o00 = (y0 * wi + x0) * 4;
        let o01 = o00 + 4;
        let o10 = o00 + wi * 4;
        let o11 = o10 + 4;
        let w00 = (1.0 - tx) * (1.0 - ty);
        let w01 = tx * (1.0 - ty);
        let w10 = (1.0 - tx) * ty;
        let w11 = tx * ty;
        return Rgba {
            r: *src.add(o00) * w00
                + *src.add(o01) * w01
                + *src.add(o10) * w10
                + *src.add(o11) * w11,
            g: *src.add(o00 + 1) * w00
                + *src.add(o01 + 1) * w01
                + *src.add(o10 + 1) * w10
                + *src.add(o11 + 1) * w11,
            b: *src.add(o00 + 2) * w00
                + *src.add(o01 + 2) * w01
                + *src.add(o10 + 2) * w10
                + *src.add(o11 + 2) * w11,
            a: *src.add(o00 + 3) * w00
                + *src.add(o01 + 3) * w01
                + *src.add(o10 + 3) * w10
                + *src.add(o11 + 3) * w11,
        };
    }
    let x0f = floorf(fx);
    let y0f = floorf(fy);
    let x0 = x0f as i32;
    let y0 = y0f as i32;
    let tx = fx - x0f;
    let ty = fy - y0f;
    let mut out = Rgba::default();
    let mut j = 0;
    while j < 2 {
        let (yy, oky) = wrap1(y0 + j, h, edge);
        if oky {
            let wy = if j == 0 { 1.0 - ty } else { ty };
            if wy > 0.0 {
                let mut i = 0;
                while i < 2 {
                    let (xx, okx) = wrap1(x0 + i, w, edge);
                    if okx {
                        let wx = if i == 0 { 1.0 - tx } else { tx };
                        let wt = wx * wy;
                        if wt > 0.0 {
                            let o = (yy * w as usize + xx) * 4;
                            out.r += *src.add(o) * wt;
                            out.g += *src.add(o + 1) * wt;
                            out.b += *src.add(o + 2) * wt;
                            out.a += *src.add(o + 3) * wt;
                        }
                    }
                    i += 1;
                }
            }
        }
        j += 1;
    }
    out
}

#[inline]
pub unsafe fn sample_bicubic(src: *const f32, w: i32, h: i32, edge: i32, fx: f32, fy: f32) -> Rgba {
    let x0 = floorf(fx) as i32;
    let y0 = floorf(fy) as i32;
    let mut out = Rgba::default();
    let mut j = -1;
    while j <= 2 {
        let (yy, oky) = wrap1(y0 + j, h, edge);
        if oky {
            let wy = cubic_weight(fy - (y0 + j) as f32);
            if wy != 0.0 {
                let mut i = -1;
                while i <= 2 {
                    let (xx, okx) = wrap1(x0 + i, w, edge);
                    if okx {
                        let wt = wy * cubic_weight(fx - (x0 + i) as f32);
                        if wt != 0.0 {
                            let o = (yy * w as usize + xx) * 4;
                            out.r += *src.add(o) * wt;
                            out.g += *src.add(o + 1) * wt;
                            out.b += *src.add(o + 2) * wt;
                            out.a += *src.add(o + 3) * wt;
                        }
                    }
                    i += 1;
                }
            }
        }
        j += 1;
    }
    out
}

#[inline]
pub unsafe fn sample(
    src: *const f32,
    w: i32,
    h: i32,
    edge: i32,
    method: i32,
    fx: f32,
    fy: f32,
) -> Rgba {
    if method == 2 {
        sample_nearest(src, w, h, edge, fx, fy)
    } else if method == 1 {
        sample_bicubic(src, w, h, edge, fx, fy)
    } else {
        sample_bilinear(src, w, h, edge, fx, fy)
    }
}

// ---------------------------------------------------------------- 模糊

#[inline]
unsafe fn cdf(pre: *const f32, t: f32, len: usize) -> f32 {
    if t <= 0.0 {
        return *pre;
    }
    let lf = len as f32;
    if t >= lf {
        return *pre.add(len);
    }
    let k = t as usize;
    let f = t - (k as f32);
    let a = *pre.add(k);
    let b = *pre.add(k + 1);
    a + (b - a) * f
}

unsafe fn blur_h(pix: *mut f32, w: usize, h: usize, r: f32, pre: *mut f32) {
    let stride = w + 1;
    let wf = w as f32;
    for y in 0..h {
        let row = y * w;
        for c in 0..4 {
            *pre.add(c * stride) = 0.0;
        }
        for x in 0..w {
            let s = (row + x) * 4;
            for c in 0..4 {
                let p = pre.add(c * stride);
                *p.add(x + 1) = *p.add(x) + *pix.add(s + c);
            }
        }
        for x in 0..w {
            let xf = x as f32;
            let lo = maxf(0.0, xf - r);
            let hi = minf(wf, xf + r + 1.0);
            let cnt = hi - lo;
            let inv = if cnt > EPS { 1.0 / cnt } else { 0.0 };
            let s = (row + x) * 4;
            for c in 0..4 {
                let p = pre.add(c * stride);
                let sum = cdf(p, hi, w) - cdf(p, lo, w);
                *pix.add(s + c) = sum * inv;
            }
        }
    }
}

unsafe fn blur_v(pix: *mut f32, w: usize, h: usize, r: f32, pre: *mut f32) {
    let stride = h + 1;
    let hf = h as f32;
    for x in 0..w {
        for c in 0..4 {
            *pre.add(c * stride) = 0.0;
        }
        for y in 0..h {
            let s = (y * w + x) * 4;
            for c in 0..4 {
                let p = pre.add(c * stride);
                *p.add(y + 1) = *p.add(y) + *pix.add(s + c);
            }
        }
        for y in 0..h {
            let yf = y as f32;
            let lo = maxf(0.0, yf - r);
            let hi = minf(hf, yf + r + 1.0);
            let cnt = hi - lo;
            let inv = if cnt > EPS { 1.0 / cnt } else { 0.0 };
            let s = (y * w + x) * 4;
            for c in 0..4 {
                let p = pre.add(c * stride);
                let sum = cdf(p, hi, h) - cdf(p, lo, h);
                *pix.add(s + c) = sum * inv;
            }
        }
    }
}

pub unsafe fn blur_inplace(pix: *mut f32, w: usize, h: usize, radius: f32, pre: *mut f32) {
    if radius <= 0.5 {
        return;
    }
    let r = radius / 1.414_213_6;
    blur_h(pix, w, h, r, pre);
    blur_v(pix, w, h, r, pre);
    blur_h(pix, w, h, r, pre);
    blur_v(pix, w, h, r, pre);
}

/// 单通道模糊：给高度场做羽化用。
unsafe fn blur1_h(buf: *mut f32, w: usize, h: usize, r: f32, pre: *mut f32) {
    for y in 0..h {
        let row = y * w;
        *pre = 0.0;
        for x in 0..w {
            *pre.add(x + 1) = *pre.add(x) + *buf.add(row + x);
        }
        for x in 0..w {
            let xf = x as f32;
            let lo = maxf(0.0, xf - r);
            let hi = minf(w as f32, xf + r + 1.0);
            let cnt = hi - lo;
            let inv = if cnt > EPS { 1.0 / cnt } else { 0.0 };
            *buf.add(row + x) = (cdf(pre, hi, w) - cdf(pre, lo, w)) * inv;
        }
    }
}

unsafe fn blur1_v(buf: *mut f32, w: usize, h: usize, r: f32, pre: *mut f32) {
    for x in 0..w {
        *pre = 0.0;
        for y in 0..h {
            *pre.add(y + 1) = *pre.add(y) + *buf.add(y * w + x);
        }
        for y in 0..h {
            let yf = y as f32;
            let lo = maxf(0.0, yf - r);
            let hi = minf(h as f32, yf + r + 1.0);
            let cnt = hi - lo;
            let inv = if cnt > EPS { 1.0 / cnt } else { 0.0 };
            *buf.add(y * w + x) = (cdf(pre, hi, h) - cdf(pre, lo, h)) * inv;
        }
    }
}

pub unsafe fn blur1_inplace(buf: *mut f32, w: usize, h: usize, radius: f32, pre: *mut f32) {
    if radius <= 0.5 {
        return;
    }
    let r = radius / 1.414_213_6;
    blur1_h(buf, w, h, r, pre);
    blur1_v(buf, w, h, r, pre);
    blur1_h(buf, w, h, r, pre);
    blur1_v(buf, w, h, r, pre);
}

// ---------------------------------------------------------------- 梯度场

/// 高度场原地换成 ∂h/∂x，`gb` 存 ∂h/∂y。
pub unsafe fn make_gradients(hb: *mut f32, gb: *mut f32, w: usize, h: usize) {
    let mut y = 0;
    while y < h {
        let up = if y > 0 { y - 1 } else { y };
        let dn = if y + 1 < h { y + 1 } else { y };
        let r0 = up * w;
        let r1 = dn * w;
        let ro = y * w;
        let mut x = 0;
        while x < w {
            *gb.add(ro + x) = (*hb.add(r1 + x) - *hb.add(r0 + x)) * 0.5;
            x += 1;
        }
        y += 1;
    }
    y = 0;
    while y < h {
        let row = y * w;
        let mut prev = *hb.add(row);
        let mut cur = prev;
        let mut x = 0;
        while x < w {
            let next = *hb.add(row + if x + 1 < w { x + 1 } else { x });
            let g = (next - prev) * 0.5;
            prev = cur;
            cur = next;
            *hb.add(row + x) = g;
            x += 1;
        }
        y += 1;
    }
}

#[inline]
pub unsafe fn grad_at(
    gx: *const f32,
    gy: *const f32,
    w: i32,
    h: i32,
    fx: f32,
    fy: f32,
) -> (f32, f32) {
    let x = clampf(fx, 0.0, (w - 1) as f32);
    let y = clampf(fy, 0.0, (h - 1) as f32);
    let x0f = floorf(x);
    let y0f = floorf(y);
    let x0 = x0f as usize;
    let y0 = y0f as usize;
    let x1 = if x0 + 1 < w as usize { x0 + 1 } else { x0 };
    let y1 = if y0 + 1 < h as usize { y0 + 1 } else { y0 };
    let tx = x - x0f;
    let ty = y - y0f;
    let wi = w as usize;
    let o00 = y0 * wi + x0;
    let o01 = y0 * wi + x1;
    let o10 = y1 * wi + x0;
    let o11 = y1 * wi + x1;
    let w00 = (1.0 - tx) * (1.0 - ty);
    let w01 = tx * (1.0 - ty);
    let w10 = (1.0 - tx) * ty;
    let w11 = tx * ty;
    let dx = *gx.add(o00) * w00 + *gx.add(o01) * w01 + *gx.add(o10) * w10 + *gx.add(o11) * w11;
    let dy = *gy.add(o00) * w00 + *gy.add(o01) * w01 + *gy.add(o10) * w10 + *gy.add(o11) * w11;
    (dx, dy)
}
