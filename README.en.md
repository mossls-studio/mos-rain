# mos_rain — Rain on Glass for EffectCraft

Rain on a window, for real: a fine mist of tiny droplets, beads of mixed sizes,
and a few drops stretched by gravity into thin, tapering tracks that slide slowly
down the glass. Every drop is a dome in a full-resolution height field, fused by
screen blending; slopes fade to zero at the rim, so the background is never torn.

A pixel effect: it rewrites the layer's pixels and does not touch transform
properties.

| Field | Value |
| --- | --- |
| Effect ID | `mos.rain` |
| Display name | mos_rain |
| Category | MOS Deform |
| Version | 1.0.0 |
| Author | MOSSLS |
| Plug-in ABI | 1 (`ec_api_version` = 1) |
| manifest schema | 2 (parameter groups) |
| Render params | 20 f64 (17 UI params; color expands to 4 components) |
| Platform | EffectCraft desktop, effectcraft-cli |

## Install

The plug-in is a single file, `mos_rain.wasm`, with no dependencies.

1. **Install (recommended)**: Effect ▸ Install Effect Plug-in… and pick
   `dist\zh-zh\mos_rain.wasm` (or `dist\mos_rain\mos_rain.wasm`). It is copied to
   `%APPDATA%\EffectCraft\Plug-ins` and loaded on every launch.
2. **Temporary**: Effect ▸ Load Effect Plug-in… and pick the `.wasm`; lasts for
   the current session only.
3. **CLI**:
   `effectcraft-cli script --eval "app.run('effect.plugins.load', {path: '…\\mos_rain.wasm'})"`

Uninstall = delete `Plug-ins\mos_rain.wasm`. A plug-in with the same name is not
overwritten; remove the old file before changing versions.

Four language packs are built:

```
dist/zh-zh/mos_rain.wasm   Chinese name + Chinese params (default)
dist/zh-en/mos_rain.wasm   Chinese name + English params
dist/en-zh/mos_rain.wasm   English name + Chinese params
dist/en-en/mos_rain.wasm   English name + English params
dist/mos_rain/             compatibility alias (= zh-zh)
```

Each wasm is about 24.9 KB; the manifest is embedded in the data section.

## Parameters

17 UI parameters in three groups. `tint` expands to r / g / b / a, so `ec_render`
receives 20 f64 values; indices 0–19 follow the table order.

### Drops

| # | Param | id | Type | Default | Range | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Refraction % | `amount` | slider | 120 | 0 – 300 | Overall displacement |
| 2 | Drop size % | `size` | slider | 50 | 0 – 100 | Main drop radius relative to the grid |
| 3 | Density % | `density` | slider | 48 | 0 – 100 | Number of main drops; high values merge into puddles |
| 4 | Streaks % | `streak` | slider | 30 | 0 – 100 | Fraction of drops stretched by gravity |
| 5 | Trail length % | `trail` | slider | 40 | 0 – 100 | Trail length relative to drop radius |
| 6 | Fine mist % | `mist` | slider | 50 | 0 – 100 | Density of 2–4 px droplets |
| 7 | Flow speed % | `speed` | slider | 25 | 0 – 200 | Sliding speed of streaks; 0 = frozen |

### Glass

| # | Param | id | Type | Default | Range | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 8 | Dispersion % | `dispersion` | slider | 12 | 0 – 100 | R / B refraction difference |
| 9 | Highlight % | `highlight` | slider | 110 | 0 – 300 | Wet specular + sharp glint |
| 10 | Rim light % | `rim` | slider | 50 | 0 – 300 | Fresnel rim, lit side only |
| 11 | Light angle | `light_angle` | angle | 135 | 0 – 360 | 0° = right, 90° = up (screen space) |
| 12 | Frost % | `frost` | slider | 0 | 0 – 100 | Blurs the drops only; background stays sharp |
| 13 | Tint | `tint` | color | pale blue [0.72, 0.88, 1.0, 1.0] | — | Tints transmitted light in drops |
| 14 | Tint amount % | `tint_amount` | slider | 0 | 0 – 100 | Tint mix |

### Output

| # | Param | id | Type | Default | Notes |
| --- | --- | --- | --- | --- | --- |
| 15 | Sampling | `sampling` | popup | 0 = bilinear | Bilinear / bicubic / nearest; bicubic only for the main sample |
| 16 | Edges | `edge_mode` | popup | 0 = stretch | Stretch / repeat / mirror / transparent |
| 17 | Mix % | `mix` | slider | 100 (0 – 100) | Blend with the source |

Parameters act only on drops: highlight, frost, tint and dispersion are gated by
the local drop mask, so flat glass is neither brightened nor blurred. When
`amount`, `frost` and `tint_amount` are all 0 and highlight/rim are off, the
effect is skipped and the source is returned unchanged.

## Usage

