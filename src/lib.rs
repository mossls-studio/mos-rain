// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 MOSSLS
//
// mos_rain —— 真实雨天玻璃窗（v4.0.0 从零重写）：
//   1. 全分辨率解析元球势场 F + 精确解析梯度 ∇F + 势场加权光学折射位移场；
//   2. 重力下垂梨形与代数角谐波微扰的有机雨滴，真实凸透镜倒像折射与三通道色散；
//   3. 大雨滴、小雨滴、长条雨滴分项独立控制（密度 / 大小 / 相位 / 置换强度）；
//   4. 粘滑重力持续下落、固定玻璃蜿蜒水痕与钉住残珠、自动擦净背景雾气与小水珠。

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

// ---------------------------------------------------------------- 数学与场结构

const CONTACT_T: f32 = 0.30;
const INV_QC: f32 = 1.487_048_8; // 1 / sqrt(1 - sqrt(0.30))，保证 F == CONTACT_T 恰好落在视觉半径上
const MAX_STREAKS: usize = 28;

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

/// 严格单调的周期性粘滑重力函数：输入相位 ph ∈ [0, 1)，返回粘滑推进位置 u ∈ [0, 1)。
#[inline]
fn stick_slip(ph: f32, wobble_off: f32) -> f32 {
    let tau = ph * TAU + wobble_off;
    let delta =
        0.32 * (fsin(tau) - fsin(wobble_off)) + 0.05 * (fsin(2.0 * tau) - fsin(2.0 * wobble_off));
    ph + delta * (1.0 / TAU)
}

/// 固定在玻璃坐标 y 上的 3 频非公度蜿蜒路径：返回 (x, dx/dy)。
#[inline]
fn streak_path(y: f32, x0: f32, p1: f32, p2: f32, p3: f32, amp: f32) -> (f32, f32) {
    let f1 = 0.010;
    let f2 = 0.028;
    let f3 = 0.068;
    let a1 = y * f1 + p1;
    let a2 = y * f2 + p2;
    let a3 = y * f3 + p3;
    let x = x0 + amp * (0.55 * fsin(a1) + 0.30 * fsin(a2) + 0.15 * fsin(a3));
    let slope = amp * (0.55 * f1 * fcos(a1) + 0.30 * f2 * fcos(a2) + 0.15 * f3 * fcos(a3));
    (x, slope)
}

struct RainField {
    w: usize,
    h: usize,
    fbuf: *mut f32,
    gxb: *mut f32,
    gyb: *mut f32,
    dxb: *mut f32,
    dyb: *mut f32,
    wbuf: *mut f32,
}

