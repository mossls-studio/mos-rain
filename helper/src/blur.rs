// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 MOSSLS

//! Multithreaded isotropic separable Gaussian approximation:
//! - Small sigma (`<= 1.25`): exact 5-tap separable Gaussian on X and Y (zero quantization error);
//! - Larger sigma: 3 box passes per axis (`boxes_for_gauss`) with only 2 tiled transposes total,
//!   ensuring identical horizontal and vertical curvature for custom rain contours and fog blur.

use std::thread;

fn boxes_for_gauss(sigma: f32, n: usize) -> Vec<usize> {
    if sigma <= 0.0 {
        return vec![1; n];
    }
    let w_ideal = (12.0 * sigma * sigma / n as f32 + 1.0).sqrt();
    let mut wl = w_ideal.floor() as i32;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wl = wl.max(1);
    let wu = wl + 2;
    let m_ideal = (12.0 * sigma * sigma
        - (n as f32) * (wl * wl) as f32
        - 4.0 * (n as f32) * wl as f32
        - 3.0 * (n as f32))
        / (-4.0 * wl as f32 - 4.0);
    let m = m_ideal.round().max(0.0) as usize;
    (0..n).map(|i| if i < m { wl as usize } else { wu as usize }).collect()
}

#[inline]
fn clamp_i(i: i64, n: usize) -> usize {
    if i < 0 {
        0
    } else if i as usize >= n {
        n - 1
    } else {
        i as usize
    }
}

#[inline]
fn clamp_isize(i: isize, n: usize) -> usize {
    if i < 0 {
        0
    } else if i as usize >= n {
        n - 1
    } else {
        i as usize
    }
}

fn threads_for(h: usize) -> usize {
    let n = thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    n.min(h.max(1)).max(1)
}

fn gauss5_weights(sigma: f32) -> [f32; 3] {
    let inv2s2 = 0.5 / (sigma * sigma).max(1e-4);
    let w0 = 1.0f32;
    let w1 = (-1.0f32 * inv2s2).exp();
    let w2 = (-4.0f32 * inv2s2).exp();
    let inv_sum = 1.0 / (w0 + 2.0 * (w1 + w2));
    [w0 * inv_sum, w1 * inv_sum, w2 * inv_sum]
}

fn row_gauss5(s: &[f32], d: &mut [f32], w: usize, k: [f32; 3]) {
    if w == 0 {
        return;
    }
    let [k0, k1, k2] = k;
    let edge = 2usize.min(w);
    for x in 0..edge {
        let m2 = clamp_isize(x as isize - 2, w);
        let m1 = clamp_isize(x as isize - 1, w);
        let p1 = clamp_isize(x as isize + 1, w);
        let p2 = clamp_isize(x as isize + 2, w);
        d[x] = s[x] * k0 + (s[m1] + s[p1]) * k1 + (s[m2] + s[p2]) * k2;
    }
    if w > 4 {
        for x in 2..(w - 2) {
            d[x] = s[x] * k0 + (s[x - 1] + s[x + 1]) * k1 + (s[x - 2] + s[x + 2]) * k2;
        }
    }
    let tail = edge.max(w.saturating_sub(2));
    for x in tail..w {
        let m2 = clamp_isize(x as isize - 2, w);
        let m1 = clamp_isize(x as isize - 1, w);
        let p1 = clamp_isize(x as isize + 1, w);
        let p2 = clamp_isize(x as isize + 2, w);
        d[x] = s[x] * k0 + (s[m1] + s[p1]) * k1 + (s[m2] + s[p2]) * k2;
    }
}

fn gauss5_rows(src: &[f32], dst: &mut [f32], w: usize, h: usize, k: [f32; 3]) {
    let threads = threads_for(h);
    if threads <= 1 || h < 96 {
        for y in 0..h {
            row_gauss5(&src[y * w..(y + 1) * w], &mut dst[y * w..(y + 1) * w], w, k);
        }
        return;
    }
    let chunk = h.div_ceil(threads);
    thread::scope(|scope| {
        for (ci, out) in dst.chunks_mut(chunk * w).enumerate() {
            let start = ci * chunk;
            scope.spawn(move || {
                for (yi, row) in out.chunks_mut(w).enumerate() {
                    let y = start + yi;
                    row_gauss5(&src[y * w..(y + 1) * w], row, w, k);
                }
            });
        }
    });
}

