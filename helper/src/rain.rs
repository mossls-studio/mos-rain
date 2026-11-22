// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 MOSSLS

//! mos_rain v5.0.0:
//! - Procedural rain on window glass (large drops, small condensation beads, stick-slip meandering streaks);
//! - Optional custom map layer extracted by Luminance or Alpha to derive liquid dome edges, lighting
//!   contours, convex refraction displacement, 3-channel dispersion and meniscus shading.

use crate::blur;
use std::f32::consts::TAU;
use std::thread;

pub const I_CUSTOM_LAYER: usize = 0;
pub const I_CUSTOM_MODE: usize = 1;
pub const I_CUSTOM_CHANNEL: usize = 2;
pub const I_CUSTOM_INVERT: usize = 3;
pub const I_CUSTOM_THRESHOLD: usize = 4;
pub const I_CUSTOM_BEVEL: usize = 5;
pub const I_CUSTOM_DISP: usize = 6;

pub const I_BIG_DENSITY: usize = 7;
pub const I_BIG_SIZE: usize = 8;
pub const I_BIG_PHASE: usize = 9;
pub const I_BIG_DISP: usize = 10;

pub const I_SMALL_DENSITY: usize = 11;
pub const I_SMALL_SIZE: usize = 12;
pub const I_SMALL_PHASE: usize = 13;
pub const I_SMALL_DISP: usize = 14;

pub const I_STREAK_DENSITY: usize = 15;
pub const I_STREAK_SIZE: usize = 16;
pub const I_STREAK_PHASE: usize = 17;
pub const I_STREAK_DISP: usize = 18;

pub const I_FLOW_SPEED: usize = 19;
pub const I_DISPLACEMENT: usize = 20;
pub const I_DISPERSION: usize = 21;
pub const I_HIGHLIGHT: usize = 22;
pub const I_LIGHT_ANGLE: usize = 23;
pub const I_BG_BLUR: usize = 24;

pub const I_SEED: usize = 25;
pub const I_SAMPLING: usize = 26;
pub const I_EDGE_MODE: usize = 27;
pub const I_MIX: usize = 28;

const CONTACT_T: f32 = 0.30;
const INV_QC: f32 = 1.487_048_8;
const MAX_STREAKS: usize = 28;

pub trait Layers {
    fn layer_image(
        &mut self,
        param: &str,
        layer_id: i64,
        time: f64,
        target_w: usize,
        target_h: usize,
    ) -> Option<(usize, usize, Vec<f32>)>;
}

pub fn manifest() -> serde_json::Value {
    serde_json::json!({
        "api": 2,
        "id": "mos.rain",
        "name": "mos_雨滴",
        "category": "MOS 形变",
        "version": "5.0.0",
        "author": "MOSSLS",
        "description": "真实雨天玻璃窗：支持自定义图层按亮度或 Alpha 提取轮廓，把自定义光照与边缘套用液态折射、三通道色散与弯月面高光；也内置大雨滴、小雨滴与粘滑流动长条雨滴，可单独或融合使用。",
        "params": [
            {"id": "custom_layer", "name": "自定义图层", "type": "layer", "group": "自定义贴图"},
            {"id": "custom_mode", "name": "来源模式", "type": "popup", "options": ["仅自定义贴图", "与程序雨滴融合", "仅程序雨滴"], "default": 0, "group": "自定义贴图"},
            {"id": "custom_channel", "name": "提取方式", "type": "popup", "options": ["亮度", "Alpha"], "default": 0, "group": "自定义贴图"},
            {"id": "custom_invert", "name": "反转提取", "type": "checkbox", "default": false, "group": "自定义贴图"},
            {"id": "custom_threshold", "name": "边缘阈值 %", "type": "slider", "default": 18, "min": 0, "max": 95, "decimals": 1, "group": "自定义贴图"},
            {"id": "custom_bevel", "name": "边缘曲率 px", "type": "slider", "default": 16, "min": 1, "max": 80, "sliderMax": 50, "decimals": 1, "group": "自定义贴图"},
            {"id": "custom_disp", "name": "自定义置换强度 %", "type": "slider", "default": 100, "min": 0, "max": 300, "decimals": 1, "group": "自定义贴图"},

            {"id": "big_density", "name": "大雨滴密度 %", "type": "slider", "default": 55, "min": 0, "max": 100, "decimals": 1, "group": "大雨滴"},
            {"id": "big_size", "name": "大雨滴大小 %", "type": "slider", "default": 55, "min": 10, "max": 200, "decimals": 1, "group": "大雨滴"},
            {"id": "big_phase", "name": "大雨滴相位", "type": "slider", "default": 0, "min": 0, "max": 100, "decimals": 1, "group": "大雨滴"},
            {"id": "big_disp", "name": "大雨滴置换强度 %", "type": "slider", "default": 100, "min": 0, "max": 300, "decimals": 1, "group": "大雨滴"},

            {"id": "small_density", "name": "小雨滴密度 %", "type": "slider", "default": 65, "min": 0, "max": 100, "decimals": 1, "group": "小雨滴"},
            {"id": "small_size", "name": "小雨滴大小 %", "type": "slider", "default": 50, "min": 10, "max": 200, "decimals": 1, "group": "小雨滴"},
            {"id": "small_phase", "name": "小雨滴相位", "type": "slider", "default": 0, "min": 0, "max": 100, "decimals": 1, "group": "小雨滴"},
            {"id": "small_disp", "name": "小雨滴置换强度 %", "type": "slider", "default": 100, "min": 0, "max": 300, "decimals": 1, "group": "小雨滴"},

            {"id": "streak_density", "name": "长条密度 %", "type": "slider", "default": 55, "min": 0, "max": 100, "decimals": 1, "group": "长条雨滴"},
            {"id": "streak_size", "name": "长条大小 %", "type": "slider", "default": 55, "min": 10, "max": 200, "decimals": 1, "group": "长条雨滴"},
            {"id": "streak_phase", "name": "长条相位", "type": "slider", "default": 0, "min": 0, "max": 100, "decimals": 1, "group": "长条雨滴"},
            {"id": "streak_disp", "name": "长条置换强度 %", "type": "slider", "default": 100, "min": 0, "max": 300, "decimals": 1, "group": "长条雨滴"},

            {"id": "flow_speed", "name": "下落流速 %", "type": "slider", "default": 60, "min": 0, "max": 300, "decimals": 1, "group": "流动与光学"},
            {"id": "displacement", "name": "总置换强度 %", "type": "slider", "default": 100, "min": 0, "max": 300, "decimals": 1, "group": "流动与光学"},
            {"id": "dispersion", "name": "色散强度 %", "type": "slider", "default": 22, "min": 0, "max": 100, "decimals": 1, "group": "流动与光学"},
            {"id": "highlight", "name": "高光强度 %", "type": "slider", "default": 85, "min": 0, "max": 300, "decimals": 1, "group": "流动与光学"},
            {"id": "light_angle", "name": "光照角度", "type": "angle", "default": 120, "group": "流动与光学"},
            {"id": "bg_blur", "name": "玻璃雾气 %", "type": "slider", "default": 18, "min": 0, "max": 100, "decimals": 1, "group": "流动与光学"},

            {"id": "seed", "name": "随机种子", "type": "slider", "default": 0, "min": 0, "max": 100, "decimals": 0, "group": "输出"},
            {"id": "sampling", "name": "采样", "type": "popup", "options": ["双线性", "双三次", "最近邻"], "default": 0, "group": "输出"},
            {"id": "edge_mode", "name": "边缘", "type": "popup", "options": ["拉伸", "重复", "镜像", "透明"], "default": 0, "group": "输出"},
            {"id": "mix", "name": "混合 %", "type": "slider", "default": 100, "min": 0, "max": 100, "decimals": 0, "group": "输出"}
        ]
    })
}

struct Params {
    custom_layer: i64,
    custom_mode: i32,
    custom_channel: i32,
    custom_invert: bool,
    custom_threshold: f32,
    custom_bevel: f32,
    custom_disp: f32,