/// 有机重力形变水滴：
/// - 重力下垂梨形（上尖下坠）+ 2/3 次代数角谐波不规则轮廓（零额外三角函数）；
/// - 向全分辨率元球场写入势场 F、精确解析梯度 ∇F，以及凸透镜倒像折射位移。
#[inline]
#[allow(clippy::too_many_arguments)]
unsafe fn splat_organic_drop(
    fld: &RainField,
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
    let x0f = floorf(cx - pad_x);
    let x1f = ceilf(cx + pad_x);
    let y0f = floorf(cy - pad_y);
    let y1f = ceilf(cy + pad_y);
    if x1f < 0.0 || y1f < 0.0 || x0f > wf1 || y0f > hf1 {
        return;
    }

    let x0 = maxf(0.0, x0f) as usize;
    let x1 = minf(wf1, x1f) as usize;
    let y0 = maxf(0.0, y0f) as usize;
    let y1 = minf(hf1, y1f) as usize;

    let inv_ra_s = 1.0 / ra_s;
    let inv_rb_s = 1.0 / rb_s;
    let belly_shift = 0.18 * sag;
    let disp_rx = ra_vis * 2.30 * type_disp;
    let disp_ry = rb_vis * 2.30 * type_disp;
    let gw = fld.w;

    let mut py = y0;
    while py <= y1 {
        let dy_px = py as f32 + 0.5 - cy;
        let v = dy_px * inv_rb_s;
        let v_c = clampf(v, -1.0, 1.0);
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

        let mut px = x0;
        while px <= x1 {
            let dx_px = px as f32 + 0.5 - cx;
            let u = dx_px * inv_ra_s;
            let u_eff = u * inv_ws;
            let r2 = u_eff * u_eff + vb2;
            if r2 >= 1.21 {
                px += 1;
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
                *fld.fbuf.add(idx) += f;

                let k_g = -4.0 * weight * t / (rmod * rmod);
                let df_due = k_g * u_eff;
                let df_dvb = k_g * v_belly;
                let df_dx = df_due * inv_ra_s * inv_ws;
                let df_dy = df_dvb * inv_rb_s - df_due * u_eff * sag_y_k;
                *fld.gxb.add(idx) += df_dx;
                *fld.gyb.add(idx) += df_dy;

                // 真实凸透镜倒像折射：折射系数 > 1 且向边缘弯折，产生清晰的倒立背景缩影。
                let q_vis2 = minf(1.0, q2 * 2.22);
                let bend = 1.0 + 0.85 * q_vis2;
                let inv_rm = 1.0 / rmod;
                let disp_x = -(u_eff * inv_rm) * disp_rx * bend;
                let disp_y = -(v_belly * inv_rm) * disp_ry * bend;
                *fld.dxb.add(idx) += disp_x * f;
                *fld.dyb.add(idx) += disp_y * f;
            }
            px += 1;
        }
        py += 1;
    }
}

/// 写入长条路径上的擦除带（清除背景雾气并带走沿途小水珠）。
#[inline]
unsafe fn paint_wipe(fld: &RainField, cx: f32, py: usize, wipe_r: f32, strength: f32) {
    if wipe_r < 0.5 || strength <= 0.0 {
        return;
    }
    let wf1 = (fld.w - 1) as f32;
    let x0f = floorf(cx - wipe_r);
    let x1f = ceilf(cx + wipe_r);
    if x1f < 0.0 || x0f > wf1 {
        return;
    }
    let x0 = maxf(0.0, x0f) as usize;
    let x1 = minf(wf1, x1f) as usize;
    let inv_wr = 1.0 / wipe_r;
    let row = py * fld.w;
    let mut px = x0;
    while px <= x1 {
        let d = fabsf(px as f32 + 0.5 - cx) * inv_wr;
        if d < 1.0 {
            let wp = smoothstep01(1.0 - d) * strength;
            let slot = fld.wbuf.add(row + px);
            if wp > *slot {
                *slot = wp;
            }
        }
        px += 1;
    }
}

// ---------------------------------------------------------------- 三层雨滴场构建