- **Rain over the whole frame**: apply to an **adjustment layer** (Layer ▸ New ▸
  Adjustment Layer) or a precomp so the entire composite is refracted. Applied to
  a single layer, only that layer's pixels move; dense drops may be clipped by the
  layer bounds.
- **Drizzle**: size 35–45, density 25–40, streak 15–25, trail 25–35, mist 70–90,
  speed 10–20, amount 90–110.
- **Medium rain (default)**: 120 / 50 / 48 / 30 / 40 / 50 / 25.
- **Downpour**: amount 150–190, size 65–80, density 80–95, streak 55–70,
  trail 55–70, mist 80–100, speed 60–90. Most of the frame is covered; good for a
  “looking through a rainy window” shot.
- **Mist only**: size 5–12, density 0, streak 0, mist 100, amount 80–110,
  highlight 120–160.
- **Long tracks**: streak 70–100, trail 80–100, speed 40–70.
- **Frosted glass**: frost 30–70, highlight/rim 40–70 so blurred drops do not keep
  a razor-sharp rim.
- **Cold glass**: tint pale blue/cyan, tint_amount 10–25; only the drops are
  tinted.
- **Dispersion** 8–25 % looks natural; above 45 % is a stylized prism.
- **Edges = transparent** suits transparent layers; with very large refraction the
  drops may sample outside the frame and show transparent notches. Keep amount
  below 150 in that case.

## Algorithm

**1. Full-resolution height field.** Drops are only a few pixels wide; half
resolution would blur their rims, so the height field matches the frame. Main
drops sit on a jittered grid: `cell = max(8, min(w,h) × 0.065)`, each cell kept by
a hash threshold `0.15 + 0.85 × density`. Radius
`r = cell × (0.18 + 0.50 × size) × (0.42 + 1.05 × hash) × (0.75 + 0.55 × block factor)`, times a 0.82 tightening
factor so the default 55 % reads as medium rain instead of a sheet of bubbles.

**2. Domes.** Each drop is an axis-aligned ellipse with normalized radius `q` and
height `v = (1 − q²)(1 − q⁸) × vh`. The center keeps its slope (magnifying the
background) while the rim slope fades to zero, so the boundary never tears.
Drops are accumulated with screen blending `old + v − old·v`; overlapping drops
merge into a fuller puddle instead of leaving a hard seam.

**3. Mist.** A denser jittered grid (about 0.22× the main cell), radius 1–3 px,
height 0.35–0.7, kept by `mist`. Their curvature is high, but the local-scale
field keeps their displacement near themselves, so they read as fine mist rather
than wide ripples.

**4. Streaks.** Drops with `hash < streak` are stretched: the head ellipse grows
1.15–1.4× vertically and a track is drawn above it. The trail is not a chain of
ellipsoids: each row takes the centerline and half-width, uses the same dome as a
cross-section, and tapers width and height to 0 with `sqrt(1 − t)`, plus a gentle
sin sway. The gradient is therefore one continuous film, not a string of beads.
The head is drawn last and covers the trail root.

**5. Sliding.** `slide = time × speed × 0.5 × (0.5 + hash)` pixels; head and trail
move together, wrapping at `h + 12r` back to 6r above the frame. Round drops only
drift by up to 0.6 px.

**6. Gradients and local scale.** The height field is replaced in place by
`∂h/∂x` / `∂h/∂y`. During splatting each drop writes its own radius into a second
`strength` field (max). At render time:

- displacement `d = 0.30 · s² · (amount/100) · ∇h`, where `s` is the local radius;
- normal `n = normalize(−∇h · 0.6s, 1)`;
- displacement clamp `dmax = 2.2s`.

A 2 px mist droplet only disturbs its own neighborhood; a 15 px bead gets the
displacement of a thick lens. A single global scale would make tiny droplets twist
the background into 10 px halos and turn the frame into bubble wrap.

**7. Sampling and dispersion.** The displaced sample follows `sampling`. With
dispersion, R uses `1 − ca` of the displacement and B uses `1 + ca`; G is the
average of the two samples. The two chromatic samples are only a few pixels apart,
so they stay bilinear; bicubic is reserved for the main sample.

**8. Frost.** The source is downsampled 2×, blurred with four box passes
(prefix sums + fractional window edges), then sampled at the refracted coordinate
and blended into the sharp result. The blend is gated by the drop mask, so only
drops go soft. Frost also lowers highlight/rim and flattens the normals.

**9. Lighting.**

- Sharp glint: distance in slope space from the ideal reflection direction,
  smoothed into a few-pixel wet highlight;
- Fresnel: Schlick's fifth power, lit side only;
- Rim ring: normal squared `n² > 0.85`, controlled by `rim`;
- A darker contour on the shadow side gives drops volume on bright backgrounds;
- Tiny mist droplets get a small scattering lift so they do not vanish on dark
  glass;
- Added light is attenuated by background luminance (`1 − 0.68·luma`) so bright
  areas do not burn to pure white.

