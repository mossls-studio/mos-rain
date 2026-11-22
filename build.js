// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 MOSSLS

// build.js — 编译 Rust 源码（内嵌 manifest）→ dist/<names>-<params>/mos_rain.wasm
//   node build.js                 构建 4 种语言组合
//   node build.js --variant=zh-en 只构建「中文名称 + 英文参数」一种
//   node build.js --lang=zh       兼容旧用法（= zh-zh），--lang=en = en-en
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = __dirname;
const WASM_NAME = 'mos_rain.wasm';
const CRATE = 'mos_rain';
const VARIANTS = ['zh-zh', 'zh-en', 'en-zh', 'en-en'];
const TARGET = 'wasm32-unknown-unknown';
const TARGET_DIR = path.join(ROOT, 'build', 'target');

function pickLang(value, lang) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return undefined;
  if (!Object.prototype.hasOwnProperty.call(value, lang)) return undefined;
  return value[lang];
}

function localize(base, overlay, namesLang, paramsLang) {
  const out = JSON.parse(JSON.stringify(base));
  for (const key of ['category', 'name', 'description']) {
    const v = pickLang(overlay[key], namesLang);
    if (typeof v === 'string') out[key] = v;
  }
  const groups = overlay.groups || {};
  const perParam = overlay.params || {};
  for (const p of out.params) {
    const po = perParam[p.id];
    if (po) {
      const n = pickLang(po.name, paramsLang);
      if (typeof n === 'string') p.name = n;
      const opts = pickLang(po.options, paramsLang);
      if (Array.isArray(opts)) {
        p.options = opts.map((o, i) => {
          const v = typeof o === 'string' ? o : pickLang(o, paramsLang);
          return typeof v === 'string' ? v : p.options[i];
        });
      }
    }
    if (p.group && groups[p.group]) {
      const g = pickLang(groups[p.group], namesLang);
      if (typeof g === 'string') p.group = g;
    }
  }
  return out;
}

function buildVariant(variant, base, overlay) {
  const [namesLang, paramsLang] = variant.split('-');
  const manifest = JSON.stringify(localize(base, overlay, namesLang, paramsLang));
  const mbytes = Buffer.from(manifest, 'utf8');

  const buildDir = path.join(ROOT, 'build');
  fs.mkdirSync(buildDir, { recursive: true });
  const manifestPath = path.join(buildDir, `manifest.${variant}.json`);
  fs.writeFileSync(manifestPath, manifest + '\n');

  const env = Object.assign({}, process.env, { MOS_MANIFEST_FILE: manifestPath });
  const res = spawnSync(
    'cargo',
    ['build', '--release', '--target', TARGET, '--target-dir', TARGET_DIR],
    { cwd: ROOT, env, encoding: 'utf8' }
  );
  if (res.status !== 0) {
    console.error(`cargo 编译失败 [${variant}]:\n${res.stderr || res.stdout || ''}`);
    process.exit(1);
  }

  const artifact = path.join(TARGET_DIR, TARGET, 'release', `${CRATE}.wasm`);
  if (!fs.existsSync(artifact)) {
    console.error(`找不到产物 ${artifact}（检查 Cargo.toml 的 crate-type）`);
    process.exit(1);
  }
  const wasm = fs.readFileSync(artifact);

  if (wasm.indexOf(mbytes) < 0) {
    console.error(
      `[${variant}] 产物里没有嵌进本语言的 manifest —— cargo 复用了旧产物。\n` +
        `试试清理 build/target 后重跑。`
    );
    process.exit(1);
  }

  const distDir = path.join(ROOT, 'dist', variant);
  fs.mkdirSync(distDir, { recursive: true });
  fs.writeFileSync(path.join(distDir, WASM_NAME), wasm);

  const readme = path.join(ROOT, `README.${namesLang}.md`);
  const readmeFallback = path.join(ROOT, 'README.md');
  const readmeSrc = fs.existsSync(readme) ? readme : readmeFallback;
  if (fs.existsSync(readmeSrc)) {
    fs.copyFileSync(readmeSrc, path.join(distDir, 'README.md'));
  }

  if (variant === 'zh-zh') {
    const flat = path.join(ROOT, 'dist', WASM_NAME);
    fs.copyFileSync(path.join(distDir, WASM_NAME), flat);
    const alias = path.join(ROOT, 'dist', 'mos_rain');
    fs.mkdirSync(alias, { recursive: true });
    fs.copyFileSync(path.join(distDir, WASM_NAME), path.join(alias, WASM_NAME));
    if (fs.existsSync(readmeSrc)) {
      fs.copyFileSync(readmeSrc, path.join(alias, 'README.md'));
    }
  }

  console.log(`OK  [${variant}]  wasm ${wasm.length} bytes  manifest ${mbytes.length} bytes  ->  ${distDir}`);
}

function main() {
  let variants = VARIANTS;
  const variantArg = process.argv.find((a) => a.startsWith('--variant='));
  const langArg = process.argv.find((a) => a.startsWith('--lang='));
  if (variantArg) variants = [variantArg.split('=')[1]];
  else if (langArg) {
    const l = langArg.split('=')[1];
    variants = [`${l}-${l}`];
  }
  for (const v of variants) {
    if (!VARIANTS.includes(v)) throw new Error(`unknown variant ${v} (expected ${VARIANTS.join('|')})`);
  }
  const base = JSON.parse(fs.readFileSync(path.join(ROOT, 'src', 'manifest.json'), 'utf8'));
  const overlay = JSON.parse(fs.readFileSync(path.join(ROOT, 'src', 'i18n.json'), 'utf8'));
  for (const v of variants) buildVariant(v, base, overlay);
}

try {
  main();
} catch (e) {
  console.error(e && e.message ? e.message : e);
  process.exit(1);
}