#[allow(clippy::too_many_arguments)]
unsafe fn build_rain_field(
    fld: &RainField,
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
    seed: f32,
    time: f32,
) {
    let w = fld.w;
    let h = fld.h;
    let n = w * h;
    let mut i = 0;
    while i < n {
        *fld.fbuf.add(i) = 0.0;
        *fld.gxb.add(i) = 0.0;
        *fld.gyb.add(i) = 0.0;
        *fld.dxb.add(i) = 0.0;
        *fld.dyb.add(i) = 0.0;
        *fld.wbuf.add(i) = 0.0;
        i += 1;
    }

    let wf = w as f32;
    let hf = h as f32;
    let min_dim = minf(wf, hf);
    let salt = (seed * 1000.0) as u32;
    let speed_k = flow_speed * 0.01;

    // -------------------------------- 层 1：长条雨滴（Streaks）
    if streak_density > 0.001 {
        let count = minf(
            (1.0 + floorf(streak_density * 25.0)) as f32,
            MAX_STREAKS as f32,
        ) as usize;
        let sz_s = 0.35 + 1.35 * (streak_size * 0.01);
        let t_streak = time * speed_k * 0.90 + streak_phase * 0.25;

        let mut s = 0;
        while s < count {
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
            let head_r = maxf(2.6, min_dim * (0.010 + 0.0095 * unit(h3)) * sz_s);
            let trail_len = hf * (0.28 + 0.45 * unit(h4)) * (0.65 + 0.35 * (streak_size * 0.01));
            let cycle = 4.2 + 3.8 * unit(h5);
            let ph = fract01(unit(h6) + t_streak / cycle);
            let u_fall = stick_slip(ph, unit(h1) * TAU);
            let margin = hf * 0.10 + head_r * 2.5;
            let y_head = -margin + u_fall * (hf + trail_len + 2.0 * margin);

            // 1.1 蜿蜒细水痕（逐行绘制，垂直切线的精确解析梯度与圆柱折射）
            let y0 = maxf(0.0, floorf(y_head - trail_len)) as usize;
            let y1 = minf(hf - 1.0, ceilf(y_head)) as usize;
            let inv_tl = 1.0 / maxf(trail_len, 1.0);
            let wf1 = wf - 1.0;

            let mut py = y0;
            while py <= y1 {
                let y_center = py as f32 + 0.5;
                let d = y_head - y_center;
                if d >= 0.0 && d <= trail_len {
                    let t_tr = d * inv_tl;
                    let (xc, slope) = streak_path(y_center, x0, p1, p2, p3, amp);
                    let w_mod = 0.65 + 0.35 * fsin(y_center * 0.047 + p2);
                    let tail_fade = smoothstep01((1.0 - t_tr) * 5.0);
                    let w_vis =
                        head_r * (0.38 + 0.22 * (1.0 - t_tr)) * w_mod * (0.45 + 0.55 * tail_fade);
                    let weight = 0.72 + 0.28 * tail_fade;
                    let wipe_r = head_r * (1.45 + 0.25 * (1.0 - t_tr));
                    paint_wipe(fld, xc, py, wipe_r, 0.92 * tail_fade);

                    if w_vis >= 0.60 {
                        let norm_s = inv_sqrt(1.0 + slope * slope);
                        let nx_p = norm_s;
                        let ny_p = -slope * norm_s;
                        let w_horiz = w_vis / norm_s;
                        let w_s = w_horiz * INV_QC;
                        let inv_ws = 1.0 / w_s;

                        let x0f = floorf(xc - w_s);
                        let x1f = ceilf(xc + w_s);
                        if x1f >= 0.0 && x0f <= wf1 {
                            let x0i = maxf(0.0, x0f) as usize;
                            let x1i = minf(wf1, x1f) as usize;
                            let row = py * w;
                            let mut px = x0i;
                            while px <= x1i {
                                let dx_px = px as f32 + 0.5 - xc;
                                let u_perp = dx_px * inv_ws;
                                let q2 = u_perp * u_perp;
                                if q2 < 1.0 {
                                    let tt = 1.0 - q2;
                                    let f = tt * tt * weight;
                                    let idx = row + px;
                                    *fld.fbuf.add(idx) += f;

                                    let df_du = -4.0 * weight * tt * u_perp;
                                    *fld.gxb.add(idx) += df_du * inv_ws;
                                    *fld.gyb.add(idx) += -df_du * slope * inv_ws;

                                    let q_vis2 = minf(1.0, q2 * 2.22);
                                    let disp_p = -u_perp
                                        * w_vis
                                        * 2.70
                                        * (1.0 + 0.75 * q_vis2)
                                        * streak_disp;
                                    *fld.dxb.add(idx) += disp_p * nx_p * f;
                                    *fld.dyb.add(idx) += disp_p * ny_p * f;
                                }
                                px += 1;
                            }
                        }
                    }
                }
                py += 1;
            }

            // 1.2 领头大水滴（向下坠落并融入水痕前端）
            if y_head > -head_r * 3.0 && y_head < hf + head_r * 3.0 {
                let (hx, _) = streak_path(y_head, x0, p1, p2, p3, amp);
                let w2c = (unit(h2) - 0.5) * 0.08;
                let w2s = (unit(h3) - 0.5) * 0.08;
                let w3c = (unit(h4) - 0.5) * 0.06;
                let w3s = (unit(h5) - 0.5) * 0.06;
                splat_organic_drop(
                    fld,
                    hx,
                    y_head,
                    head_r * 0.88,
                    head_r * 1.38,
                    0.30,
                    w2c,
                    w2s,
                    w3c,
                    w3s,
                    1.0,
                    streak_disp,
                );
            }

            // 1.3 钉在玻璃上的尾迹残珠（按固定玻璃 y 步长采样，不跟头硬平移）
            let step = maxf(26.0, head_r * 4.8);
            let y_lo = maxf(-step, y_head - trail_len * 1.15);
            let y_hi = minf(hf + step, y_head - head_r * 1.8);
            if y_hi > y_lo {
                let b0 = floorf(y_lo / step) as i32;
                let b1 = ceilf(y_hi / step) as i32;
                let mut bi = b0;
                while bi <= b1 {
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
                            let lag_t = clamp01(lag / maxf(trail_len * 1.15, 1.0));
                            let fade_b = 1.0 - smoothstep01((lag_t - 0.65) * 2.86) * 0.45;
                            let rb = head_r * (0.28 + 0.22 * unit(bh3)) * (1.0 - 0.50 * lag_t);
                            let (xb, _) = streak_path(y_b, x0, p1, p2, p3, amp);
                            splat_organic_drop(
                                fld,
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
                                streak_disp,
                            );
                        }
                    }
                    bi += 1;
                }
            }

            s += 1;
        }
    }

    // -------------------------------- 层 2：大雨滴（Large Drops）
    if big_density > 0.001 {
        let cell_big = maxf(24.0, min_dim * 0.088);
        let inv_cb = 1.0 / cell_big;
        let gwi = ceilf(wf * inv_cb) as i32 + 1;
        let ghi = ceilf(hf * inv_cb) as i32 + 1;
        let sz_b = 0.35 + 1.35 * (big_size * 0.01);
        let t_big = time * speed_k * 0.55 + big_phase * 0.25;

        let mut gy = -1;
        while gy <= ghi {
            let mut gx = -1;
            while gx <= gwi {
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
                if unit(h0) > big_density * 0.85 || cm > 0.35 + 0.65 * big_density {
                    gx += 1;
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
                        let cx = bx + drift_amp * fsin(cy * 0.045 + drift_p);

                        // 滑落擦净身后雾气与小水珠
                        let wy0 = maxf(0.0, floorf(by)) as usize;
                        let wy1 = minf(hf - 1.0, ceilf(cy)) as usize;
                        let mut wy = wy0;
                        while wy <= wy1 {
                            let wx = bx + drift_amp * fsin((wy as f32 + 0.5) * 0.045 + drift_p);
                            paint_wipe(fld, wx, wy, r_vis * 1.15, 0.85 * life_scale);
                            wy += 1;
                        }

                        // 沿滑落轨迹留 1 颗钉在玻璃上的小残珠
                        if u_slip > 0.35 {
                            let y_res = by + slide_dist * 0.28;
                            let x_res = bx + drift_amp * fsin(y_res * 0.045 + drift_p);
                            let r_res = r_vis * (0.28 + 0.12 * unit(h7)) * vanish;
                            splat_organic_drop(
                                fld,
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
                                big_disp,
                            );
                        }

                        let r_cur = r_vis * (0.65 + 0.35 * life_scale);
                        let sag = 0.24 + 0.10 * u_slip;
                        let aspect_y = 1.18 + 0.18 * unit(h7);
                        splat_organic_drop(
                            fld,
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
                            big_disp,
                        );
                    } else {
                        let r_cur = r_vis * (0.62 + 0.38 * life_scale);
                        let sag = 0.22 + 0.08 * ph;
                        let aspect_y = 1.12 + 0.14 * unit(h7);
                        splat_organic_drop(
                            fld,
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
                            big_disp,
                        );
                    }
                }
                gx += 1;
            }
            gy += 1;
        }
    }

    // -------------------------------- 层 3：小雨滴（Small Drops：中小水珠 + 微细冷凝珠）
    if small_density > 0.001 {
        let sz_m = 0.35 + 1.35 * (small_size * 0.01);
        let t_small = time * speed_k * 0.22 + small_phase * 0.25;

        // 3.1 中小水珠
        let cell_s1 = maxf(11.0, min_dim * 0.030);
        let inv_s1 = 1.0 / cell_s1;
        let gw1 = ceilf(wf * inv_s1) as i32 + 1;
        let gh1 = ceilf(hf * inv_s1) as i32 + 1;

        let mut gy = -1;
        while gy <= gh1 {
            let mut gx = -1;
            while gx <= gw1 {
                let h0 = hash_u32(
                    (gx as u32).wrapping_mul(0x9e37_79b9)
                        ^ (gy as u32).wrapping_mul(0x85eb_ca6b)
                        ^ 0x1f83_d9ab
                        ^ salt,
                );
                if unit(h0) > small_density * 0.80 {
                    gx += 1;
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
                    gx += 1;
                    continue;
                }

                let r_vis =
                    min_dim * (0.0034 + 0.0046 * unit(h3)) * sz_m * (0.65 + 0.35 * life_scale);
                if r_vis < 1.05 {
                    gx += 1;
                    continue;
                }
                let bx = (gx as f32 + 0.5 + (unit(h1) - 0.5) * 0.94) * cell_s1;
                let slip = if unit(h4) < 0.22 {
                    r_vis * (0.5 + 1.3 * unit(h5)) * stick_slip(ph, unit(h1) * TAU)
                } else {
                    0.0
                };
                let by = (gy as f32 + 0.5 + (unit(h2) - 0.5) * 0.94) * cell_s1 + slip;

                // 被滑落水流或大雨滴擦除
                if bx >= 0.0 && bx < wf && by >= 0.0 && by < hf {
                    let ix = minf(wf - 1.0, maxf(0.0, bx)) as usize;
                    let iy = minf(hf - 1.0, maxf(0.0, by)) as usize;
                    let idx = iy * w + ix;
                    if *fld.wbuf.add(idx) > 0.28 || *fld.fbuf.add(idx) > 0.24 {
                        gx += 1;
                        continue;
                    }
                }

                splat_organic_drop(
                    fld,
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
                    small_disp,
                );
                gx += 1;
            }
            gy += 1;
        }

        // 3.2 微细冷凝珠
        let cell_s2 = maxf(6.0, min_dim * 0.0155);
        let inv_s2 = 1.0 / cell_s2;
        let gw2 = ceilf(wf * inv_s2) as i32 + 1;
        let gh2 = ceilf(hf * inv_s2) as i32 + 1;

        gy = -1;
        while gy <= gh2 {
            let mut gx = -1;
            while gx <= gw2 {
                let h0 = hash_u32(
                    (gx as u32).wrapping_mul(0x27d4_eb2d)
                        ^ (gy as u32).wrapping_mul(0x1656_67b1)
                        ^ 0x6a09_e667
                        ^ salt,
                );
                if unit(h0) > small_density * 0.58 {
                    gx += 1;
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
                    gx += 1;
                    continue;
                }

                let r_vis =
                    min_dim * (0.0023 + 0.0025 * unit(h3)) * sz_m * (0.68 + 0.32 * life_scale);
                if r_vis < 1.05 {
                    gx += 1;
                    continue;
                }
                let bx = (gx as f32 + 0.5 + (unit(h1) - 0.5) * 0.92) * cell_s2;
                let by = (gy as f32 + 0.5 + (unit(h2) - 0.5) * 0.92) * cell_s2;

                if bx >= 0.0 && bx < wf && by >= 0.0 && by < hf {
                    let ix = minf(wf - 1.0, maxf(0.0, bx)) as usize;
                    let iy = minf(hf - 1.0, maxf(0.0, by)) as usize;
                    let idx = iy * w + ix;
                    if *fld.wbuf.add(idx) > 0.28 || *fld.fbuf.add(idx) > 0.24 {
                        gx += 1;
                        continue;
                    }
                }

                splat_organic_drop(
                    fld,
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
                    small_disp,
                );
                gx += 1;
            }
            gy += 1;
        }
    }
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

    let big_density = clamp01(p(0) * 0.01);
    let big_size = clampf(p(1), 10.0, 200.0);
    let big_phase = p(2); // 相位不截断，支持关键帧连续推进
    let big_disp = clampf(p(3), 0.0, 300.0) * 0.01;

    let small_density = clamp01(p(4) * 0.01);
    let small_size = clampf(p(5), 10.0, 200.0);
    let small_phase = p(6);
    let small_disp = clampf(p(7), 0.0, 300.0) * 0.01;

    let streak_density = clamp01(p(8) * 0.01);
    let streak_size = clampf(p(9), 10.0, 200.0);
    let streak_phase = p(10);
    let streak_disp = clampf(p(11), 0.0, 300.0) * 0.01;

    let flow_speed = clampf(p(12), 0.0, 300.0);
    let master_disp = clampf(p(13), 0.0, 300.0) * 0.01;
    let dispersion = clamp01(p(14) * 0.01);
    let hi_k = clampf(p(15), 0.0, 300.0) * 0.01;
    let light_angle = p(16);
    let bg_blur = clamp01(p(17) * 0.01);

    let seed = clampf(p(18), 0.0, 100.0);
    let sampling = p(19) as i32;
    let edge = p(20) as i32;
    let mix = clamp01(p(21) * 0.01);

    let t = if time.is_finite() {
        maxf(0.0, time as f32)
    } else {
        0.0
    };

    let any_drops = big_density > 0.001 || small_density > 0.001 || streak_density > 0.001;
    if mix <= 0.0 || (!any_drops && bg_blur <= 0.001) {
        return 0;
    }

    let wi = w as i32;
    let hi = h as i32;
    let n4 = n * 4;

    let fbuf = arena_alloc(n * 4) as *mut f32;
    let gxb = arena_alloc(n * 4) as *mut f32;
    let gyb = arena_alloc(n * 4) as *mut f32;
    let dxb = arena_alloc(n * 4) as *mut f32;
    let dyb = arena_alloc(n * 4) as *mut f32;
    let wbuf = arena_alloc(n * 4) as *mut f32;
    let src = arena_alloc(n * 16) as *mut f32;
    if fbuf.is_null()
        || gxb.is_null()
        || gyb.is_null()
        || dxb.is_null()
        || dyb.is_null()
        || wbuf.is_null()
        || src.is_null()
    {
        return 1;
    }

    let mut blur_buf: *mut f32 = core::ptr::null_mut();
    let mut bw = 0usize;
    let mut bh = 0usize;
    let mut pre: *mut f32 = core::ptr::null_mut();
    if bg_blur > 0.001 {
        bw = maxf(2.0, floorf(wf * 0.5)) as usize;
        bh = maxf(2.0, floorf(hf * 0.5)) as usize;
        let long = if w > h { w } else { h };
        pre = arena_alloc((long + 1) * 16) as *mut f32;
        blur_buf = arena_alloc(bw * bh * 16) as *mut f32;
        if pre.is_null() || blur_buf.is_null() {
            return 1;
        }
    }

    let fld = RainField {
        w,
        h,
        fbuf,
        gxb,
        gyb,
        dxb,
        dyb,
        wbuf,
    };

    unsafe {
        let mut i = 0;
        while i < n4 {
            *src.add(i) = *px.add(i);
            i += 1;
        }

        build_rain_field(
            &fld,
            big_density,
            big_size,
            big_phase,
            big_disp,
            small_density,
            small_size,
            small_phase,
            small_disp,
            streak_density,
            streak_size,
            streak_phase,
            streak_disp,
            flow_speed,
            seed,
            t,
        );

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
            let radius = bg_blur * 0.022 * min_dim * 0.5;
            blur_inplace(blur_buf, bw, bh, radius, pre);
        }

        let bwi = bw as i32;
        let bhi = bh as i32;
        let has_fog = !blur_buf.is_null();
        let dmax = min_dim * 0.22;
        let ca = dispersion * 0.16;
        let three_phase = ca >= 0.03;
        let two_phase = ca > 0.004 && !three_phase;
        let chroma_method = if sampling == 1 { 0 } else { sampling };

        let la = light_angle * PI / 180.0;
        let lx = fcos(la);
        let ly = -fsin(la);
        let lz = 0.65;
        let inv_ll = inv_sqrt(lx * lx + ly * ly + lz * lz);
        let lnx = lx * inv_ll;
        let lny = ly * inv_ll;
        let lnz = lz * inv_ll;
        let inv_hl = inv_sqrt(lnx * lnx + lny * lny + (lnz + 1.0) * (lnz + 1.0));
        let hx = lnx * inv_hl;
        let hy = lny * inv_hl;
        let hz = (lnz + 1.0) * inv_hl;
        let inv_span_t = 1.0 / (1.0 - CONTACT_T);

        let mut y = 0;
        while y < h {
            let row = y * w;
            let yf = y as f32;
            let mut x = 0;
            while x < w {
                let idx = row + x;
                let f = *fbuf.add(idx);
                let o = idx * 4;
                let orr = *px.add(o);
                let org = *px.add(o + 1);
                let orb = *px.add(o + 2);
                let ora = *px.add(o + 3);

                let (mut r, mut g, mut b, mut a);

                if f <= 0.08 {
                    if !has_fog {
                        x += 1;
                        continue;
                    }
                    let wipe = *wbuf.add(idx);
                    let fog = bg_blur * (1.0 - wipe * 0.88);
                    let soft = sample(blur_buf, bwi, bhi, edge, sampling, x as f32 * 0.5, yf * 0.5);
                    r = orr + (soft.r - orr) * fog;
                    g = org + (soft.g - org) * fog;
                    b = orb + (soft.b - orb) * fog;
                    a = ora + (soft.a - ora) * fog;
                } else {
                    let gx = *gxb.add(idx);
                    let gy = *gyb.add(idx);
                    let g_mag = sqrtf(gx * gx + gy * gy);
                    let dist_px = (f - CONTACT_T) / maxf(g_mag, 1.0e-4);
                    let alpha = smoothstep01((dist_px + 0.65) / 1.30);

                    if alpha <= 0.001 {
                        if !has_fog {
                            x += 1;
                            continue;
                        }
                        let wipe = *wbuf.add(idx);
                        let fog = bg_blur * (1.0 - wipe * 0.88);
                        let soft =
                            sample(blur_buf, bwi, bhi, edge, sampling, x as f32 * 0.5, yf * 0.5);
                        r = orr + (soft.r - orr) * fog;
                        g = org + (soft.g - org) * fog;
                        b = orb + (soft.b - orb) * fog;
                        a = ora + (soft.a - ora) * fog;
                    } else {
                        let inv_f = 1.0 / f;
                        let mut dx = *dxb.add(idx) * inv_f * master_disp;
                        let mut dy = *dyb.add(idx) * inv_f * master_disp;
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

                        let xf = x as f32;
                        let fxg = xf + dx;
                        let fyg = yf + dy;

                        if three_phase {
                            let sr = sample(
                                src,
                                wi,
                                hi,
                                edge,
                                chroma_method,
                                xf + dx * (1.0 - ca),
                                yf + dy * (1.0 - ca),
                            );
                            let sg = sample(src, wi, hi, edge, sampling, fxg, fyg);
                            let sb = sample(
                                src,
                                wi,
                                hi,
                                edge,
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
                                src,
                                wi,
                                hi,
                                edge,
                                chroma_method,
                                xf + dx * (1.0 - ca),
                                yf + dy * (1.0 - ca),
                            );
                            let sb = sample(
                                src,
                                wi,
                                hi,
                                edge,
                                chroma_method,
                                xf + dx * (1.0 + ca),
                                yf + dy * (1.0 + ca),
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

                        if hi_k > 0.0 {
                            let u_in = clamp01((f - CONTACT_T) * inv_span_t);
                            let q_norm = sqrtf(maxf(0.0, 1.0 - u_in));
                            let inv_g = if g_mag > 1.0e-5 { 1.0 / g_mag } else { 0.0 };
                            let ex = -gx * inv_g;
                            let ey = -gy * inv_g;
                            let slope = 1.45 * q_norm;
                            let nx = ex * slope;
                            let ny = ey * slope;
                            let invn = inv_sqrt(nx * nx + ny * ny + 1.0);

                            // 1. 纤细暗接触线（弯月面全反射，不把水珠涂成黑块）
                            let rim_band = smoothstep01(1.0 - u_in * 2.8);
                            let lit_dot = ex * lnx + ey * lny;
                            let dark_side = 0.55 + 0.45 * clamp01(-lit_dot);
                            let dark_rim =
                                rim_band * dark_side * minf(0.46, 0.40 * minf(hi_k, 1.4));
                            r *= 1.0 - dark_rim;
                            g *= 1.0 - dark_rim;
                            b *= 1.0 - dark_rim;

                            let lum = clamp01(r * 0.299 + g * 0.587 + b * 0.114);

                            // 2. 底部/背光侧内聚焦散亮弧
                            let opp = clamp01(-lit_dot);
                            let caustic = rim_band * opp * opp * 0.28 * hi_k * (0.40 + 0.60 * lum);

                            // 3. 锐利窗光镜面点高光（超小凝珠降噪）+ 迎光弯月面细微亮线
                            let ndh = (nx * hx + ny * hy + hz) * invn;
                            let dir = clamp01(ndh);
                            let d2 = dir * dir;
                            let p8 = d2 * d2 * d2 * d2;
                            let p12 = p8 * d2 * d2;
                            let p16 = p8 * p8;
                            let p32 = p16 * p16;
                            let curve_gate = smoothstep01((q_norm - 0.08) * 3.0);
                            let spec_aa = 1.0 - 0.72 * smoothstep01((g_mag - 0.38) * 1.4);
                            let spec = (p32 * 0.72 * spec_aa + p12 * 0.06)
                                * curve_gate
                                * hi_k
                                * (1.0 - 0.45 * lum);
                            let fres_lit =
                                rim_band * clamp01(lit_dot) * 0.20 * hi_k * (1.0 - 0.40 * lum);

                            let add = (spec + caustic + fres_lit) * a;
                            r += add * 0.98;
                            g += add * 0.99;
                            b += add;
                        }

                        if alpha < 1.0 {
                            let (mut br, mut bg_g, mut bb, mut ba) = (orr, org, orb, ora);
                            if has_fog {
                                let wipe = *wbuf.add(idx);
                                let fog = bg_blur * (1.0 - wipe * 0.88);
                                let soft =
                                    sample(blur_buf, bwi, bhi, edge, sampling, xf * 0.5, yf * 0.5);
                                br += (soft.r - br) * fog;
                                bg_g += (soft.g - bg_g) * fog;
                                bb += (soft.b - bb) * fog;
                                ba += (soft.a - ba) * fog;
                            }
                            r = br + (r - br) * alpha;
                            g = bg_g + (g - bg_g) * alpha;
                            b = bb + (b - bb) * alpha;
                            a = ba + (a - ba) * alpha;
                        }
                    }
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
