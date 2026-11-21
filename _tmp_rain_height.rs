/// 一滴水：有限斜率的抛物面穹顶。
///
/// 边缘保留斜率，才会出现真实水珠那种“暗轮廓 + 亮高光”的硬边；
/// 之前把边缘斜率收到 0，珠子就成了一团软泡泡。screen 叠加让重叠的水
/// 融成水洼；`sr` 存这颗水的局部尺度，渲染时按它缩放折射量。
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
fn stream_x(y: f32, x0: f32, phase: f32, amp: f32, drift: f32) -> f32 {
    let yy = y + drift;
    x0 + amp * (0.62 * fsin(yy * 0.021 + phase) + 0.38 * fsin(yy * 0.0085 + phase * 1.7))
}

#[inline]
fn stream_w(y: f32, width: f32, phase: f32, drift: f32) -> f32 {
    let yy = y + drift;
    let w = width
        * (0.62
            + 0.28 * fsin(yy * 0.037 + phase * 2.1)
            + 0.18 * fsin(yy * 0.011 + phase * 0.7));
    maxf(w, width * 0.25)
}

/// 一条长水流：沿 y 逐行取蜿蜒中心线和半宽，横截面是有限斜率的拱。
/// 宽度沿程起伏，和珠子叠在一起时就会形成参考图里那种粗细不均的水痕。
unsafe fn stream(
    hb: *mut f32,
    sb: *mut f32,
    gw: usize,
    gh: usize,
    x0: f32,
    phase: f32,
    amp: f32,
    width: f32,
    height: f32,
    y_top: f32,
    y_bottom: f32,
    drift: f32,
) {
    let y0 = maxf(0.0, floorf(y_top)) as usize;
    let y1 = minf((gh - 1) as f32, ceilf(y_bottom)) as usize;
    let mut py = y0;
    while py <= y1 {
        let yf = py as f32;
        let xc = stream_x(yf, x0, phase, amp, drift);
        let w = stream_w(yf, width, phase, drift);
        let yy = yf + drift;
        let hgt = height
            * (0.72 + 0.22 * fsin(yy * 0.026 + phase * 1.3)
                + 0.10 * fsin(yy * 0.007 + phase * 0.4));
        let ia = 1.0 / (w * w);
        let x0i = maxf(0.0, floorf(xc - w)) as usize;
        let x1i = minf((gw - 1) as f32, ceilf(xc + w)) as usize;
        let row = py * gw;
        let mut px = x0i;
        while px <= x1i {
            let dx = px as f32 - xc;
            let q2 = dx * dx * ia;
            if q2 < 1.0 {
                let v = (1.0 - q2) * hgt;
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
///
/// 三层：细密水雾铺底；抖动网格上的圆水珠/泪滴；少量贯穿画面的长水流，
/// 水流上再串几颗大水珠。所有层都 screen 叠加，交界处自然融成一片。
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

    let cell = maxf(8.0, min_dim * 0.062);
    let rad_base = 0.18 + 0.50 * size01;
    let rad_nom = cell * rad_base * 0.82;
    let raw_thresh = 0.15 + 0.85 * density01;
    let drift = time * speed * 0.35;

    // ---- 1. 细密水雾：更小的网格、1–4px 的雾滴，铺满玻璃。
    let cell_m = maxf(3.0, cell * 0.20);
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
            let cx = (gx as f32 + 0.5 + (unit(h1) - 0.5) * 0.92) * cell_m;
            let cy = (gy as f32 + 0.5 + (unit(h2) - 0.5) * 0.92) * cell_m;
            let r = cell_m * (0.10 + 0.26 * unit(h3));
            let vh = 0.38 + 0.34 * unit(hash_u32(h3 ^ 0x1f83_d9ab));
            splat(hb, sb, gw, gh, cx, cy, r, r * 0.94, vh, r);
            gx += 1;
        }
        gy += 1;
    }

    // ---- 2. 中号水珠：抖动网格，density 定数量，size 定半径。
    let inv_cell = 1.0 / cell;
    let gwi = ceilf(wf * inv_cell) as i32 + 1;
    let ghi = ceilf(hf * inv_cell) as i32 + 1;
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
            if unit(h0) > raw_thresh || cm > 0.35 + 0.65 * density01 {
                gx += 1;
                continue;
            }
            let h1 = hash_u32(h0 ^ 0xa5a5_a5a5);
            let h2 = hash_u32(h1.wrapping_add(0x1234_5677));
            let h3 = hash_u32(h2 ^ 0x68e3_1da4);
            let h4 = hash_u32(h3.wrapping_add(0x7f4a_7c15));
            let h5 = hash_u32(h4 ^ 0x2545_f491);
            let jx = (unit(h1) - 0.5) * 1.02;
            let jy = (unit(h2) - 0.5) * 1.02;
            let cx = (gx as f32 + 0.5 + jx) * cell;
            let cy = (gy as f32 + 0.5 + jy) * cell;
            let r = rad_nom * (0.42 + 1.05 * unit(h3)) * (0.75 + 0.55 * cm);
            let teardrop = unit(h4) < 0.22;
            let ra = r * (0.92 + 0.16 * unit(h1));
            let rb = if teardrop {
                r * (1.25 + 0.25 * unit(h2))
            } else {
                r * (0.92 + 0.16 * unit(h2))
            };
            let vh = 0.88 + 0.12 * unit(h5);
            splat(hb, sb, gw, gh, cx, cy, ra, rb, vh, maxf(ra, rb));
            gx += 1;
        }
        gy += 1;
    }

    // ---- 3. 长水流：参考图的主角。数量由 streak 控制，长度由 trail 控制。
    let count = (3.0 + streak01 * 15.0) as i32;
    let mut s = 0;
    while s < count {
        let hs = hash_u32(0x51ed_2701u32.wrapping_add((s as u32).wrapping_mul(0x9e37_79b9)));
        let hs2 = hash_u32(hs ^ 0xbb67_ae85);
        let hs3 = hash_u32(hs2.wrapping_add(0x3c6e_f372));
        let hs4 = hash_u32(hs3 ^ 0x510e_527f);
        let hs5 = hash_u32(hs4.wrapping_add(0x1f83_d9ab));

        let x0 = unit(hs) * wf;
        let phase = unit(hs2) * TAU;
        let amp = rad_nom * (0.8 + 1.6 * unit(hs3));
        let width = rad_nom * (0.30 + 0.35 * unit(hs4));
        let height = 0.52 + 0.30 * unit(hs5);
        let len = hf * (0.35 + 0.65 * trail01);
        let top = -len * 0.15 + unit(hs2) * hf * 0.55;
        let bottom = top + len;

        stream(
            hb, sb, gw, gh, x0, phase, amp, width, height, top, bottom, drift,
        );

        // 水流上的大水珠：每隔一段长一颗，和水流融在一起，形成粗细不均的水痕。
        let beads = 2 + (unit(hs3) * 4.0) as i32;
        let mut k = 0;
        while k < beads {
            let hb2 = hash_u32(hs4.wrapping_add((k as u32).wrapping_mul(0x27d4_eb2d)));
            let y = top + (k as f32 + 0.5) / beads as f32 * len;
            let x = stream_x(y, x0, phase, amp, drift);
            let r = width * (1.4 + 1.2 * unit(hb2));
            let vh = 0.85 + 0.15 * unit(hash_u32(hb2 ^ 0x68e3_1da4));
            splat(hb, sb, gw, gh, x, y, r * 0.95, r * 1.15, vh, r);
            k += 1;
        }
        s += 1;
    }

    // 折射量按标称半径缩放：小雾滴不搅大范围，大水珠才有厚透镜的位移。
    (0.26 * rad_nom * rad_nom, 0.60 * rad_nom, rad_nom)
}