/// One clamped box blur along a row, radius `r`.
fn row_box(s: &[f32], d: &mut [f32], w: usize, r: usize) {
    if w == 0 {
        return;
    }
    let k = 1.0 / (2 * r + 1) as f32;
    let mut sum = 0.0f32;
    for i in -(r as i64)..=(r as i64) {
        sum += s[clamp_i(i, w)];
    }
    let mut x = 0usize;
    let left_end = r.min(w);
    while x < left_end {
        d[x] = sum * k;
        sum += s[(x + 1 + r).min(w - 1)] - s[0];
        x += 1;
    }
    let interior_end = (w - 1).saturating_sub(r);
    while x < interior_end {
        d[x] = sum * k;
        sum += s[x + 1 + r] - s[x - r];
        x += 1;
    }
    while x < w {
        d[x] = sum * k;
        sum += s[w - 1] - s[(x - r).min(w - 1)];
        x += 1;
    }
}

/// Horizontal box blur of a `w x h` plane, rows spread over threads.
fn box_rows(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    if w == 0 || h == 0 {
        return;
    }
    if r == 0 {
        dst[..w * h].copy_from_slice(&src[..w * h]);
        return;
    }
    let threads = threads_for(h);
    if threads <= 1 || h < 96 {
        for y in 0..h {
            row_box(&src[y * w..(y + 1) * w], &mut dst[y * w..(y + 1) * w], w, r);
        }
        return;
    }
    let chunk = h.div_ceil(threads);
    thread::scope(|scope| {
        for (ci, out) in dst.chunks_mut(chunk * w).enumerate() {
            let start = ci * chunk;
            scope.spawn(move || {
                for (yi, row) in out.chunks_mut(w).enumerate() {
                    let y = start + yi;
                    row_box(&src[y * w..(y + 1) * w], row, w, r);
                }
            });
        }
    });
}

/// `src` is `w x h`, `dst` is `h x w` (`dst[x * h + y] = src[y * w + x]`), tiled for cache.
fn transpose(src: &[f32], dst: &mut [f32], w: usize, h: usize) {
    const BLOCK: usize = 32;
    let mut y0 = 0;
    while y0 < h {
        let y1 = (y0 + BLOCK).min(h);
        let mut x0 = 0;
        while x0 < w {
            let x1 = (x0 + BLOCK).min(w);
            for y in y0..y1 {
                let row = &src[y * w..y * w + w];
                for x in x0..x1 {
                    dst[x * h + y] = row[x];
                }
            }
            x0 = x1;
        }
        y0 = y1;
    }
}

/// Isotropic 2D Gaussian blur of a single plane using 3 box passes per axis.
fn blur_plane_isotropic(src: &[f32], w: usize, h: usize, widths: &[usize]) -> Vec<f32> {
    let n = w * h;
    if n == 0 {
        return Vec::new();
    }
    let mut a = src.to_vec();
    let mut b = vec![0.0f32; n];
    for &width in widths {
        let r = width.saturating_sub(1) / 2;
        if r > 0 {
            box_rows(&a, &mut b, w, h, r);
            std::mem::swap(&mut a, &mut b);
        }
    }
    transpose(&a, &mut b, w, h);
    std::mem::swap(&mut a, &mut b);
    for &width in widths {
        let r = width.saturating_sub(1) / 2;
        if r > 0 {
            box_rows(&a, &mut b, h, w, r);
            std::mem::swap(&mut a, &mut b);
        }
    }
    transpose(&a, &mut b, h, w);
    b
}

/// Blur one channel plane with an isotropic Gaussian of `sigma` pixels.
pub fn gauss_plane(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    if sigma <= 0.05 || w == 0 || h == 0 {
        return src.to_vec();
    }
    if sigma <= 1.25 {
        let k = gauss5_weights(sigma);
        let n = w * h;
        let mut a = vec![0.0f32; n];
        let mut b = vec![0.0f32; n];
        gauss5_rows(src, &mut a, w, h, k);
        transpose(&a, &mut b, w, h);
        gauss5_rows(&b, &mut a, h, w, k);
        transpose(&a, &mut b, h, w);
        return b;
    }
    blur_plane_isotropic(src, w, h, &boxes_for_gauss(sigma, 3))
}

