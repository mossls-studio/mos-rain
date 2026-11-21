/// 一滴水：有限斜率的抛物面穹顶，边缘保留斜率，才有暗轮廓和锐高光。
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
fn stream_x(y: f32, x0: f32, phase: f32, amp: f32) -> f32 {
    x0 + amp * (0.62 * fsin(y * 0.017 + phase) + 0.38 * fsin(y * 0.043 + phase * 1.7))
}

#[inline]
fn stream_w(y: f32, width: f32, phase: f32, drift: f32) -> f32 {
    let yy = y + drift;
    let body = 0.55 + 0.45 * fsin(yy * 0.023 + phase * 2.3) * fsin(yy * 0.023 + phase * 2.3);
    let slow = 0.78 + 0.22 * fsin(yy * 0.0065 + phase * 0.9);
    maxf(width * body * slow, width * 0.22)
}

/// 一条水流：半圆截面（sqrt(1-q²)），宽度/高度沿程起伏。
/// 比穹顶陡，边缘会产生清晰的暗轮廓 + 亮边，像真正的水流而不是软带。
unsafe fn stream(
    hb: *mut f32,
    sb: *mut f32,
    gw: usize,
    gh: usize,
    x0: f32,
    phase: f32,
    amp: f32,
    width: f32,
    height_scale: f32,
    y_top: f32,
    y_bottom: f32,
    drift: f32,
) {
    let y0 = maxf(0.0, floorf(y_top)) as usize;
    let y1 = minf((gh - 1) as f32, ceilf(y_bottom)) as usize;
    let mut py = y0;
    while py <= y1 {
        let yf = py as f32;
        let xc = stream_x(yf, x0, phase, amp);
        let w = stream_w(yf, width, phase, drift);
        let hgt = w * height_scale * (0.82 + 0.18 * fsin(yf * 0.019 + phase * 1.4));
        let ia = 1.0 / (w * w);
        let x0i = maxf(0.0, floorf(xc - w)) as usize;
        let x1i = minf((gw - 1) as f32, ceilf(xc + w)) as usize;
        let row = py * gw;
        let mut px = x0i;
        while px <= x1i {
            let dx = px as f32 - xc;
            let q2 = dx * dx * ia;
            if q2 < 1.0 {
                let v = sqrtf(1.0 - q2) * hgt;
                let slot = hb.add(row + px);
                let old = *slot;
                *slot = old + v - old * v;
                let sp = sb.add(row + px);
                if w * 1.5 > *sp {
                    *sp = w * 1.5;
                }
            }
            px += 1;
        }
        py += 1;
    }
}