    big_density: f32,
    big_size: f32,
    big_phase: f32,
    big_disp: f32,

    small_density: f32,
    small_size: f32,
    small_phase: f32,
    small_disp: f32,

    streak_density: f32,
    streak_size: f32,
    streak_phase: f32,
    streak_disp: f32,

    flow_speed: f32,
    master_disp: f32,
    dispersion: f32,
    hi_k: f32,
    light_angle: f32,
    bg_blur: f32,

    seed: f32,
    sampling: i32,
    edge_mode: i32,
    mix: f32,
}

impl Params {
    fn parse(flat: &[f64]) -> Self {
        let f = |i: usize, d: f64| -> f32 {
            let v = flat.get(i).copied().unwrap_or(d);
            if v.is_finite() { v as f32 } else { d as f32 }
        };
        let layer_v = flat.get(I_CUSTOM_LAYER).copied().unwrap_or(-1.0);
        let custom_layer = if layer_v.is_finite() { layer_v as i64 } else { -1 };
        Self {
            custom_layer,
            custom_mode: (f(I_CUSTOM_MODE, 0.0).round() as i32).clamp(0, 2),
            custom_channel: (f(I_CUSTOM_CHANNEL, 0.0).round() as i32).clamp(0, 1),
            custom_invert: f(I_CUSTOM_INVERT, 0.0) != 0.0,
            custom_threshold: f(I_CUSTOM_THRESHOLD, 18.0).clamp(0.0, 95.0),
            custom_bevel: f(I_CUSTOM_BEVEL, 16.0).clamp(1.0, 80.0),
            custom_disp: f(I_CUSTOM_DISP, 100.0).clamp(0.0, 300.0) * 0.01,

            big_density: clamp01(f(I_BIG_DENSITY, 55.0) * 0.01),
            big_size: f(I_BIG_SIZE, 55.0).clamp(10.0, 200.0),
            big_phase: f(I_BIG_PHASE, 0.0),
            big_disp: f(I_BIG_DISP, 100.0).clamp(0.0, 300.0) * 0.01,

            small_density: clamp01(f(I_SMALL_DENSITY, 65.0) * 0.01),
            small_size: f(I_SMALL_SIZE, 50.0).clamp(10.0, 200.0),
            small_phase: f(I_SMALL_PHASE, 0.0),
            small_disp: f(I_SMALL_DISP, 100.0).clamp(0.0, 300.0) * 0.01,

            streak_density: clamp01(f(I_STREAK_DENSITY, 55.0) * 0.01),
            streak_size: f(I_STREAK_SIZE, 55.0).clamp(10.0, 200.0),
            streak_phase: f(I_STREAK_PHASE, 0.0),
            streak_disp: f(I_STREAK_DISP, 100.0).clamp(0.0, 300.0) * 0.01,

            flow_speed: f(I_FLOW_SPEED, 60.0).clamp(0.0, 300.0),
            master_disp: f(I_DISPLACEMENT, 100.0).clamp(0.0, 300.0) * 0.01,
            dispersion: clamp01(f(I_DISPERSION, 22.0) * 0.01),
            hi_k: f(I_HIGHLIGHT, 85.0).clamp(0.0, 300.0) * 0.01,
            light_angle: f(I_LIGHT_ANGLE, 120.0),
            bg_blur: clamp01(f(I_BG_BLUR, 18.0) * 0.01),

            seed: f(I_SEED, 0.0).clamp(0.0, 100.0),
            sampling: f(I_SAMPLING, 0.0).round() as i32,
            edge_mode: f(I_EDGE_MODE, 0.0).round() as i32,
            mix: clamp01(f(I_MIX, 100.0) * 0.01),
        }
    }
}

// ---------------------------------------------------------------- Math & Sampling Helpers

#[inline]
fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