/// Single-plane 2x box downsample -> isotropic blur -> multi-threaded bilinear upsample.
pub fn gauss_plane_downsampled(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    if sigma <= 4.0 || w < 64 || h < 64 {
        return gauss_plane(src, w, h, sigma);
    }
    let dw = (w / 2).max(1);
    let dh = (h / 2).max(1);
    let mut small = vec![0.0f32; dw * dh];
    let threads = threads_for(dh);
    let chunk = dh.div_ceil(threads);
    thread::scope(|scope| {
        for (ci, rows) in small.chunks_mut(chunk * dw).enumerate() {
            let start = ci * chunk;
            scope.spawn(move || {
                for (ri, row) in rows.chunks_mut(dw).enumerate() {
                    let y0 = ((start + ri) * 2).min(h - 1);
                    let y1 = (y0 + 1).min(h - 1);
                    let r0 = &src[y0 * w..(y0 + 1) * w];
                    let r1 = &src[y1 * w..(y1 + 1) * w];
                    for (dx, dst) in row.iter_mut().enumerate() {
                        let x0 = (dx * 2).min(w - 1);
                        let x1 = (x0 + 1).min(w - 1);
                        *dst = (r0[x0] + r0[x1] + r1[x0] + r1[x1]) * 0.25;
                    }
                }
            });
        }
    });

    let blurred = gauss_plane(&small, dw, dh, sigma * 0.5);

    let mut out = vec![0.0f32; w * h];
    let mut xs: Vec<(usize, usize, f32)> = Vec::with_capacity(w);
    let dx_ratio = dw as f32 / w as f32;
    for x in 0..w {
        let fx = ((x as f32 + 0.5) * dx_ratio - 0.5).max(0.0);
        let x0 = (fx.floor() as usize).min(dw - 1);
        xs.push((x0, (x0 + 1).min(dw - 1), fx - x0 as f32));
    }
    let threads_up = threads_for(h);
    let chunk_up = h.div_ceil(threads_up);
    let dy_ratio = dh as f32 / h as f32;
    thread::scope(|scope| {
        for (ci, rows) in out.chunks_mut(chunk_up * w).enumerate() {
            let start = ci * chunk_up;
            let xs_ref = &xs;
            let b_ref = &blurred;
            scope.spawn(move || {
                for (ri, row) in rows.chunks_mut(w).enumerate() {
                    let y = start + ri;
                    let fy = ((y as f32 + 0.5) * dy_ratio - 0.5).max(0.0);
                    let y0 = (fy.floor() as usize).min(dh - 1);
                    let y1 = (y0 + 1).min(dh - 1);
                    let wy = fy - y0 as f32;
                    let r0 = &b_ref[y0 * dw..(y0 + 1) * dw];
                    let r1 = &b_ref[y1 * dw..(y1 + 1) * dw];
                    for (x, dst) in row.iter_mut().enumerate() {
                        let (x0, x1, wx) = xs_ref[x];
                        let top = r0[x0] * (1.0 - wx) + r0[x1] * wx;
                        let bot = r1[x0] * (1.0 - wx) + r1[x1] * wx;
                        *dst = top * (1.0 - wy) + bot * wy;
                    }
                }
            });
        }
    });
    out
}

/// Blur `channels` interleaved planes (4 for RGBA) with an isotropic Gaussian of `sigma`.
pub fn gauss(src: &[f32], w: usize, h: usize, channels: usize, sigma: f32) -> Vec<f32> {
    let n = w * h;
    if sigma <= 0.05 || n == 0 || channels == 0 {
        return src.to_vec();
    }
    let mut out = vec![0.0f32; n * channels];
    let mut plane = vec![0.0f32; n];
    for c in 0..channels {
        for i in 0..n {
            plane[i] = src[i * channels + c];
        }
        let blurred = gauss_plane(&plane, w, h, sigma);
        for i in 0..n {
            out[i * channels + c] = blurred[i];
        }
    }
    out
}