/// 生成雨滴高度场。三层：细雾、抖动网格上的水珠、贯穿画面的长水流。
/// 水流上每隔一段长一颗大水珠，screen 叠加后形成参考图那种粗细不均的水痕。
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

    let cell = maxf(8.0, min_dim * 0.058);
    let rad_base = 0.20 + 0.55 * size01;
    let rad_nom = cell * rad_base * 0.82;
    let drift = time * speed * 0.6;

    // ---- 1. 细密水雾
    let cell_m = maxf(3.0, cell * 0.18);
    let inv_cm = 1.0 / cell_m;
    let gwm = ceilf(wf * inv_cm) as i32 + 1;
    let ghm = ceilf(hf * inv_cm) as i32 + 1;
    let mut gy = -1;
    while gy <= ghm {
        let mut gx = -1;
        while gx <= gwm {
            let seed = (gx as u32)
                .wrapping_mul(0x9e37_79b9)
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
            let cx = (gx as f32 + 0.5 + (unit(h1) - 0.5) * 0.95) * cell_m;
            let cy = (gy as f32 + 0.5 + (unit(h2) - 0.5) * 0.95) * cell_m;
            let r = cell_m * (0.09 + 0.26 * unit(h3));
            let vh = 0.45 + 0.35 * unit(hash_u32(h3 ^ 0x1f83_d9ab));
            splat(hb, sb, gw, gh, cx, cy, r, r, vh, r);
            gx += 1;
        }
        gy += 1;
    }

    // ---- 2. 中号水珠：抖动网格 + 粗网格留白
    let inv_cell = 1.0 / cell;
    let gwi = ceilf(wf * inv_cell) as i32 + 1;
    let ghi = ceilf(hf * inv_cell) as i32 + 1;
    let raw_thresh = 0.10 + 0.90 * density01;
    gy = -1;
    while gy <= ghi {
        let mut gx = -1;
        while gx <= gwi {
            let seed = (gx as u32)
                .wrapping_mul(0x9e37_79b9)
                ^ (gy as u32).wrapping_mul(0x85eb_ca6b);
            let h0 = hash_u32(seed);
            let cgx = gx >> 2;
            let cgy = gy >> 2;
            let cm = unit(hash_u32(
                (cgx as u32).wrapping_mul(0x27d4_eb2d)
                    ^ (cgy as u32).wrapping_mul(0x1656_67b1),
            ));
            if unit(h0) > raw_thresh || cm > 0.30 + 0.70 * density01 {
                gx += 1;
                continue;
            }
            let h1 = hash_u32(h0 ^ 0xa5a5_a5a5);
            let h2 = hash_u32(h1.wrapping_add(0x1234_5677));
            let h3 = hash_u32(h2 ^ 0x68e3_1da4);
            let h4 = hash_u32(h3.wrapping_add(0x7f4a_7c15));
            let jx = (unit(h1) - 0.5) * 1.05;
            let jy = (unit(h2) - 0.5) * 1.05;
            let cx = (gx as f32 + 0.5 + jx) * cell;
            let cy = (gy as f32 + 0.5 + jy) * cell;
            let r = rad_nom * (0.38 + 0.95 * unit(h3)) * (0.72 + 0.55 * cm);
            let teardrop = unit(h4) < 0.25;
            let ra = r * (0.92 + 0.16 * unit(h1));
            let rb = if teardrop {
                r * (1.22 + 0.22 * unit(h2))
            } else {
                r * (0.92 + 0.16 * unit(h2))
            };
            let vh = 0.90 + 0.10 * unit(hash_u32(h4 ^ 0x2545_f491));
            splat(hb, sb, gw, gh, cx, cy, ra, rb, vh, maxf(ra, rb));
            gx += 1;
        }
        gy += 1;
    }

    // ---- 3. 长水流
    let count = (2.0 + streak01 * 14.0) as i32;
    let mut s = 0;
    while s < count {
        let hs = hash_u32(0x51ed_2701u32.wrapping_add((s as u32).wrapping_mul(0x9e37_79b9)));
        let hs2 = hash_u32(hs ^ 0xbb67_ae85);
        let hs3 = hash_u32(hs2.wrapping_add(0x3c6e_f372));
        let hs4 = hash_u32(hs3 ^ 0x510e_527f);
        let hs5 = hash_u32(hs4.wrapping_add(0x1f83_d9ab));

        let x0 = unit(hs) * wf;
        let phase = unit(hs2) * TAU;
        let amp = rad_nom * (0.5 + 1.4 * unit(hs3));
        let width = rad_nom * (0.50 + 0.55 * unit(hs4));
        let hscale = 0.85 + 0.25 * unit(hs5);
        let len = hf * (0.40 + 0.60 * trail01);
        let top = -len * 0.18 + unit(hs2) * hf * 0.55;
        let bottom = top + len;

        stream(
            hb, sb, gw, gh, x0, phase, amp, width, hscale, top, bottom, drift,
        );

        // 水流上的节点水珠：沿程每隔一段长一颗，和水流融成粗细不均的水痕。
        let mut y = top + 12.0 + unit(hs4) * 40.0;
        let step = 42.0 + unit(hs5) * 55.0;
        let mut k = 0;
        while y < bottom && k < 24 {
            let hk = hash_u32(hs3.wrapping_add((k as u32).wrapping_mul(0x27d4_eb2d)));
            if unit(hk) > 0.25 {
                let x = stream_x(y, x0, phase, amp);
                let w = stream_w(y - 0.0, width, phase, drift);
                let r = w * (1.15 + 0.75 * unit(hash_u32(hk ^ 0x68e3_1da4)));
                let vh = 0.95 + 0.05 * unit(hash_u32(hk ^ 0x1f83_d9ab));
                splat(hb, sb, gw, gh, x, y, r * 0.95, r * 1.12, vh, r);
            }
            y += step;
            k += 1;
        }

        // 分叉：少数水流在中途分出一条更细的支流，形成参考图的 Y 形水网。
        if unit(hs5) < 0.55 && len > 80.0 {
            let by = top + len * (0.25 + 0.35 * unit(hs3));
            let bx = stream_x(by, x0, phase, amp);
            let bdir = if unit(hs4) < 0.5 { -1.0 } else { 1.0 };
            let bw = width * 0.55;
            let blen = len * (0.18 + 0.22 * unit(hs2));
            let mut yy = by;
            while yy < by + blen {
                let t = (yy - by) / blen;
                let xc = bx + bdir * (yy - by) * 0.32 + bw * 0.7 * fsin((yy - by) * 0.06 + phase);
                let w = bw * (1.0 - 0.65 * t);
                let hgt = w * hscale * 0.9;
                let ia = 1.0 / (w * w);
                let x0i = maxf(0.0, floorf(xc - w)) as usize;
                let x1i = minf((gw - 1) as f32, ceilf(xc + w)) as usize;
                let row = yy as usize * gw;
                let mut px = x0i;
                while px <= x1i {
                    let dx = px as f32 - xc;
                    let q2 = dx * dx * ia;
                    if q2 < 1.0 {
                        let v = sqrtf(1.0 - q2) * hgt;
                        let slot = hb.add(row + px);
                        let old = *slot;
                        *slot = old + v - old * v;
                        let sp = sb.add(row + px);
                        if w * 1.4 > *sp {
                            *sp = w * 1.4;
                        }
                    }
                    px += 1;
                }
                yy += 1.0;
            }
        }
        s += 1;
    }

    // ---- 4. 几颗孤立的大水珠
    let big = 2 + (unit(hash_u32(0x9e37_79b9)) * 4.0) as i32;
    let mut k = 0;
    while k < big {
        let hk = hash_u32(0x7f4a_7c15u32.wrapping_add((k as u32).wrapping_mul(0x85eb_ca6b)));
        let hk2 = hash_u32(hk ^ 0x2545_f491);
        let x = unit(hk) * wf;
        let y = unit(hk2) * hf;
        let r = rad_nom * (1.5 + 1.2 * unit(hash_u32(hk2 ^ 0x68e3_1da4)));
        splat(hb, sb, gw, gh, x, y, r * 0.92, r * 1.18, 1.0, r);
        k += 1;
    }

    (0.30 * rad_nom * rad_nom, 0.60 * rad_nom, rad_nom)
}