**10. Output.** Tint is applied to drops only; RGB is clamped to `[0, a]` in
premultiplied alpha, then blended with the source by `mix`.

## Performance

The host runs plug-ins in the **wasmi interpreter**; the same kernel is about an
order of magnitude faster in Node/V8. Numbers below are V8 (Node 22.22.2, Windows
11), one `ec_render` call, median of 5 runs, on a 960×540 / 1920×1080 photo,
excluding compositing and PNG encoding:

| Resolution | Default | Dense | All streaks | Mist only | Frost 65% | Bicubic | No light |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 960×540 | 87 ms | 95 ms | 77 ms | 63 ms | 89 ms | 58 ms | 30 ms |
| 1920×1080 | 272 ms | 268 ms | 244 ms | 218 ms | 321 ms | 246 ms | 128 ms |

(“All streaks” = `streak 100 / trail 55`; “Mist only” = `size 8 / density 0 /
mist 100`; no light = `highlight 0 / rim 0 / dispersion 0`.)

Frost is the most expensive feature (half-res four-pass blur + one extra sample).
Dispersion = 0 saves a sample; highlight/rim both 0 skips the whole lighting block
and is about 3× faster. The full-resolution height field is the main cost, but it
only splats — no per-pixel trigonometry.

**Preview advice.** 1080p full resolution through an interpreter is not real-time.
Drop the playback resolution to **50 %** (1/4 of the pixels) first; then disable
frost, set dispersion to 0, or switch sampling back to bilinear.

## Layout

```
src/lib.rs             Rust no_std kernel (ABI 1)
src/kit.rs             shared kernel: arena, math, sampling, blur, gradients (frozen)
src/manifest.json      params and metadata (injected into wasm at build time)
src/i18n.json          four-language overlay
build.js               localized manifest → cargo build → dist
tools/test_wasm.js     fake host: ABI, self-checks, 10 renders, per-frame timing
tools/engine_check.jsx engine smoke test: load → apply → 3 images
dist/                  installable artifacts
tests/assets/          photo-*.rgba reference frames (git-ignored)
tests/out/             renders and zoom crops (git-ignored)
```

## Development

```powershell
node build.js                  # build all 4 language variants
node build.js --variant=zh-zh  # build one
node tools\test_wasm.js        # 960×540 renders + timing → tests\out
node tools\test_wasm.js --w=1920 --h=1080
node tools\test_wasm.js --wasm=dist\en-en\mos_rain.wasm
```

Reference photo: convert an image to rgba8 at
`tests/assets/photo-960x540.rgba`, then run
`node tools/test_wasm.js --card=photo`; output goes to `tests/out/photo/`.

```bash
ffmpeg -i in.png -vf "scale=960:540:force_original_aspect_ratio=increase,crop=960:540" \
  -pix_fmt rgba -f rawvideo tests/assets/photo-960x540.rgba
```

Self-checks (8):

1. wasm has no imports and ABI = 1;
2. manifest id = `mos.rain`;
3. manifest carries 17 UI params;
4. `mix = 0` returns the source byte-for-byte;
5. rendering the same params twice is byte-identical;
6. default params change > 20 % of pixels;
7. `streak` from 35 to 0 visibly changes the image;
8. all effect amounts 0 returns the source.

Compile check (no dist, verifies no_std and no warnings):

```bash
cargo fmt
MOS_MANIFEST_FILE=$PWD/src/manifest.json \
  cargo build --release --target wasm32-unknown-unknown --target-dir build/target
```

Engine smoke test (needs the EffectCraft CLI; not run by this repository):

```bash
effectcraft-cli script tools/engine_check.jsx
```

Output goes to `tests/out/engine/`: `rain-default.png`, `rain-streak.png`,
`rain-mist.png`.

## Known limits

- Refraction is a 2D height-field gradient approximation — no ray tracing, total
  internal reflection or environment reflection. Thickness is suggested by the
  dome, ring, highlight and dispersion.
- Drop size is stylized. The default 55 % at 960×540 is close to macro; for a
  true window scale, lower `size` to 30–40.
- At high density, screen blending saturates overlapping drops into a plateau;
  that is the intended puddle look, but extreme `density 100` + `size 100` becomes
  one large sheet.
- Streak trails are analytic height ridges, not a fluid sim; very long trails
  (`trail 100`) can feel slightly regular, and the sway is a single sine.
- Mist is only 1–3 px at 960×540 and may read as noise after heavy compression;
  raise the render resolution or lower `mist`.
- Frost is a half-res blur mix, not a BRDF; high `frost` erases drop detail.
- `Edges = transparent` with large displacement can sample outside the frame and
  produce transparent notches.
- Per-pixel cost is high; 1080p real-time preview is unrealistic (see above).
- Lighting is additive; on a near-white background the rim approaches 1.0 but the
  whole drop is not overexposed.

---

## License

AGPL-3.0-or-later (GNU Affero General Public License, version 3 or later). See
`LICENSE` in the repository root.
