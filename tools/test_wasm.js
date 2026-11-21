// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 MOSSLS
//
// 无宿主测试：直接加载 wasm，按 ABI 1 的约定铺参数和像素，跑几张测试图看效果。
//   node tools/test_wasm.js [--wasm=dist/zh-zh/mos_rain.wasm] [--w=960] [--h=540]
//                           [--card=synthetic|photo]
//
// 输出写到 tests/out/*.png（.gitignore 已忽略），并打印每张图的耗时和改动量。

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
const CARD = arg('card', 'synthetic'); // synthetic | photo
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
  ihdr[8] = 8; // bit depth
  ihdr[9] = 2; // RGB
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
    // 预乘 → 直通
    const a = pix[o + 3];
    const inv = a > 1e-6 ? 1 / a : 0;
    out[i * 3] = Math.max(0, Math.min(255, Math.round(pix[o] * inv * 255)));
    out[i * 3 + 1] = Math.max(0, Math.min(255, Math.round(pix[o + 1] * inv * 255)));
    out[i * 3 + 2] = Math.max(0, Math.min(255, Math.round(pix[o + 2] * inv * 255)));
  }
  return out;
}

// 局部放大（最近邻），用来盯雨滴边缘、拖尾这类细节。
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

function makeTestCard(w, h) {
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
      // 大色块
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
      // 细网格（折射最容易看出来）
      const grid = 40;
      if (x % grid < 1.5 || y % grid < 1.5) {
        r += 0.22;
        g += 0.24;
        b += 0.26;
      }
      // 同心环
      const dx = x - w / 2;
      const dy = y - h / 2;
      const rr = Math.hypot(dx, dy);
      const ring = ((rr % 76) + 76) % 76;
      if (Math.abs(ring - 38) < 1.6) {
        r += 0.40;
        g += 0.34;
        b += 0.12;
      }
      // 右下角棋盘（高频细节）
      if (x > w - 300 && y > h - 170) {
        const c = ((Math.floor(x / 14) + Math.floor(y / 14)) & 1) === 0;
        if (c) {
          r = 0.92;
          g = 0.92;
          b = 0.95;
        } else {
          r = 0.06;
          g = 0.07;
          b = 0.09;
        }
      }
      // 文字似的色条
      if (y > 26 && y < 44 && x > 30 && x < 30 + 190) {
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

// ---------------------------------------------------------------- 真实素材
// 用 ffmpeg 预转成 rgba8：
// ffmpeg -i in.png -vf "scale=960:540:force_original_aspect_ratio=increase,crop=960:540" \
//   -pix_fmt rgba -f rawvideo tests/assets/photo-960x540.rgba

function loadPhoto(w, h) {
  if (!fs.existsSync(PHOTO)) {
    console.error(`找不到 ${PHOTO}\n用 ffmpeg 把一张图转成 rgba8 放进去。`);
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

// ---------------------------------------------------------------- 参数（17 → 20 个 f64）

function baseParams() {
  return {
    amount: 140,
    size: 45,
    density: 40,
    streak: 50,
    trail: 60,
    mist: 60,
    speed: 20,
    dispersion: 12,
    highlight: 95,
    rim: 40,
    light: 135,
    frost: 0,
    tint: [0.72, 0.88, 1.0, 1.0],
    tintAmount: 0,
    sampling: 0,
    edge: 0,
    mix: 100,
    seed: 0,
    timeOffset: 4,
  };
}

function flatten(d) {
  return [
    d.amount,
    d.size,
    d.density,
    d.streak,
    d.trail,
    d.mist,
    d.speed,
    d.dispersion,
    d.highlight,
    d.rim,
    d.light,
    d.frost,
    d.tint[0],
    d.tint[1],
    d.tint[2],
    d.tint[3],
    d.tintAmount,
    d.sampling,
    d.edge,
    d.mix,
    d.seed,
    d.timeOffset,
  ];
}

const CASES = [
  { name: '01-rain-default', p: {}, crop: [280, 80, 320, 240] },
  {
    name: '02-rain-dense',
    p: { density: 88, mist: 85, size: 48, amount: 135 },
  },
  { name: '03-rain-round', p: { streak: 0, density: 65 } },
  {
    name: '04-rain-streak',
    p: { streak: 100, trail: 55, density: 40 },
    crop: [280, 40, 320, 300],
  },
  {
    name: '05-rain-longtrail',
    p: { streak: 75, trail: 100, speed: 70, density: 40 },
    t: 1.2,
  },
  {
    name: '06-rain-mist',
    p: { size: 8, density: 0, mist: 100, streak: 0, amount: 100, highlight: 95 },
    crop: [280, 120, 320, 220],
  },
  { name: '07-rain-frost', p: { frost: 65 } },
  { name: '08-rain-bicubic', p: { sampling: 1, amount: 140 } },
  {
    name: '09-rain-nolight',
    p: { highlight: 0, rim: 0, dispersion: 0 },
  },
  { name: '13-rain-empty', p: { timeOffset: 0 }, t: 0.0 },
  { name: '12-rain-motion-a', p: {}, t: 0.0 },
  { name: '12-rain-motion-b', p: {}, t: 0.5 },
  { name: '12-rain-motion-c', p: {}, t: 1.0 },
  { name: '12-rain-motion-d', p: {}, t: 1.5 },
  {
    name: '10-rain-edge-transparent',
    p: { edge: 3, amount: 190, dispersion: 0 },
  },
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

  const card = CARD === 'photo' ? loadPhoto(W, H) : makeTestCard(W, H);
  const outDir = path.join(ROOT, 'tests', 'out', CARD === 'photo' ? 'photo' : '.');
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
  check('manifest id', manifest.id === 'mos.rain', manifest.id);
  check('19 ui params', manifest.params.length === 19, String(manifest.params.length));

  const mix0 = renderRaw(Object.assign(baseParams(), { mix: 0 }));
  check('mix=0 returns source', mix0.rc === 0 && samePixels(mix0.pix, card));

  const det0 = renderRaw(baseParams());
  const det1 = renderRaw(baseParams());
  check('deterministic', samePixels(det0.pix, det1.pix));
  check(
    'default changes pixels',
    det0.rc === 0 && changedRatio(det0.pix, card, W * H) > 0.10,
    (changedRatio(det0.pix, card, W * H) * 100).toFixed(1) + '%'
  );

  const noStreak = renderRaw(Object.assign(baseParams(), { streak: 0 }));
  check(
    'streak changes image',
    changedRatio(noStreak.pix, det0.pix, W * H) > 0.005,
    (changedRatio(noStreak.pix, det0.pix, W * H) * 100).toFixed(2) + '%'
  );

  const off = renderRaw(
    Object.assign(baseParams(), {
      amount: 0,
      highlight: 0,
      rim: 0,
      frost: 0,
      tintAmount: 0,
    })
  );
  check('all zero returns source', off.rc === 0 && samePixels(off.pix, card));
  console.log(`自检 ${passed}/${total} 通过`);

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
      `${c.name.padEnd(24)} ${ms.toFixed(1).padStart(7)}ms  changed ${(
        (changed / (W * H)) *
        100
      ).toFixed(1)}%  meanΔ ${(sum / (W * H)).toFixed(4)}  maxΔ ${maxd.toFixed(3)}`
    );
  }
  console.log(`→ ${outDir}`);
}

main();
