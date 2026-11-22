// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 MOSSLS
//
// 无宿主测试：直接加载 wasm，按 ABI 1 铺参数和像素，运行自检并输出样张。
//   node tools/test_wasm.js [--wasm=dist/zh-zh/mos_rain.wasm] [--w=960] [--h=540]
//                           [--card=synthetic|window|photo]

const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const ROOT = path.join(__dirname, '..');

function arg(name, def) {
  const hit = process.argv.find((a) => a.startsWith(`--${name}=`));
  return hit ? hit.split('=')[1] : def;
}

const W = parseInt(arg('w', '960'), 10);
const H = parseInt(arg('h', '540'), 10);
const WASM = path.resolve(ROOT, arg('wasm', 'dist/zh-zh/mos_rain.wasm'));
const CARD = arg('card', 'synthetic'); // synthetic | window | photo
const PHOTO = path.join(ROOT, 'tests', 'assets', `photo-${W}x${H}.rgba`);

// ---------------------------------------------------------------- PNG

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([len, body, crc]);
}

function encodePng(w, h, rgb) {
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) {
    raw[y * (w * 3 + 1)] = 0;
    rgb.copy(raw, y * (w * 3 + 1) + 1, y * w * 3, (y + 1) * w * 3);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8;
  ihdr[9] = 2;
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(raw, { level: 6 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

function toRgb(pix, w, h) {
  const out = Buffer.alloc(w * h * 3);
  for (let i = 0, n = w * h; i < n; i++) {
    const o = i * 4;
    const a = pix[o + 3];
    const inv = a > 1e-6 ? 1 / a : 0;
    out[i * 3] = Math.max(0, Math.min(255, Math.round(pix[o] * inv * 255)));
    out[i * 3 + 1] = Math.max(0, Math.min(255, Math.round(pix[o + 1] * inv * 255)));
    out[i * 3 + 2] = Math.max(0, Math.min(255, Math.round(pix[o + 2] * inv * 255)));
  }
  return out;
}

function cropZoom(pix, w, h, rect, zoom) {
  const [cx, cy, cw, ch] = rect;
  const ow = cw * zoom;
  const oh = ch * zoom;
  const out = Buffer.alloc(ow * oh * 3);
  for (let y = 0; y < oh; y++) {
    const sy = Math.max(0, Math.min(h - 1, cy + Math.floor(y / zoom)));
    for (let x = 0; x < ow; x++) {
      const sx = Math.max(0, Math.min(w - 1, cx + Math.floor(x / zoom)));
      const o = (sy * w + sx) * 4;
      const a = pix[o + 3];
      const inv = a > 1e-6 ? 1 / a : 0;
      const d = (y * ow + x) * 3;
      out[d] = Math.max(0, Math.min(255, Math.round(pix[o] * inv * 255)));
      out[d + 1] = Math.max(0, Math.min(255, Math.round(pix[o + 1] * inv * 255)));
      out[d + 2] = Math.max(0, Math.min(255, Math.round(pix[o + 2] * inv * 255)));
    }
  }
  return encodePng(ow, oh, out);
}

// ---------------------------------------------------------------- 测试卡

function makeSyntheticCard(w, h) {
  const px = new Float32Array(w * h * 4);
  const blobs = [
    [0.28, 0.34, 0.20, [1.0, 0.52, 0.18]],
    [0.72, 0.30, 0.16, [0.15, 0.85, 0.95]],
    [0.55, 0.74, 0.22, [0.95, 0.22, 0.55]],
    [0.18, 0.78, 0.12, [0.45, 1.0, 0.35]],
  ];
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const u = x / w;
      const v = y / h;
      let r = 0.035 + 0.07 * u;
      let g = 0.075 + 0.10 * v;
      let b = 0.16 + 0.22 * (1.0 - u * 0.55);
      for (const [bx, by, br, c] of blobs) {
        const dx = (u - bx) * (w / h);
        const dy = v - by;
        const d = Math.hypot(dx, dy);
        const fall = Math.max(0, 1 - d / br);
        const k = fall * fall * (3 - 2 * fall) * 0.9;
        r += (c[0] - r) * k;
        g += (c[1] - g) * k;
        b += (c[2] - b) * k;
      }
      const grid = 40;
      if (x % grid < 1.5 || y % grid < 1.5) {
        r += 0.22;
        g += 0.24;
        b += 0.26;
      }
      const dx = x - w / 2;
      const dy = y - h / 2;
      const rr = Math.hypot(dx, dy);
      const ring = ((rr % 76) + 76) % 76;
      if (Math.abs(ring - 38) < 1.6) {
        r += 0.40;
        g += 0.34;
        b += 0.12;
      }
      if (x > w - 300 && y > h - 170) {
        const c = ((Math.floor(x / 14) + Math.floor(y / 14)) & 1) === 0;
        r = c ? 0.92 : 0.06;
        g = c ? 0.92 : 0.07;
        b = c ? 0.95 : 0.09;
      }
      if (y > 26 && y < 44 && x > 30 && x < 220) {
        const band = Math.floor((x - 30) / 22) % 3;
        if (band === 0) {
          r = 0.98;
          g = 0.82;
          b = 0.25;
        } else if (band === 1) {
          r = 0.25;
          g = 0.92;
          b = 0.55;
        } else {
          r = 0.95;
          g = 0.35;
          b = 0.35;
        }
      }
      const i = (y * w + x) * 4;
      px[i] = Math.max(0, Math.min(1, r));
      px[i + 1] = Math.max(0, Math.min(1, g));
      px[i + 2] = Math.max(0, Math.min(1, b));
      px[i + 3] = 1;
    }
  }
  return px;
}

/// 真实雨窗模拟测试卡：冷青阴天亮窗 + 深灰窗格竖条与横梁 + 柔光背景
function makeWindowCard(w, h) {
  const px = new Float32Array(w * h * 4);
  const mullionsX = [0.14, 0.35, 0.62, 0.84];
  const transomsY = [0.34, 0.68];
  for (let y = 0; y < h; y++) {
    const v = y / h;
    for (let x = 0; x < w; x++) {
      const u = x / w;
      // 阴天冷青渐变窗景（上亮下暗）
      let sky = Math.max(0, 1.0 - v * 0.92);
      sky = sky * sky * 0.65 + 0.18 * (1.0 - v);
      let r = 0.09 + 0.66 * sky + 0.14 * Math.sin(u * 4.2 + 0.5) * sky;
      let g = 0.14 + 0.72 * sky + 0.12 * Math.sin(u * 4.2 + 0.5) * sky;
      let b = 0.17 + 0.74 * sky + 0.10 * Math.cos(u * 3.1) * sky;

      // 柔和散景亮斑
      const bokeh = [
        [0.24, 0.18, 0.16, 0.18],
        [0.50, 0.24, 0.20, 0.22],
        [0.74, 0.19, 0.15, 0.18],
        [0.44, 0.52, 0.18, 0.12],
      ];
      for (const [bx, by, br, gain] of bokeh) {
        const d = Math.hypot((u - bx) * (w / h), v - by);
        const s = Math.max(0, 1 - d / br);
        const k = s * s * (3 - 2 * s) * gain;
        r += k * 0.88;
        g += k * 0.96;
        b += k * 1.00;
      }

      // 虚化深色窗框竖条与横梁
      let frame = 0;
      for (const mx of mullionsX) {
        const d = Math.abs(u - mx);
        if (d < 0.032) {
          const s = 1 - d / 0.032;
          frame = Math.max(frame, s * s * (3 - 2 * s));
        }
      }
      for (const ty of transomsY) {
        const d = Math.abs(v - ty);
        if (d < 0.028) {
          const s = 1 - d / 0.028;
          frame = Math.max(frame, s * s * (3 - 2 * s) * 0.78);
        }
      }
      r *= 1 - 0.84 * frame;
      g *= 1 - 0.82 * frame;
      b *= 1 - 0.78 * frame;

      const i = (y * w + x) * 4;
      px[i] = Math.max(0, Math.min(1, r));
      px[i + 1] = Math.max(0, Math.min(1, g));
      px[i + 2] = Math.max(0, Math.min(1, b));
      px[i + 3] = 1;
    }
  }
  return px;
}

function loadPhoto(w, h) {
  if (!fs.existsSync(PHOTO)) {
    console.error(`找不到 ${PHOTO}`);
    process.exit(1);
  }
  const raw = fs.readFileSync(PHOTO);
  if (raw.length !== w * h * 4) {
    console.error(`${PHOTO} 大小 ${raw.length}，期望 ${w * h * 4}`);
    process.exit(1);
  }
  const px = new Float32Array(w * h * 4);
  for (let i = 0; i < w * h * 4; i++) px[i] = raw[i] / 255;
  return px;
}

// ---------------------------------------------------------------- 自检工具

function samePixels(a, b) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

function changedRatio(a, b, n) {
  let c = 0;
  for (let i = 0; i < n; i++) {
    let d = 0;
    for (let k = 0; k < 4; k++) d += Math.abs(a[i * 4 + k] - b[i * 4 + k]);
    if (d / 4 > 0.002) c++;
  }
  return c / n;
}

// ---------------------------------------------------------------- 参数（22 个 f64）

function baseParams() {
  return {
    bigDensity: 55,
    bigSize: 55,
    bigPhase: 0,
    bigDisp: 100,
    smallDensity: 65,
    smallSize: 50,
    smallPhase: 0,
    smallDisp: 100,
    streakDensity: 55,
    streakSize: 55,
    streakPhase: 0,
    streakDisp: 100,
    flowSpeed: 60,
    displacement: 100,
    dispersion: 22,
    highlight: 85,
    lightAngle: 120,
    bgBlur: 18,
    seed: 0,
    sampling: 0,
    edge: 0,
    mix: 100,
  };
}

function flatten(d) {
  return [
    d.bigDensity,
    d.bigSize,
    d.bigPhase,
    d.bigDisp,
    d.smallDensity,
    d.smallSize,
    d.smallPhase,
    d.smallDisp,
    d.streakDensity,
    d.streakSize,
    d.streakPhase,
    d.streakDisp,
    d.flowSpeed,
    d.displacement,
    d.dispersion,
    d.highlight,
    d.lightAngle,
    d.bgBlur,
    d.seed,
    d.sampling,
    d.edge,
    d.mix,
  ];
}

const CASES = [
  { name: '01-rain-default', p: {}, crop: [240, 80, 320, 240] },
  {
    name: '02-rain-big-only',
    p: { smallDensity: 0, streakDensity: 0, bigDensity: 75, bigSize: 75, bgBlur: 0 },
  },
  {
    name: '03-rain-small-only',
    p: { bigDensity: 0, streakDensity: 0, smallDensity: 85, smallSize: 65, bgBlur: 12 },
  },
  {
    name: '04-rain-streak-only',
    p: { bigDensity: 0, smallDensity: 0, streakDensity: 80, streakSize: 70, bgBlur: 22 },
    crop: [240, 80, 320, 260],
  },
  {
    name: '05-rain-dispersion',
    p: { dispersion: 90, displacement: 150, bigSize: 80, bgBlur: 0 },
    crop: [240, 80, 320, 240],
  },
  { name: '06-rain-heavy-fog', p: { bgBlur: 70, streakDensity: 75, bigDensity: 65 } },
  { name: '07-rain-nolight', p: { highlight: 0, bgBlur: 0, dispersion: 0 } },
  { name: '08-rain-t0', p: {}, t: 0.0 },
  { name: '09-rain-t1', p: {}, t: 0.5 },
  { name: '10-rain-t2', p: {}, t: 1.0 },
  { name: '11-rain-t3', p: {}, t: 1.5 },
  { name: '12-rain-bicubic', p: { sampling: 1, dispersion: 40 } },
  { name: '13-rain-edge-transparent', p: { edge: 3, displacement: 150, bgBlur: 0 } },
];

// ---------------------------------------------------------------- 主流程

function main() {
  if (!fs.existsSync(WASM)) {
    console.error(`找不到 wasm: ${WASM}\n先运行 node build.js --variant=zh-zh`);
    process.exit(1);
  }
  const bytes = fs.readFileSync(WASM);
  const mod = new WebAssembly.Module(bytes);
  const imports = WebAssembly.Module.imports(mod);
  if (imports.length) {
    console.error('wasm 不应有 import：', imports);
    process.exit(1);
  }
  const instance = new WebAssembly.Instance(mod, {});
  const ex = instance.exports;
  const manifest = JSON.parse(
    Buffer.from(ex.memory.buffer, ex.ec_manifest_ptr(), ex.ec_manifest_len()).toString('utf8')
  );
  console.log(
    `wasm ${bytes.length}B  api ${ex.ec_api_version()}  id ${manifest.id}  params ${manifest.params.length}`
  );

  const NPARAMS = flatten(baseParams()).length;
  if (NPARAMS !== 22) throw new Error(`参数展开应为 22 个 f64，实际 ${NPARAMS}`);

  const card =
    CARD === 'photo'
      ? loadPhoto(W, H)
      : CARD === 'window'
        ? makeWindowCard(W, H)
        : makeSyntheticCard(W, H);
  const outDir = path.join(
    ROOT,
    'tests',
    'out',
    CARD === 'photo' ? 'photo' : CARD === 'window' ? 'window' : '.'
  );
  fs.mkdirSync(outDir, { recursive: true });

  let passed = 0;
  let total = 0;
  function check(name, ok, detail) {
    total++;
    if (ok) passed++;
    else console.error(`FAIL  ${name}${detail ? ' — ' + detail : ''}`);
  }

  function renderRaw(p, t = 0) {
    const totalBytes = NPARAMS * 8 + W * H * 16;
    const base = ex.ec_alloc(totalBytes);
    if (!base) throw new Error('ec_alloc 失败');
    new Float64Array(ex.memory.buffer, base, NPARAMS).set(flatten(p));
    const pixOff = base + NPARAMS * 8;
    new Float32Array(ex.memory.buffer, pixOff, W * H * 4).set(card);
    const rc = ex.ec_render(pixOff, W, H, base, NPARAMS, t, 1.0);
    return { rc, pix: new Float32Array(new Float32Array(ex.memory.buffer, pixOff, W * H * 4)) };
  }

  check('api version = 1', ex.ec_api_version() === 1, String(ex.ec_api_version()));
  check('manifest id = mos.rain', manifest.id === 'mos.rain', manifest.id);
  check('22 ui params', manifest.params.length === 22, String(manifest.params.length));

  const mix0 = renderRaw(Object.assign(baseParams(), { mix: 0 }));
  check('mix=0 returns source', mix0.rc === 0 && samePixels(mix0.pix, card));

  const det0 = renderRaw(baseParams());
  const det1 = renderRaw(baseParams());
  check('deterministic', samePixels(det0.pix, det1.pix));
  check(
    'default changes pixels',
    det0.rc === 0 && changedRatio(det0.pix, card, W * H) > 0.15,
    (changedRatio(det0.pix, card, W * H) * 100).toFixed(1) + '%'
  );

  const bigOnly0 = renderRaw(
    Object.assign(baseParams(), { smallDensity: 0, streakDensity: 0, bgBlur: 0 })
  );
  check('big only changes pixels', changedRatio(bigOnly0.pix, card, W * H) > 0.02);

  const smallOnly0 = renderRaw(
    Object.assign(baseParams(), { bigDensity: 0, streakDensity: 0, bgBlur: 0 })
  );
  check('small only changes pixels', changedRatio(smallOnly0.pix, card, W * H) > 0.02);

  const streakOnly0 = renderRaw(
    Object.assign(baseParams(), { bigDensity: 0, smallDensity: 0, bgBlur: 0 })
  );
  check('streak only changes pixels', changedRatio(streakOnly0.pix, card, W * H) > 0.01);

  const bigPhase = renderRaw(
    Object.assign(baseParams(), { smallDensity: 0, streakDensity: 0, bgBlur: 0, bigPhase: 12 })
  );
  check('big_phase works independently', changedRatio(bigPhase.pix, bigOnly0.pix, W * H) > 0.01);

  const smallPhase = renderRaw(
    Object.assign(baseParams(), { bigDensity: 0, streakDensity: 0, bgBlur: 0, smallPhase: 12 })
  );
  check(
    'small_phase works independently',
    changedRatio(smallPhase.pix, smallOnly0.pix, W * H) > 0.01
  );

  const streakPhase = renderRaw(
    Object.assign(baseParams(), { bigDensity: 0, smallDensity: 0, bgBlur: 0, streakPhase: 12 })
  );
  check(
    'streak_phase works independently',
    changedRatio(streakPhase.pix, streakOnly0.pix, W * H) > 0.01
  );

  const bigDisp0 = renderRaw(
    Object.assign(baseParams(), { smallDensity: 0, streakDensity: 0, bgBlur: 0, bigDisp: 0 })
  );
  check('big_disp changes pixels', changedRatio(bigDisp0.pix, bigOnly0.pix, W * H) > 0.02);

  const smallDisp0 = renderRaw(
    Object.assign(baseParams(), { bigDensity: 0, streakDensity: 0, bgBlur: 0, smallDisp: 0 })
  );
  check('small_disp changes pixels', changedRatio(smallDisp0.pix, smallOnly0.pix, W * H) > 0.02);

  const streakDisp0 = renderRaw(
    Object.assign(baseParams(), { bigDensity: 0, smallDensity: 0, bgBlur: 0, streakDisp: 0 })
  );
  const streakDispThresh = CARD === 'window' ? 0.002 : 0.008;
  check(
    'streak_disp changes pixels',
    changedRatio(streakDisp0.pix, streakOnly0.pix, W * H) > streakDispThresh,
    (changedRatio(streakDisp0.pix, streakOnly0.pix, W * H) * 100).toFixed(2) + '%'
  );

  const disp0 = renderRaw(Object.assign(baseParams(), { dispersion: 0 }));
  const disp90 = renderRaw(Object.assign(baseParams(), { dispersion: 90 }));
  check('dispersion changes pixels', changedRatio(disp0.pix, disp90.pix, W * H) > 0.02);

  const t0Frame = renderRaw(baseParams(), 0.0);
  const t1Frame = renderRaw(baseParams(), 0.5);
  check('flow_speed animates over time', changedRatio(t0Frame.pix, t1Frame.pix, W * H) > 0.04);

  const off = renderRaw(
    Object.assign(baseParams(), {
      bigDensity: 0,
      smallDensity: 0,
      streakDensity: 0,
      bgBlur: 0,
    })
  );
  check('all zero returns source', off.rc === 0 && samePixels(off.pix, card));

  console.log(`自检 ${passed}/${total} 通过`);
  if (passed !== total) process.exitCode = 1;

  fs.writeFileSync(path.join(outDir, '00-source.png'), encodePng(W, H, toRgb(card, W, H)));

  for (const c of CASES) {
    const params = Object.assign(baseParams(), c.p);
    const totalBytes = NPARAMS * 8 + W * H * 16;
    const base = ex.ec_alloc(totalBytes);
    if (!base) throw new Error('ec_alloc 失败');
    new Float64Array(ex.memory.buffer, base, NPARAMS).set(flatten(params));
    const pixOff = base + NPARAMS * 8;
    new Float32Array(ex.memory.buffer, pixOff, W * H * 4).set(card);

    const t0 = process.hrtime.bigint();
    const rc = ex.ec_render(pixOff, W, H, base, NPARAMS, c.t || 0, 1.0);
    const ms = Number(process.hrtime.bigint() - t0) / 1e6;
    if (rc !== 0) throw new Error(`ec_render 返回 ${rc} (${c.name})`);

    const pix = new Float32Array(ex.memory.buffer, pixOff, W * H * 4);
    let changed = 0;
    let sum = 0;
    let maxd = 0;
    for (let i = 0; i < W * H; i++) {
      let d = 0;
      for (let k = 0; k < 4; k++) d += Math.abs(card[i * 4 + k] - pix[i * 4 + k]);
      d /= 4;
      sum += d;
      if (d > 0.002) changed++;
      if (d > maxd) maxd = d;
    }
    fs.writeFileSync(path.join(outDir, `${c.name}.png`), encodePng(W, H, toRgb(pix, W, H)));
    if (c.crop) {
      fs.writeFileSync(
        path.join(outDir, `${c.name}-zoom.png`),
        cropZoom(pix, W, H, c.crop, 3)
      );
    }
    console.log(
      `${c.name.padEnd(26)} ${ms.toFixed(1).padStart(7)}ms  changed ${(
        (changed / (W * H)) *
        100
      ).toFixed(1)}%  meanΔ ${(sum / (W * H)).toFixed(4)}  maxΔ ${maxd.toFixed(3)}`
    );
  }
  console.log(`→ ${outDir}`);
}

main();