/// Downsample 4 interleaved channels by `k` with a box average. Rows are spread over threads.
fn downsample(src: &[f32], w: usize, h: usize, k: usize) -> (Vec<f32>, usize, usize) {
    let dw = (w / k).max(1);
    let dh = (h / k).max(1);
    let mut out = vec![0.0f32; dw * dh * 4];
    let inv = 1.0 / (k * k) as f32;
    let threads = threads_for(dh);
    let chunk = dh.div_ceil(threads.max(1));
    thread::scope(|scope| {
        for (ci, rows) in out.chunks_mut(chunk * dw * 4).enumerate() {
            let start = ci * chunk;
            scope.spawn(move || {
                for (ri, row) in rows.chunks_mut(dw * 4).enumerate() {
                    let dy = start + ri;
                    let ybase = (dy * k).min(h - 1);
                    for dx in 0..dw {
                        let xbase = (dx * k).min(w - 1);
                        let lim = k.min(w - xbase);
                        let (mut a0, mut a1, mut a2, mut a3) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
                        for sy in 0..k {
                            let y = (ybase + sy).min(h - 1);
                            let mut i = (y * w + xbase) * 4;
                            for _ in 0..lim {
                                a0 += src[i];
                                a1 += src[i + 1];
                                a2 += src[i + 2];
                                a3 += src[i + 3];
                                i += 4;
                            }
                        }
                        let o = dx * 4;
                        row[o] = a0 * inv;
                        row[o + 1] = a1 * inv;
                        row[o + 2] = a2 * inv;
                        row[o + 3] = a3 * inv;
                    }
                }
            });
        }
    });
    (out, dw, dh)
}

/// Bilinear upsample of 4 interleaved channels to `w x h`.
fn upsample(src: &[f32], dw: usize, dh: usize, w: usize, h: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; w * h * 4];
    let mut xs: Vec<(usize, usize, f32)> = Vec::with_capacity(w);
    for x in 0..w {
        let fx = ((x as f32 + 0.5) * (dw as f32 / w as f32) - 0.5).max(0.0);
        let x0 = (fx.floor() as usize).min(dw - 1);
        xs.push((x0, (x0 + 1).min(dw - 1), fx - x0 as f32));
    }
    let threads = threads_for(h);
    let chunk = h.div_ceil(threads.max(1));
    let dy = dh as f32 / h as f32;
    thread::scope(|scope| {
        for (ci, rows) in out.chunks_mut(chunk * w * 4).enumerate() {
            let start = ci * chunk;
            let xs = &xs;
            scope.spawn(move || {
                for (ri, row) in rows.chunks_mut(w * 4).enumerate() {
                    let y = start + ri;
                    let fy = ((y as f32 + 0.5) * dy - 0.5).max(0.0);
                    let y0 = (fy.floor() as usize).min(dh - 1);
                    let y1 = (y0 + 1).min(dh - 1);
                    let wy = fy - y0 as f32;
                    let r0 = y0 * dw * 4;
                    let r1 = y1 * dw * 4;
                    for x in 0..w {
                        let (x0, x1, wx) = xs[x];
                        let p00 = r0 + x0 * 4;
                        let p10 = r0 + x1 * 4;
                        let p01 = r1 + x0 * 4;
                        let p11 = r1 + x1 * 4;
                        let o = x * 4;
                        for c in 0..4 {
                            let top = src[p00 + c] * (1.0 - wx) + src[p10 + c] * wx;
                            let bot = src[p01 + c] * (1.0 - wx) + src[p11 + c] * wx;
                            row[o + c] = top * (1.0 - wy) + bot * wy;
                        }
                    }
                }
            });
        }
    });
    out
}

/// Fast downsampled interleaved Gaussian blur for subtle frosted glass condensation.
pub fn gauss_rgba_downsampled(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    if sigma <= 0.05 || w == 0 || h == 0 {
        return src.to_vec();
    }
    let k = ((sigma / 2.2).ceil() as usize).clamp(2, 6).min(w).min(h);
    if k <= 1 {
        return gauss(src, w, h, 4, sigma);
    }
    let (small, dw, dh) = downsample(src, w, h, k);
    let blurred = gauss(&small, dw, dh, 4, sigma / k as f32);
    upsample(&blurred, dw, dh, w, h)
}