#[inline]
fn smoothstep01(t: f32) -> f32 {
    let t = clamp01(t);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn fract01(v: f32) -> f32 {
    let f = v - v.floor();
    if f < 0.0 { f + 1.0 } else { f }
}

#[inline]
fn inv_sqrt(v: f32) -> f32 {
    if v > 0.0 { 1.0 / v.sqrt() } else { 0.0 }
}

#[inline]
fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

#[inline]
fn unit(v: u32) -> f32 {
    (v >> 8) as f32 * (1.0 / 16_777_216.0)
}

#[derive(Clone, Copy, Default)]
struct Rgba {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

#[inline]
fn wrap1(i: i32, n: i32, edge: i32) -> (usize, bool) {
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
fn cubic_weight(t: f32) -> f32 {
    let a = t.abs();
    if a <= 1.0 {
        (1.5 * a - 2.5) * a * a + 1.0
    } else if a < 2.0 {
        ((-0.5 * a + 2.5) * a - 4.0) * a + 2.0
    } else {
        0.0
    }
}

#[inline]
fn sample_nearest(src: &[f32], w: i32, h: i32, edge: i32, fx: f32, fy: f32) -> Rgba {
    let xi = fx.round() as i32;
    let yi = fy.round() as i32;
    let (xx, okx) = wrap1(xi, w, edge);
    let (yy, oky) = wrap1(yi, h, edge);
    if !okx || !oky {
        return Rgba::default();
    }
    let o = (yy * w as usize + xx) * 4;
    Rgba {
        r: src[o],
        g: src[o + 1],
        b: src[o + 2],
        a: src[o + 3],
    }
}

#[inline]
fn sample_bilinear(src: &[f32], w: i32, h: i32, edge: i32, fx: f32, fy: f32) -> Rgba {
    if fx >= 0.0 && fy >= 0.0 && fx < (w - 1) as f32 && fy < (h - 1) as f32 {
        let x0f = fx.floor();
        let y0f = fy.floor();
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
            r: src[o00] * w00 + src[o01] * w01 + src[o10] * w10 + src[o11] * w11,
            g: src[o00 + 1] * w00 + src[o01 + 1] * w01 + src[o10 + 1] * w10 + src[o11 + 1] * w11,
            b: src[o00 + 2] * w00 + src[o01 + 2] * w01 + src[o10 + 2] * w10 + src[o11 + 2] * w11,
            a: src[o00 + 3] * w00 + src[o01 + 3] * w01 + src[o10 + 3] * w10 + src[o11 + 3] * w11,
        };
    }
    let x0f = fx.floor();
    let y0f = fy.floor();
    let x0 = x0f as i32;
    let y0 = y0f as i32;
    let tx = fx - x0f;
    let ty = fy - y0f;
    let mut out = Rgba::default();
    for j in 0..2 {
        let (yy, oky) = wrap1(y0 + j, h, edge);
        if oky {
            let wy = if j == 0 { 1.0 - ty } else { ty };
            if wy > 0.0 {
                for i in 0..2 {
                    let (xx, okx) = wrap1(x0 + i, w, edge);
                    if okx {
                        let wx = if i == 0 { 1.0 - tx } else { tx };
                        let wt = wx * wy;
                        if wt > 0.0 {
                            let o = (yy * w as usize + xx) * 4;
                            out.r += src[o] * wt;
                            out.g += src[o + 1] * wt;
                            out.b += src[o + 2] * wt;
                            out.a += src[o + 3] * wt;
                        }
                    }
                }
            }
        }
    }
    out
}

#[inline]
fn sample_bicubic(src: &[f32], w: i32, h: i32, edge: i32, fx: f32, fy: f32) -> Rgba {
    let x0 = fx.floor() as i32;
    let y0 = fy.floor() as i32;
    let mut out = Rgba::default();
    for j in -1..=2 {
        let (yy, oky) = wrap1(y0 + j, h, edge);
        if oky {
            let wy = cubic_weight(fy - (y0 + j) as f32);
            if wy != 0.0 {
                for i in -1..=2 {
                    let (xx, okx) = wrap1(x0 + i, w, edge);
                    if okx {
                        let wt = wy * cubic_weight(fx - (x0 + i) as f32);
                        if wt != 0.0 {
                            let o = (yy * w as usize + xx) * 4;
                            out.r += src[o] * wt;
                            out.g += src[o + 1] * wt;
                            out.b += src[o + 2] * wt;
                            out.a += src[o + 3] * wt;
                        }
                    }
                }
            }
        }
    }
    out
}

#[inline]
fn sample(src: &[f32], w: i32, h: i32, edge: i32, method: i32, fx: f32, fy: f32) -> Rgba {
    if method == 2 {
        sample_nearest(src, w, h, edge, fx, fy)
    } else if method == 1 {
        sample_bicubic(src, w, h, edge, fx, fy)
    } else {
        sample_bilinear(src, w, h, edge, fx, fy)
    }
}

fn resample_rgba(src: &[f32], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<f32> {
    if sw == dw && sh == dh {
        return src.to_vec();
    }
    let mut out = vec![0.0f32; dw * dh * 4];
    if sw == 0 || sh == 0 {
        return out;
    }
    for y in 0..dh {
        let sy = ((y as f32 + 0.5) * sh as f32 / dh as f32 - 0.5).clamp(0.0, (sh - 1) as f32);
        let y0 = sy.floor() as usize;
        let y1 = (y0 + 1).min(sh - 1);
        let fy = sy - y0 as f32;
        for x in 0..dw {
            let sx =
                ((x as f32 + 0.5) * sw as f32 / dw as f32 - 0.5).clamp(0.0, (sw - 1) as f32);
            let x0 = sx.floor() as usize;
            let x1 = (x0 + 1).min(sw - 1);
            let fx = sx - x0 as f32;
            for c in 0..4 {
                let p00 = src[(y0 * sw + x0) * 4 + c];
                let p10 = src[(y0 * sw + x1) * 4 + c];
                let p01 = src[(y1 * sw + x0) * 4 + c];
                let p11 = src[(y1 * sw + x1) * 4 + c];
                let top = p00 + (p10 - p00) * fx;
                let bot = p01 + (p11 - p01) * fx;
                out[(y * dw + x) * 4 + c] = top + (bot - top) * fy;
            }
        }
    }
    out
}

// ---------------------------------------------------------------- Procedural Rain Field (v4.0 Parity)

#[inline]
fn stick_slip(ph: f32, wobble_off: f32) -> f32 {
    let tau = ph * TAU + wobble_off;
    let delta =
        0.32 * (tau.sin() - wobble_off.sin()) + 0.05 * ((2.0 * tau).sin() - (2.0 * wobble_off).sin());
    ph + delta * (1.0 / TAU)
}

#[inline]
fn streak_path(y: f32, x0: f32, p1: f32, p2: f32, p3: f32, amp: f32) -> (f32, f32) {
    let f1 = 0.010;
    let f2 = 0.028;
    let f3 = 0.068;
    let a1 = y * f1 + p1;
    let a2 = y * f2 + p2;
    let a3 = y * f3 + p3;
    let x = x0 + amp * (0.55 * a1.sin() + 0.30 * a2.sin() + 0.15 * a3.sin());
    let slope = amp * (0.55 * f1 * a1.cos() + 0.30 * f2 * a2.cos() + 0.15 * f3 * a3.cos());
    (x, slope)
}

struct RainField {
    w: usize,
    h: usize,
    fbuf: Vec<f32>,
    gxb: Vec<f32>,
    gyb: Vec<f32>,
    dxb: Vec<f32>,
    dyb: Vec<f32>,
    wbuf: Vec<f32>,
}

impl RainField {
    fn new(w: usize, h: usize) -> Self {
        let n = w * h;
        Self {
            w,
            h,
            fbuf: vec![0.0; n],
            gxb: vec![0.0; n],
            gyb: vec![0.0; n],
            dxb: vec![0.0; n],
            dyb: vec![0.0; n],
            wbuf: vec![0.0; n],
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn splat_organic_drop(
    fld: &mut RainField,
    cx: f32,
    cy: f32,
    ra_vis: f32,
    rb_vis: f32,
    sag: f32,
    w2c: f32,
    w2s: f32,
    w3c: f32,
    w3s: f32,
    weight: f32,
    type_disp: f32,
) {
    if ra_vis < 0.55 || rb_vis < 0.55 || weight <= CONTACT_T * 0.55 {
        return;
    }
    let ra_s = ra_vis * INV_QC;
    let rb_s = rb_vis * INV_QC;
    let pad_x = ra_s * (1.10 + sag * 1.10);
    let pad_y = rb_s * 1.15;

    let wf1 = (fld.w - 1) as f32;
    let hf1 = (fld.h - 1) as f32;
    let x0f = (cx - pad_x).floor();
    let x1f = (cx + pad_x).ceil();
    let y0f = (cy - pad_y).floor();
    let y1f = (cy + pad_y).ceil();
    if x1f < 0.0 || y1f < 0.0 || x0f > wf1 || y0f > hf1 {
        return;
    }

    let x0 = x0f.max(0.0) as usize;
    let x1 = x1f.min(wf1) as usize;
    let y0 = y0f.max(0.0) as usize;
    let y1 = y1f.min(hf1) as usize;

    let inv_ra_s = 1.0 / ra_s;
    let inv_rb_s = 1.0 / rb_s;
    let belly_shift = 0.18 * sag;
    let disp_rx = ra_vis * 2.30 * type_disp;
    let disp_ry = rb_vis * 2.30 * type_disp;
    let gw = fld.w;

    for py in y0..=y1 {
        let dy_px = py as f32 + 0.5 - cy;
        let v = dy_px * inv_rb_s;
        let v_c = v.clamp(-1.0, 1.0);
        let width_scale = 1.0 + sag * v_c;
        let inv_ws = 1.0 / width_scale;
        let v_belly = v - belly_shift;
        let vb2 = v_belly * v_belly;
        let sag_y_k = if v > -1.0 && v < 1.0 {
            sag * inv_ws * inv_rb_s
        } else {
            0.0
        };
        let row = py * gw;

        for px in x0..=x1 {
            let dx_px = px as f32 + 0.5 - cx;
            let u = dx_px * inv_ra_s;
            let u_eff = u * inv_ws;
            let r2 = u_eff * u_eff + vb2;
            if r2 >= 1.21 {
                continue;
            }

            let (rmod, q2) = if r2 > 1.0e-6 {
                let inv_r = inv_sqrt(r2);
                let ca = u_eff * inv_r;
                let sa = v_belly * inv_r;
                let c2 = ca * ca - sa * sa;
                let s2 = 2.0 * ca * sa;
                let c3 = c2 * ca - s2 * sa;
                let s3 = s2 * ca + c2 * sa;
                let rm = 1.0 + w2c * c2 + w2s * s2 + w3c * c3 + w3s * s3;
                (rm, r2 / (rm * rm))
            } else {
                (1.0, 0.0)
            };

            if q2 < 1.0 {
                let t = 1.0 - q2;
                let f = t * t * weight;
                let idx = row + px;
                fld.fbuf[idx] += f;

                let k_g = -4.0 * weight * t / (rmod * rmod);
                let df_due = k_g * u_eff;
                let df_dvb = k_g * v_belly;
                let df_dx = df_due * inv_ra_s * inv_ws;
                let df_dy = df_dvb * inv_rb_s - df_due * u_eff * sag_y_k;
                fld.gxb[idx] += df_dx;
                fld.gyb[idx] += df_dy;

                let q_vis2 = (q2 * 2.22).min(1.0);
                let bend = 1.0 + 0.85 * q_vis2;
                let inv_rm = 1.0 / rmod;
                let disp_x = -(u_eff * inv_rm) * disp_rx * bend;
                let disp_y = -(v_belly * inv_rm) * disp_ry * bend;
                fld.dxb[idx] += disp_x * f;
                fld.dyb[idx] += disp_y * f;
            }
        }
    }
}

#[inline]
fn paint_wipe(fld: &mut RainField, cx: f32, py: usize, wipe_r: f32, strength: f32) {
    if wipe_r < 0.5 || strength <= 0.0 {
        return;
    }
    let wf1 = (fld.w - 1) as f32;
    let x0f = (cx - wipe_r).floor();
    let x1f = (cx + wipe_r).ceil();
    if x1f < 0.0 || x0f > wf1 {
        return;
    }
    let x0 = x0f.max(0.0) as usize;
    let x1 = x1f.min(wf1) as usize;
    let inv_wr = 1.0 / wipe_r;
    let row = py * fld.w;
    for px in x0..=x1 {
        let d = (px as f32 + 0.5 - cx).abs() * inv_wr;
        if d < 1.0 {
            let wp = smoothstep01(1.0 - d) * strength;
            let idx = row + px;
            if wp > fld.wbuf[idx] {
                fld.wbuf[idx] = wp;
            }
        }
    }
}

fn build_rain_field(w: usize, h: usize, p: &Params, time: f32) -> RainField {
    let mut fld = RainField::new(w, h);
    let wf = w as f32;
    let hf = h as f32;
    let min_dim = wf.min(hf);
    let salt = (p.seed * 1000.0) as u32;
    let speed_k = p.flow_speed * 0.01;

    // Layer 1: Streaks
    if p.streak_density > 0.001 {
        let count = ((1.0 + (p.streak_density * 25.0).floor()).min(MAX_STREAKS as f32)) as usize;
        let sz_s = 0.35 + 1.35 * (p.streak_size * 0.01);
        let t_streak = time * speed_k * 0.90 + p.streak_phase * 0.25;

        for s in 0..count {
            let h0 =
                hash_u32(0x51ed_2701u32.wrapping_add((s as u32).wrapping_mul(0x9e37_79b9)) ^ salt);
            let h1 = hash_u32(h0 ^ 0xbb67_ae85);
            let h2 = hash_u32(h1.wrapping_add(0x3c6e_f372));
            let h3 = hash_u32(h2 ^ 0x510e_527f);
            let h4 = hash_u32(h3.wrapping_add(0x1f83_d9ab));
            let h5 = hash_u32(h4 ^ 0x68e3_1da4);
            let h6 = hash_u32(h5.wrapping_add(0x7f4a_7c15));

            let x0 = (0.04 + 0.92 * unit(h0)) * wf;
            let p1 = unit(h1) * TAU;
            let p2 = unit(h2) * TAU;
            let p3 = unit(h3) * TAU;
            let amp = min_dim * (0.024 + 0.042 * unit(h2));
            let head_r = (min_dim * (0.010 + 0.0095 * unit(h3)) * sz_s).max(2.6);
            let trail_len = hf * (0.28 + 0.45 * unit(h4)) * (0.65 + 0.35 * (p.streak_size * 0.01));
            let cycle = 4.2 + 3.8 * unit(h5);
            let ph = fract01(unit(h6) + t_streak / cycle);
            let u_fall = stick_slip(ph, unit(h1) * TAU);
            let margin = hf * 0.10 + head_r * 2.5;
            let y_head = -margin + u_fall * (hf + trail_len + 2.0 * margin);

            let y0 = (y_head - trail_len).floor().max(0.0) as usize;
            let y1 = y_head.ceil().min(hf - 1.0) as usize;
            let inv_tl = 1.0 / trail_len.max(1.0);
            let wf1 = wf - 1.0;

            if y1 >= y0 && y_head >= 0.0 {
                for py in y0..=y1 {
                    let y_center = py as f32 + 0.5;
                    let d = y_head - y_center;
                    if d >= 0.0 && d <= trail_len {
                        let t_tr = d * inv_tl;
                        let (xc, slope) = streak_path(y_center, x0, p1, p2, p3, amp);
                        let w_mod = 0.65 + 0.35 * (y_center * 0.047 + p2).sin();
                        let tail_fade = smoothstep01((1.0 - t_tr) * 5.0);
                        let w_vis = head_r
                            * (0.38 + 0.22 * (1.0 - t_tr))
                            * w_mod
                            * (0.45 + 0.55 * tail_fade);
                        let weight = 0.72 + 0.28 * tail_fade;
                        let wipe_r = head_r * (1.45 + 0.25 * (1.0 - t_tr));
                        paint_wipe(&mut fld, xc, py, wipe_r, 0.92 * tail_fade);

                        if w_vis >= 0.60 {
                            let norm_s = inv_sqrt(1.0 + slope * slope);
                            let nx_p = norm_s;
                            let ny_p = -slope * norm_s;
                            let w_horiz = w_vis / norm_s;
                            let w_s = w_horiz * INV_QC;
                            let inv_ws = 1.0 / w_s;

                            let x0f = (xc - w_s).floor();
                            let x1f = (xc + w_s).ceil();
                            if x1f >= 0.0 && x0f <= wf1 {
                                let x0i = x0f.max(0.0) as usize;
                                let x1i = x1f.min(wf1) as usize;
                                let row = py * w;
                                for px in x0i..=x1i {
                                    let dx_px = px as f32 + 0.5 - xc;
                                    let u_perp = dx_px * inv_ws;
                                    let q2 = u_perp * u_perp;
                                    if q2 < 1.0 {
                                        let tt = 1.0 - q2;
                                        let f = tt * tt * weight;
                                        let idx = row + px;
                                        fld.fbuf[idx] += f;

                                        let df_du = -4.0 * weight * tt * u_perp;
                                        fld.gxb[idx] += df_du * inv_ws;
                                        fld.gyb[idx] += -df_du * slope * inv_ws;

                                        let q_vis2 = (q2 * 2.22).min(1.0);
                                        let disp_p = -u_perp
                                            * w_vis
                                            * 2.70
                                            * (1.0 + 0.75 * q_vis2)
                                            * p.streak_disp;
                                        fld.dxb[idx] += disp_p * nx_p * f;
                                        fld.dyb[idx] += disp_p * ny_p * f;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if y_head > -head_r * 3.0 && y_head < hf + head_r * 3.0 {
                let (hx, _) = streak_path(y_head, x0, p1, p2, p3, amp);
                splat_organic_drop(
                    &mut fld,
                    hx,
                    y_head,
                    head_r * 0.88,
                    head_r * 1.38,
                    0.30,
                    (unit(h2) - 0.5) * 0.08,
                    (unit(h3) - 0.5) * 0.08,
                    (unit(h4) - 0.5) * 0.06,
                    (unit(h5) - 0.5) * 0.06,
                    1.0,
                    p.streak_disp,
                );
            }

            let step = (head_r * 4.8).max(26.0);
            let y_lo = (y_head - trail_len * 1.15).max(-step);
            let y_hi = (y_head - head_r * 1.8).min(hf + step);
            if y_hi > y_lo {
                let b0 = (y_lo / step).floor() as i32;
                let b1 = (y_hi / step).ceil() as i32;
                for bi in b0..=b1 {
                    let bh = hash_u32(
                        (bi as u32).wrapping_mul(0x85eb_ca6b)
                            ^ (s as u32).wrapping_mul(0x27d4_eb2d)
                            ^ salt,
                    );
                    if unit(bh) < 0.72 {
                        let bh2 = hash_u32(bh ^ 0xa5a5_5a5a);
                        let bh3 = hash_u32(bh2.wrapping_add(0x6a09_e667));
                        let y_b = (bi as f32 + 0.25 + 0.50 * unit(bh2)) * step;
                        let lag = y_head - y_b;
                        if lag > head_r * 1.8 && lag < trail_len * 1.15 {
                            let lag_t = clamp01(lag / (trail_len * 1.15).max(1.0));
                            let fade_b = 1.0 - smoothstep01((lag_t - 0.65) * 2.86) * 0.45;
                            let rb = head_r * (0.28 + 0.22 * unit(bh3)) * (1.0 - 0.50 * lag_t);
                            let (xb, _) = streak_path(y_b, x0, p1, p2, p3, amp);
                            splat_organic_drop(
                                &mut fld,
                                xb,
                                y_b,
                                rb * 0.90,
                                rb * 1.18,
                                0.22,
                                (unit(bh) - 0.5) * 0.08,
                                (unit(bh2) - 0.5) * 0.08,
                                (unit(bh3) - 0.5) * 0.06,
                                (unit(bh ^ bh3) - 0.5) * 0.06,
                                fade_b,
                                p.streak_disp,
                            );
                        }
                    }
                }
            }
        }
    }

    // Layer 2: Large Drops
    if p.big_density > 0.001 {
        let cell_big = (min_dim * 0.088).max(24.0);
        let inv_cb = 1.0 / cell_big;
        let gwi = (wf * inv_cb).ceil() as i32 + 1;
        let ghi = (hf * inv_cb).ceil() as i32 + 1;
        let sz_b = 0.35 + 1.35 * (p.big_size * 0.01);
        let t_big = time * speed_k * 0.55 + p.big_phase * 0.25;

        for gy in -1..=ghi {
            for gx in -1..=gwi {
                let seed0 = (gx as u32).wrapping_mul(0x9e37_79b9)
                    ^ (gy as u32).wrapping_mul(0x85eb_ca6b)
                    ^ 0x3c6e_f372
                    ^ salt;
                let h0 = hash_u32(seed0);
                let cm = unit(hash_u32(
                    ((gx >> 1) as u32).wrapping_mul(0x27d4_eb2d)
                        ^ ((gy >> 1) as u32).wrapping_mul(0x1656_67b1)
                        ^ salt,
                ));
                if unit(h0) > p.big_density * 0.85 || cm > 0.35 + 0.65 * p.big_density {
                    continue;
                }

                let h1 = hash_u32(h0 ^ 0xa5a5_a5a5);
                let h2 = hash_u32(h1.wrapping_add(0x1234_5677));
                let h3 = hash_u32(h2 ^ 0x68e3_1da4);
                let h4 = hash_u32(h3.wrapping_add(0x7f4a_7c15));
                let h5 = hash_u32(h4 ^ 0x2545_f491);
                let h6 = hash_u32(h5.wrapping_add(0x6a09_e667));
                let h7 = hash_u32(h6 ^ 0xbb67_ae85);

                let bx = (gx as f32 + 0.5 + (unit(h1) - 0.5) * 0.92) * cell_big;
                let by = (gy as f32 + 0.5 + (unit(h2) - 0.5) * 0.92) * cell_big;
                let r_vis = min_dim * (0.0082 + 0.0145 * unit(h3)) * sz_b * (0.78 + 0.42 * cm);

                let w2c = (unit(h4) - 0.5) * 0.11;
                let w2s = (unit(h5) - 0.5) * 0.11;
                let w3c = (unit(h6) - 0.5) * 0.08;
                let w3s = (unit(h7) - 0.5) * 0.08;

                let is_slider = unit(h4) < 0.60;
                let life = 5.8 + 6.5 * unit(h5);
                let ph = fract01(unit(h6) + t_big / life);
                let emerge = smoothstep01(clamp01(ph * 12.0));
                let vanish = 1.0 - smoothstep01(clamp01((ph - 0.88) * 8.3));
                let life_scale = emerge * vanish;

                if life_scale > 0.18 {
                    if is_slider {
                        let slide_dist = cell_big * (1.2 + 2.2 * unit(h7));
                        let u_slip = stick_slip(ph, unit(h2) * TAU);
                        let cy = by + slide_dist * u_slip;
                        let drift_amp = r_vis * (0.5 + 1.1 * unit(h1));
                        let drift_p = unit(h3) * TAU;
                        let cx = bx + drift_amp * (cy * 0.045 + drift_p).sin();

                        let wy0 = by.floor().max(0.0) as usize;
                        let wy1 = cy.ceil().min(hf - 1.0) as usize;
                        if wy1 >= wy0 && cy >= 0.0 {
                            for wy in wy0..=wy1 {
                                let wx = bx + drift_amp * ((wy as f32 + 0.5) * 0.045 + drift_p).sin();
                                paint_wipe(&mut fld, wx, wy, r_vis * 1.15, 0.85 * life_scale);
                            }
                        }

                        if u_slip > 0.35 {
                            let y_res = by + slide_dist * 0.28;
                            let x_res = bx + drift_amp * (y_res * 0.045 + drift_p).sin();
                            let r_res = r_vis * (0.28 + 0.12 * unit(h7)) * vanish;
                            splat_organic_drop(
                                &mut fld,
                                x_res,
                                y_res,
                                r_res * 0.92,
                                r_res * 1.15,
                                0.18,
                                w2c,
                                w2s,
                                w3c,
                                w3s,
                                0.90,
                                p.big_disp,
                            );
                        }

                        let r_cur = r_vis * (0.65 + 0.35 * life_scale);
                        let sag = 0.24 + 0.10 * u_slip;
                        let aspect_y = 1.18 + 0.18 * unit(h7);
                        splat_organic_drop(
                            &mut fld,
                            cx,
                            cy,
                            r_cur * 0.92,
                            r_cur * aspect_y,
                            sag,
                            w2c,
                            w2s,
                            w3c,
                            w3s,
                            1.0,
                            p.big_disp,
                        );
                    } else {
                        let r_cur = r_vis * (0.62 + 0.38 * life_scale);
                        let sag = 0.22 + 0.08 * ph;
                        let aspect_y = 1.12 + 0.14 * unit(h7);
                        splat_organic_drop(
                            &mut fld,
                            bx,
                            by,
                            r_cur * (0.94 + 0.12 * unit(h1)),
                            r_cur * aspect_y,
                            sag,
                            w2c,
                            w2s,
                            w3c,
                            w3s,
                            1.0,
                            p.big_disp,
                        );
                    }
                }
            }
        }
    }

    // Layer 3: Small Drops + Micro Condensation
    if p.small_density > 0.001 {
        let sz_m = 0.35 + 1.35 * (p.small_size * 0.01);
        let t_small = time * speed_k * 0.22 + p.small_phase * 0.25;

        let cell_s1 = (min_dim * 0.030).max(11.0);
        let inv_s1 = 1.0 / cell_s1;
        let gw1 = (wf * inv_s1).ceil() as i32 + 1;
        let gh1 = (hf * inv_s1).ceil() as i32 + 1;

        for gy in -1..=gh1 {
            for gx in -1..=gw1 {
                let h0 = hash_u32(
                    (gx as u32).wrapping_mul(0x9e37_79b9)
                        ^ (gy as u32).wrapping_mul(0x85eb_ca6b)
                        ^ 0x1f83_d9ab
                        ^ salt,
                );
                if unit(h0) > p.small_density * 0.80 {
                    continue;
                }
                let h1 = hash_u32(h0 ^ 0xbb67_ae85);
                let h2 = hash_u32(h1.wrapping_add(0x3c6e_f372));
                let h3 = hash_u32(h2 ^ 0x510e_527f);
                let h4 = hash_u32(h3.wrapping_add(0x7f4a_7c15));
                let h5 = hash_u32(h4 ^ 0x2545_f491);

                let life = 4.0 + 5.5 * unit(h4);
                let ph = fract01(unit(h5) + t_small / life);
                let life_scale = smoothstep01(clamp01(ph * 10.0))
                    * (1.0 - smoothstep01(clamp01((ph - 0.85) * 6.6)));
                if life_scale <= 0.20 {
                    continue;
                }

                let r_vis =
                    min_dim * (0.0034 + 0.0046 * unit(h3)) * sz_m * (0.65 + 0.35 * life_scale);
                if r_vis < 1.05 {
                    continue;
                }
                let bx = (gx as f32 + 0.5 + (unit(h1) - 0.5) * 0.94) * cell_s1;
                let slip = if unit(h4) < 0.22 {
                    r_vis * (0.5 + 1.3 * unit(h5)) * stick_slip(ph, unit(h1) * TAU)
                } else {
                    0.0
                };
                let by = (gy as f32 + 0.5 + (unit(h2) - 0.5) * 0.94) * cell_s1 + slip;

                if bx >= 0.0 && bx < wf && by >= 0.0 && by < hf {
                    let ix = (bx as usize).min(w - 1);
                    let iy = (by as usize).min(h - 1);
                    let idx = iy * w + ix;
                    if fld.wbuf[idx] > 0.28 || fld.fbuf[idx] > 0.24 {
                        continue;
                    }
                }

                splat_organic_drop(
                    &mut fld,
                    bx,
                    by,
                    r_vis * (0.90 + 0.18 * unit(h1)),
                    r_vis * (1.02 + 0.22 * unit(h2)),
                    0.16,
                    (unit(h1) - 0.5) * 0.12,
                    (unit(h2) - 0.5) * 0.12,
                    (unit(h3) - 0.5) * 0.08,
                    (unit(h4) - 0.5) * 0.08,
                    1.0,
                    p.small_disp,
                );
            }
        }

        let cell_s2 = (min_dim * 0.0155).max(6.0);
        let inv_s2 = 1.0 / cell_s2;
        let gw2 = (wf * inv_s2).ceil() as i32 + 1;
        let gh2 = (hf * inv_s2).ceil() as i32 + 1;

        for gy in -1..=gh2 {
            for gx in -1..=gw2 {
                let h0 = hash_u32(
                    (gx as u32).wrapping_mul(0x27d4_eb2d)
                        ^ (gy as u32).wrapping_mul(0x1656_67b1)
                        ^ 0x6a09_e667
                        ^ salt,
                );
                if unit(h0) > p.small_density * 0.58 {
                    continue;
                }
                let h1 = hash_u32(h0 ^ 0x510e_527f);
                let h2 = hash_u32(h1.wrapping_add(0x9e37_79b9));
                let h3 = hash_u32(h2 ^ 0x85eb_ca6b);
                let h4 = hash_u32(h3.wrapping_add(0x3c6e_f372));

                let life = 3.2 + 4.5 * unit(h4);
                let ph = fract01(unit(h3) + t_small / life);
                let life_scale = smoothstep01(clamp01(ph * 10.0))
                    * (1.0 - smoothstep01(clamp01((ph - 0.84) * 6.2)));
                if life_scale <= 0.25 {
                    continue;
                }

                let r_vis =
                    min_dim * (0.0023 + 0.0025 * unit(h3)) * sz_m * (0.68 + 0.32 * life_scale);
                if r_vis < 1.05 {
                    continue;
                }
                let bx = (gx as f32 + 0.5 + (unit(h1) - 0.5) * 0.92) * cell_s2;
                let by = (gy as f32 + 0.5 + (unit(h2) - 0.5) * 0.92) * cell_s2;

                if bx >= 0.0 && bx < wf && by >= 0.0 && by < hf {
                    let ix = (bx as usize).min(w - 1);
                    let iy = (by as usize).min(h - 1);
                    let idx = iy * w + ix;
                    if fld.wbuf[idx] > 0.28 || fld.fbuf[idx] > 0.24 {
                        continue;
                    }
                }

                splat_organic_drop(
                    &mut fld,
                    bx,
                    by,
                    r_vis,
                    r_vis * (1.02 + 0.16 * unit(h4)),
                    0.12,
                    (unit(h1) - 0.5) * 0.10,
                    (unit(h2) - 0.5) * 0.10,
                    0.0,
                    0.0,
                    1.0,
                    p.small_disp,
                );
            }
        }
    }

    fld
}

// ---------------------------------------------------------------- Custom Map Field

struct CustomField {
    b_edge: Vec<f32>,
    h_dome: Vec<f32>,
    bevel_px: f32,
    disp_scale: f32,
}

fn build_custom_field(
    layers: &mut dyn Layers,
    w: usize,
    h: usize,
    scale_f: f32,
    time: f64,
    p: &Params,
) -> Option<CustomField> {
    let (cw, ch, cpx) = layers.layer_image("custom_layer", p.custom_layer, time, w, h)?;
    let n = w * h;
    if cw == 0 || ch == 0 || cpx.len() < cw * ch * 4 {
        return None;
    }
    let aligned = if cw == w && ch == h {
        cpx
    } else {
        resample_rgba(&cpx, cw, ch, w, h)
    };

    let thresh = (p.custom_threshold * 0.01).clamp(0.0, 0.95);
    let lo = (thresh - 0.03).max(0.0);
    let hi = (thresh + 0.03).max(lo + 0.05);
    let inv_win = 1.0 / (hi - lo);
    let inv_rel = 1.0 / (1.0 - thresh).max(0.06);

    let mut mask_in = vec![0.0f32; n];
    let mut height_src = vec![0.0f32; n];
    let is_alpha = p.custom_channel == 1;
    let invert = p.custom_invert;
    let threads = thread::available_parallelism()
        .map(|t| t.get())
        .unwrap_or(1)
        .min(h)
        .max(1);
    let rows_per = h.div_ceil(threads);
    let any_wet = if threads <= 1 || h < 64 {
        let mut wet = false;
        for (i, px_c) in aligned[..n * 4].chunks_exact(4).enumerate() {
            let mut raw = if is_alpha {
                px_c[3]
            } else {
                0.2126 * px_c[0] + 0.7152 * px_c[1] + 0.0722 * px_c[2]
            };
            raw = clamp01(raw);
            if invert {
                raw = 1.0 - raw;
            }
            if raw > lo {
                wet = true;
                let m = smoothstep01((raw - lo) * inv_win);
                let rel = clamp01((raw - thresh) * inv_rel);
                mask_in[i] = m;
                height_src[i] = m * (0.52 + 0.48 * rel);
            }
        }
        wet
    } else {
        let aligned_ref = &aligned[..n * 4];
        let mut wet_flags = vec![false; threads];
        thread::scope(|scope| {
            for (((m_chunk, h_chunk), a_chunk), flag) in mask_in
                .chunks_mut(rows_per * w)
                .zip(height_src.chunks_mut(rows_per * w))
                .zip(aligned_ref.chunks(rows_per * w * 4))
                .zip(wet_flags.iter_mut())
            {
                scope.spawn(move || {
                    let mut local_wet = false;
                    for ((m_dst, h_dst), px_c) in m_chunk
                        .iter_mut()
                        .zip(h_chunk.iter_mut())
                        .zip(a_chunk.chunks_exact(4))
                    {
                        let mut raw = if is_alpha {
                            px_c[3]
                        } else {
                            0.2126 * px_c[0] + 0.7152 * px_c[1] + 0.0722 * px_c[2]
                        };
                        raw = clamp01(raw);
                        if invert {
                            raw = 1.0 - raw;
                        }
                        if raw > lo {
                            local_wet = true;
                            let m = smoothstep01((raw - lo) * inv_win);
                            let rel = clamp01((raw - thresh) * inv_rel);
                            *m_dst = m;
                            *h_dst = m * (0.52 + 0.48 * rel);
                        }
                    }
                    *flag = local_wet;
                });
            }
        });
        wet_flags.into_iter().any(|w| w)
    };

    if !any_wet {
        return None;
    }

    let bevel_px = (p.custom_bevel * scale_f).max(1.0);
    let b_edge = blur::gauss_plane(&mask_in, w, h, (0.85 * scale_f).max(0.55));
    let b_mid = blur::gauss_plane(&height_src, w, h, (bevel_px * 0.22).max(1.0));
    let mut b_wide = blur::gauss_plane_downsampled(&height_src, w, h, (bevel_px * 0.55).max(2.0));

    for (wide, mid) in b_wide.iter_mut().zip(b_mid.iter()) {
        *wide = 0.42 * *mid + 0.58 * *wide;
    }
    let h_dome = b_wide;

    Some(CustomField {
        b_edge,
        h_dome,
        bevel_px,
        disp_scale: p.custom_disp,
    })
}

#[inline]
fn sobel_grad(buf: &[f32], w: usize, h: usize, x: usize, y: usize) -> (f32, f32) {
    let xm = x.saturating_sub(1);
    let xp = (x + 1).min(w - 1);
    let ym = y.saturating_sub(1);
    let yp = (y + 1).min(h - 1);

    let r0 = ym * w;
    let r1 = y * w;
    let r2 = yp * w;

    let a = buf[r0 + xm];
    let b = buf[r0 + x];
    let c = buf[r0 + xp];
    let d = buf[r1 + xm];
    let f = buf[r1 + xp];
    let g = buf[r2 + xm];
    let h_v = buf[r2 + x];
    let i = buf[r2 + xp];

    let gx = ((c - a) + 2.0 * (f - d) + (i - g)) * 0.125;
    let gy = ((g - a) + 2.0 * (h_v - b) + (i - c)) * 0.125;
    (gx, gy)
}

// ---------------------------------------------------------------- Unified Shading & Output

#[derive(Clone, Copy)]
struct DropSample {
    alpha: f32,
    q_norm: f32,
    rim_band: f32,
    ex: f32,
    ey: f32,
    spec_aa: f32,
    dx: f32,
    dy: f32,
}

impl DropSample {
    const ZERO: Self = Self {
        alpha: 0.0,
        q_norm: 0.0,
        rim_band: 0.0,
        ex: 0.0,
        ey: 0.0,
        spec_aa: 1.0,
        dx: 0.0,
        dy: 0.0,
    };
}

pub fn render(
    layers: &mut dyn Layers,
    px: &mut [f32],
    w: usize,
    h: usize,
    scale: f64,
    time: f64,
    flat: &[f64],
) -> Result<(), String> {
    if w == 0 || h == 0 || px.len() < w * h * 4 {
        return Err("frame too small".into());
    }
    if w * h > 16_000_000 {
        return Err("frame too large".into());
    }

    let p = Params::parse(flat);
    let scale_f = if scale.is_finite() && scale > 0.0 {
        scale as f32
    } else {
        1.0
    };
    let t = if time.is_finite() {
        (time as f32).max(0.0)
    } else {
        0.0
    };

    let has_custom_layer = p.custom_layer >= 0;
    let use_custom = has_custom_layer && p.custom_mode != 2;
    let use_proc = !has_custom_layer || p.custom_mode != 0;
    let any_proc_drops =
        use_proc && (p.big_density > 0.001 || p.small_density > 0.001 || p.streak_density > 0.001);

    if p.mix <= 0.0 || (!use_custom && !any_proc_drops && p.bg_blur <= 0.001) {
        return Ok(());
    }

    let custom_fld = if use_custom {
        build_custom_field(layers, w, h, scale_f, time, &p)
    } else {
        None
    };

    if custom_fld.is_none() && !any_proc_drops && p.bg_blur <= 0.001 {
        return Ok(());
    }

    let proc_fld = if any_proc_drops {
        Some(build_rain_field(w, h, &p, t))
    } else {
        None
    };

    let src = px.to_vec();
    let min_dim = (w as f32).min(h as f32);

    let blur_buf = if p.bg_blur > 0.001 {
        let sigma = p.bg_blur * 0.022 * min_dim * 0.35;
        Some(blur::gauss_rgba_downsampled(&src, w, h, sigma))
    } else {
        None
    };

    let wi = w as i32;
    let hi = h as i32;
    let dmax = min_dim * 0.22;
    let ca = p.dispersion * 0.16;
    let three_phase = ca >= 0.03;
    let two_phase = ca > 0.004 && !three_phase;
    let chroma_method = if p.sampling == 1 { 0 } else { p.sampling };

    let la = p.light_angle * std::f32::consts::PI / 180.0;
    let lx = la.cos();
    let ly = -la.sin();
    let lz = 0.65f32;
    let inv_ll = inv_sqrt(lx * lx + ly * ly + lz * lz);
    let lnx = lx * inv_ll;
    let lny = ly * inv_ll;
    let lnz = lz * inv_ll;
    let inv_hl = inv_sqrt(lnx * lnx + lny * lny + (lnz + 1.0) * (lnz + 1.0));
    let hx = lnx * inv_hl;
    let hy = lny * inv_hl;
    let hz = (lnz + 1.0) * inv_hl;
    let inv_span_t = 1.0 / (1.0 - CONTACT_T);

    let src_ref = &src;
    let blur_ref = blur_buf.as_deref();
    let proc_ref = proc_fld.as_ref();
    let custom_ref = custom_fld.as_ref();
    let params_ref = &p;

    let shade_rows = |start_y: usize, slice: &mut [f32]| {
        let rows = slice.len() / (w * 4);
        for ry in 0..rows {
            let y = start_y + ry;
            let row = y * w;
            let yf = y as f32;
            let out_row = ry * w * 4;

            for x in 0..w {
                let idx = row + x;
                let o = out_row + x * 4;
                let so = idx * 4;
                let orr = src_ref[so];
                let org = src_ref[so + 1];
                let orb = src_ref[so + 2];
                let ora = src_ref[so + 3];

                let mut ds_p = DropSample::ZERO;
                let mut wipe_p = 0.0f32;
                if let Some(pf) = proc_ref {
                    wipe_p = pf.wbuf[idx];
                    let f = pf.fbuf[idx];
                    if f > 0.08 {
                        let gx = pf.gxb[idx];
                        let gy = pf.gyb[idx];
                        let g_mag = (gx * gx + gy * gy).sqrt();
                        let dist_px = (f - CONTACT_T) / g_mag.max(1.0e-4);
                        let alpha_p = smoothstep01((dist_px + 0.65) / 1.30);
                        if alpha_p > 0.001 {
                            let inv_f = 1.0 / f;
                            let u_in = clamp01((f - CONTACT_T) * inv_span_t);
                            let q_norm_p = (1.0 - u_in).max(0.0).sqrt();
                            let inv_g = if g_mag > 1.0e-5 { 1.0 / g_mag } else { 0.0 };
                            ds_p = DropSample {
                                alpha: alpha_p,
                                q_norm: q_norm_p,
                                rim_band: smoothstep01(1.0 - u_in * 2.8),
                                ex: -gx * inv_g,
                                ey: -gy * inv_g,
                                spec_aa: 1.0 - 0.72 * smoothstep01((g_mag - 0.38) * 1.4),
                                dx: pf.dxb[idx] * inv_f,
                                dy: pf.dyb[idx] * inv_f,
                            };
                        }
                    }
                }

                let mut ds_c = DropSample::ZERO;
                if let Some(cf) = custom_ref {
                    let be = cf.b_edge[idx];
                    if be > 0.20 {
                        let alpha_c = smoothstep01((be - 0.22) / 0.56);
                        if alpha_c > 0.001 {
                            let (hxg, hyg) = sobel_grad(&cf.h_dome, w, h, x, y);
                            let g_scale = cf.bevel_px * 0.92;
                            let sx = hxg * g_scale;
                            let sy = hyg * g_scale;
                            let s_mag = (sx * sx + sy * sy).sqrt();
                            let s_sat = s_mag / (1.0 + 0.25 * s_mag);
                            let (ix, iy) = if s_mag > 1.0e-5 {
                                (sx / s_mag, sy / s_mag)
                            } else {
                                (0.0, 0.0)
                            };
                            let bend = 1.0 + 0.40 * s_sat * s_sat;
                            let disp_gain = cf.bevel_px * 1.45 * cf.disp_scale * bend;
                            let spec_aa_c = 1.0 - 0.55 * smoothstep01((3.0 - cf.bevel_px) * 0.5);
                            ds_c = DropSample {
                                alpha: alpha_c,
                                q_norm: clamp01(s_sat * 1.05),
                                rim_band: smoothstep01((s_sat - 0.18) * 1.45),
                                ex: -ix,
                                ey: -iy,
                                spec_aa: spec_aa_c,
                                dx: ix * s_sat * disp_gain,
                                dy: iy * s_sat * disp_gain,
                            };
                        }
                    }
                }

                let combined = if ds_p.alpha > 0.001 && ds_c.alpha > 0.001 {
                    let alpha = clamp01(1.0 - (1.0 - ds_p.alpha) * (1.0 - ds_c.alpha));
                    let mx = ds_p.ex * ds_p.q_norm * ds_p.alpha + ds_c.ex * ds_c.q_norm * ds_c.alpha;
                    let my = ds_p.ey * ds_p.q_norm * ds_p.alpha + ds_c.ey * ds_c.q_norm * ds_c.alpha;
                    let m_mag = (mx * mx + my * my).sqrt();
                    let q_norm = clamp01(m_mag / (1.0 + 0.20 * m_mag));
                    let (ex, ey) = if m_mag > 1.0e-5 {
                        (mx / m_mag, my / m_mag)
                    } else {
                        (0.0, 0.0)
                    };
                    let raw_rim = (ds_p.rim_band * ds_p.alpha).max(ds_c.rim_band * ds_c.alpha);
                    let rim_band = raw_rim * smoothstep01(q_norm * 1.8);
                    let w_sp = ds_p.q_norm * ds_p.alpha + 1.0e-4;
                    let w_sc = ds_c.q_norm * ds_c.alpha + 1.0e-4;
                    let spec_aa = (ds_p.spec_aa * w_sp + ds_c.spec_aa * w_sc) / (w_sp + w_sc);
                    DropSample {
                        alpha,
                        q_norm,
                        rim_band,
                        ex,
                        ey,
                        spec_aa,
                        dx: ds_p.dx * ds_p.alpha + ds_c.dx * ds_c.alpha,
                        dy: ds_p.dy * ds_p.alpha + ds_c.dy * ds_c.alpha,
                    }
                } else if ds_c.alpha > 0.001 {
                    ds_c
                } else {
                    ds_p
                };

                let (mut r, mut g, mut b, mut a);
                let wipe = wipe_p.max(ds_c.alpha);

                if combined.alpha <= 0.001 {
                    if let Some(bbuf) = blur_ref {
                        let fog = params_ref.bg_blur * (1.0 - wipe * 0.88);
                        r = orr + (bbuf[so] - orr) * fog;
                        g = org + (bbuf[so + 1] - org) * fog;
                        b = orb + (bbuf[so + 2] - orb) * fog;
                        a = ora + (bbuf[so + 3] - ora) * fog;
                    } else {
                        continue;
                    }
                } else {
                    let dx = (combined.dx * params_ref.master_disp).clamp(-dmax, dmax);
                    let dy = (combined.dy * params_ref.master_disp).clamp(-dmax, dmax);
                    let xf = x as f32;
                    let fxg = xf + dx;
                    let fyg = yf + dy;

                    if three_phase {
                        let sr = sample(
                            src_ref,
                            wi,
                            hi,
                            params_ref.edge_mode,
                            chroma_method,
                            xf + dx * (1.0 - ca),
                            yf + dy * (1.0 - ca),
                        );
                        let sg = sample(src_ref, wi, hi, params_ref.edge_mode, params_ref.sampling, fxg, fyg);
                        let sb = sample(
                            src_ref,
                            wi,
                            hi,
                            params_ref.edge_mode,
                            chroma_method,
                            xf + dx * (1.0 + ca),
                            yf + dy * (1.0 + ca),
                        );
                        r = sr.r;
                        g = sg.g;
                        b = sb.b;
                        a = sg.a;
                    } else if two_phase {
                        let sr = sample(
                            src_ref,
                            wi,
                            hi,
                            params_ref.edge_mode,
                            chroma_method,
                            xf + dx * (1.0 - ca),
                            yf + dy * (1.0 - ca),
                        );
                        let sb = sample(
                            src_ref,
                            wi,
                            hi,
                            params_ref.edge_mode,
                            chroma_method,
                            xf + dx * (1.0 + ca),
                            yf + dy * (1.0 + ca),
                        );
                        r = sr.r;
                        g = (sr.g + sb.g) * 0.5;
                        b = sb.b;
                        a = (sr.a + sb.a) * 0.5;
                    } else {
                        let s = sample(src_ref, wi, hi, params_ref.edge_mode, params_ref.sampling, fxg, fyg);
                        r = s.r;
                        g = s.g;
                        b = s.b;
                        a = s.a;
                    }

                    if params_ref.hi_k > 0.0 {
                        let slope = 1.45 * combined.q_norm;
                        let nx = combined.ex * slope;
                        let ny = combined.ey * slope;
                        let invn = inv_sqrt(nx * nx + ny * ny + 1.0);

                        let lit_dot = combined.ex * lnx + combined.ey * lny;
                        let opp = clamp01(-lit_dot);
                        let dark_side = 0.55 + 0.45 * opp;
                        let dark_rim =
                            combined.rim_band * dark_side * (0.40 * params_ref.hi_k.min(1.4)).min(0.46);
                        r *= 1.0 - dark_rim;
                        g *= 1.0 - dark_rim;
                        b *= 1.0 - dark_rim;

                        let lum = clamp01(r * 0.299 + g * 0.587 + b * 0.114);
                        let caustic =
                            combined.rim_band * opp * opp * 0.28 * params_ref.hi_k * (0.40 + 0.60 * lum);

                        let ndh = (nx * hx + ny * hy + hz) * invn;
                        let dir = clamp01(ndh);
                        let d2 = dir * dir;
                        let p8 = d2 * d2 * d2 * d2;
                        let p12 = p8 * d2 * d2;
                        let p16 = p8 * p8;
                        let p32 = p16 * p16;
                        let curve_gate = smoothstep01((combined.q_norm - 0.08) * 3.0);
                        let spec = (p32 * 0.72 * combined.spec_aa + p12 * 0.06)
                            * curve_gate
                            * params_ref.hi_k
                            * (1.0 - 0.45 * lum);
                        let fres_lit = combined.rim_band
                            * clamp01(lit_dot)
                            * 0.20
                            * params_ref.hi_k
                            * (1.0 - 0.40 * lum);

                        let add = (spec + caustic + fres_lit) * a;
                        r += add * 0.98;
                        g += add * 0.99;
                        b += add;
                    }

                    if combined.alpha < 1.0 {
                        let (mut br, mut bg_g, mut bb, mut ba) = (orr, org, orb, ora);
                        if let Some(bbuf) = blur_ref {
                            let fog = params_ref.bg_blur * (1.0 - wipe * 0.88);
                            br += (bbuf[so] - br) * fog;
                            bg_g += (bbuf[so + 1] - bg_g) * fog;
                            bb += (bbuf[so + 2] - bb) * fog;
                            ba += (bbuf[so + 3] - ba) * fog;
                        }
                        r = br + (r - br) * combined.alpha;
                        g = bg_g + (g - bg_g) * combined.alpha;
                        b = bb + (b - bb) * combined.alpha;
                        a = ba + (a - ba) * combined.alpha;
                    }
                }

                if params_ref.mix < 1.0 {
                    r = orr + (r - orr) * params_ref.mix;
                    g = org + (g - org) * params_ref.mix;
                    b = orb + (b - orb) * params_ref.mix;
                    a = ora + (a - ora) * params_ref.mix;
                }

                if a >= 0.0 {
                    r = r.clamp(0.0, a);
                    g = g.clamp(0.0, a);
                    b = b.clamp(0.0, a);
                } else {
                    r = r.max(0.0);
                    g = g.max(0.0);
                    b = b.max(0.0);
                    a = 0.0;
                }

                slice[o] = r;
                slice[o + 1] = g;
                slice[o + 2] = b;
                slice[o + 3] = a;
            }
        }
    };

    let threads = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(h)
        .max(1);
    let total_px_floats = w * h * 4;
    if threads <= 1 || h < 64 {
        shade_rows(0, &mut px[..total_px_floats]);
    } else {
        let rows_per = h.div_ceil(threads);
        thread::scope(|scope| {
            for (ci, chunk) in px[..total_px_floats].chunks_mut(rows_per * w * 4).enumerate() {
                let start_y = ci * rows_per;
                scope.spawn(move || shade_rows(start_y, chunk));
            }
        });
    }

    Ok(())
}
