# mos_rain — Rain on Glass for EffectCraft

Rain on a window, for real: a fine mist underfoot, beads of many sizes pressed
against the glass, and a handful of long streams winding down the frame. Streams
swell and pinch along their length, occasionally fork, and carry larger bead
nodes that fuse with the main run into uneven tracks. All three live in a
full-resolution height field as analytic surfaces, merged by screen blending;
dark contours, wet highlights, lit-side edge lines and dispersion together
suggest the thickness of water.

A pixel effect: it rewrites the layer's pixels and does not touch transform
properties.

| Field | Value |
| --- | --- |
| Effect ID | `mos.rain` |
| Display name | mos_rain |
| Category | MOS Deform |
| Version | 2.1.0 |
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

Each wasm is about 28 KB; the manifest is embedded in the data section.

## Parameters

17 UI parameters in three groups. `tint` expands to r / g / b / a, so `ec_render`
receives 20 f64 values; indices 0–19 follow the table order. (The `tint` alpha
component is passed in but not used by the shader.)

### Rain

| # | Param | id | Type | Default | Range | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Refraction % | `amount` | slider | 120 | 0 – 300 | Overall displacement; scaled per drop |
| 2 | Drop size % | `size` | slider | 45 | 0 – 100 | Base radius `rad_nom`, sets the overall scale |
| 3 | Density % | `density` | slider | 50 | 0 – 100 | Keep ratio `0.10+0.90×density`, then coarse-grid thinning |
| 4 | Streams % | `streak` | slider | 50 | 0 – 100 | Stream count `2+12×streak`, up to 14 |
| 5 | Stream length % | `trail` | slider | 60 | 0 – 100 | Stream length, 62 %–100 % of frame height |
| 6 | Fine mist % | `mist` | slider | 60 | 0 – 100 | Keep ratio of the mist grid; 0 leaves beads and streams only |
| 7 | Flow speed % | `speed` | slider | 20 | 0 – 200 | Advance of the stream width/height texture; beads stay put |

### Glass

| # | Param | id | Type | Default | Range | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 8 | Dispersion % | `dispersion` | slider | 12 | 0 – 100 | R / B offset (±10 % × dispersion) |
| 9 | Highlight % | `highlight` | slider | 95 | 0 – 300 | Wet base glint + sharp rim, `^8` and `^32` |
| 10 | Rim light % | `rim` | slider | 40 | 0 – 300 | Lit-side edge line + a little directional Fresnel |
| 11 | Light angle | `light_angle` | angle | 135 | 0 – 360 | 0° = right, 90° = up (screen space) |
| 12 | Frost % | `frost` | slider | 0 | 0 – 100 | Blurs the water only; background stays sharp |
| 13 | Tint | `tint` | color | pale blue [0.72, 0.88, 1.0, 1.0] | — | Tints light transmitted through water |
| 14 | Tint amount % | `tint_amount` | slider | 0 | 0 – 100 | Tint mix |

### Output

| # | Param | id | Type | Default | Notes |
| --- | --- | --- | --- | --- | --- |
| 15 | Sampling | `sampling` | popup | 0 = bilinear | Bilinear / bicubic / nearest; bicubic only for the main sample |
| 16 | Edges | `edge_mode` | popup | 0 = stretch | Stretch / repeat / mirror / transparent |
| 17 | Mix % | `mix` | slider | 100 (0 – 100) | Blend with the source |

Everything is gated by the local scale field (`s`), so flat glass is never
brightened or blurred. When `amount`, `frost` and `tint_amount` are all 0 and
highlight/rim are off, the effect is skipped and the source is returned
unchanged.

## Usage

- **Rain over the whole frame**: apply to an **adjustment layer** (Layer ▸ New ▸
  Adjustment Layer) or a precomp so the entire composite is refracted. Applied to
  a single layer, only that layer's pixels move; dense beads may be clipped by the
  layer bounds.
- **Medium rain (default)**: 120 / 45 / 50 / 50 / 60 / 60 / 20. Works for most shots.
- **Dense tracks, like the reference photo**: streams 75–100, stream length 70–100,
  drop size 35–50, density 50–70. Streams cross the frame and bead nodes string
  into uneven water lines; keep density near 50 so beads do not bury the tracks.
- **Mist only**: drop size 5–10, density 0, streams 0, fine mist 100,
  amount 80–110. Reads as a cold window just starting to fog.
- **Big beads**: drop size 65–85, density 55–70, streams 20–35. Close-up macro,
  with clean glass between beads.
- **Frosted glass**: frost 30–70, with highlight/rim pulled back to 40–70. Frost
  flattens the relief and dims the highlights while blurring the water into a
  frosted streak; stay under 50 if you want the track structure to survive.
- **Cold glass**: tint pale blue/cyan, tint_amount 10–25; only the water is tinted.
- **Dispersion** 8–25 % looks natural; above 60 % the green channel starts to
  drift (see Known limits).
- **Frozen frame**: speed 0. The default 20 only lets stream width breathe
  slowly; beads never translate.
- **Edges = transparent** suits transparent layers; with large refraction the
  water samples outside the frame and shows transparent notches. Lower
  `amount` to clean it up.

## Algorithm

**1. Full-resolution height field.** Water is 1–20 px wide; half resolution would
blur bead rims and stream cross-sections. The height, gradient and scale fields
all match the frame (`gw = w`, `gh = h`). All three layers share
`cell = max(8, min(w,h) × 0.058)` and a base radius
`rad_nom = cell × (0.20 + 0.55 × size) × 0.82`.

**2. Screen blending.** Every layer writes with `old + v − old·v`: two
overlapping beads fuse into a fuller puddle, a stream meeting a bead swells at
the junction, and no hard seam is left behind. The formula also keeps height
inside 0–1 independent of draw order.

**3. Mist.** A jittered grid with `cell_m = max(3, cell × 0.18)` and about ±0.48
cell of jitter; radius `cell_m × (0.09 + 0.26 × hash)` (roughly 1–4 px), height
0.45–0.80, kept by `mist`. Mist droplets are round domes and record their own
radius in the scale field.

**4. Beads.** The same jittered grid (spacing `cell`), kept when
`hash ≤ 0.10 + 0.90 × density` and additionally thinned by a 4×4 coarse grid
(`cm ≤ 0.30 + 0.70 × density`), so raising density first tightens the spacing,
then closes it up. Radius
`rad_nom × (0.38 + 0.95 × hash) × (0.72 + 0.55 × cm)`, with a 0.92–1.08
horizontal factor. 25 % of them become vertical teardrops (vertical radius
×1.22–1.44).

**5. Streams.** `2 + 12 × streak` of them (at most 14), length
`h × (0.62 + 0.38 × trail)`; the start may sit above the frame, so some streams
are cut short. The centerline sums two sines:
`x = x0 + amp × (0.62·sin(0.017y + φ) + 0.38·sin(0.043y + 1.7φ))` with
`amp = rad_nom × (0.5 + 1.4 × hash)` — the slow term draws the big meander, the
fast one the wiggle. Half-width is `rad_nom × (0.40 + 0.35 × hash)`, modulated
along the run by two more sines (frequencies `0.023` and `0.0065`) down to
22 %–100 %. The cross-section is a semicircle `sqrt(1 − q²)`, not a dome: the
edges are steeper, which gives a crisp dark contour and a lit edge. Height is
normalized to 0.85–1.0 in the same field units as beads, which lets the isosurface
join a bead and a stream into one track; a 0.82–1.0 ripple along the run makes
streams thicken and thin, rise and fall. The cross-section is a paraboloid
`1 − q²`.

**6. Bead nodes and forks.** A bead grows on the stream every 42–97 px (about
75 % of the time), with a radius 1.15–1.90× the local half-width, stretched
1.12× vertically; field accumulation turns the run into a “thick–thin–thick” track
instead of a constant band. About 55 % of streams longer than 80 px fork: a
branch leaves the main run, drifts sideways at `0.32 × distance travelled`,
starts at 55 % of the width and tapers to 35 %, forming the Y-shaped network of
the reference photo. Another 2–5 isolated large beads land at random.

**7. Local scale field.** Each write also records a scale (max): mist stores its
radius, beads their long semi-axis, streams `1.5 × half-width`, branches
`1.4 × half-width`. At render time, per pixel:

- gate `gate = clamp01(0.6s)`;
- displacement `d = 0.30 · (amount/100) · s² · ∇h` (times `1 − 0.35 × frost`);
- normal `n = normalize(−∇h · 0.6s, 1)`;
- displacement clamp `dmax = 2.2s`.

Refraction scales with `s²` and normals with `s`, so a 2 px mist droplet only
disturbs its own neighborhood while a 15 px bead gets the displacement of a
thick lens. A single global scale would twist the background into halos around
every tiny droplet.

**8. Sampling and dispersion.** The displaced sample follows `sampling`. With
dispersion, R uses `1 − ca` of the displacement and B uses `1 + ca`
(`ca = 0.10 × dispersion / 100`), with G the average of the two samples; at
dispersion ≤ 5 the second sample is skipped. The chromatic samples are only a few pixels apart,
so they stay bilinear — bicubic is reserved for the main sample.

**9. Frost.** The source is downsampled 2×, blurred with a separable box pass
(radius `frost × 0.035 × min_dim × 0.5`), then sampled at the refracted
coordinate and blended into the sharp result. The mix is gated by the local
`gate`, so only water goes soft. Frost also scales highlight/rim by
`1 − 0.75 × frost` and normals by `1 − 0.35 × frost`, flattening the relief.

**10. Lighting.**

- **Dark contour**: once the slope term `n2` (squared tangential normal) passes
  0.90, smoothstep darkens it, giving beads a black edge and volume instead of a
  ring of white bubbles. It is gated too, so mist does not turn into a dark film.
- **Highlight**: Blinn half-vector, `^8` for the wet base and `^32` for the sharp
  rim, added and scaled by `highlight`. Slope distance is deliberately not used,
  which avoids a single needle-point glint in one direction.
- **Lit-side edge line**: slopes with `n2 > 1.60` multiplied by
  `clamp01(2.2 × half-vector)` — cylindrical streams have no y normal, so plain
  Blinn never lights them.
- **Fresnel**: Schlick's fifth power weighted by `0.4 + 1.2 × half-vector`, just
  enough directionality, no full white ring.
- Added light is attenuated by background luminance (`1 − 0.60 × luma`) so bright
  areas do not burn to pure white.

**11. Animation.** `drift = time × speed × 0.6` only feeds the stream width
function, so width and height texture creep along; bead positions are fixed and
the frame never slides as a whole. Speed 0 freezes it.

**12. Output.** Tint applies to water only; RGB is clamped to `[0, a]` in
premultiplied alpha, then blended with the source by `mix`.

## Performance

The host runs plug-ins in the **wasmi interpreter**; the same kernel is about an
order of magnitude faster in Node/V8. Numbers below are V8 (Node 22, Windows 11),
one `ec_render` call at 960×540 / 1920×1080, excluding PNG encoding and
compositing:

| Resolution | Default | Dense / big beads | Mist only | Frost 65 % |
| --- | --- | --- | --- | --- |
| 960×540 | 56–70 ms | 70–95 ms | ~55 ms | 90–110 ms |
| 1920×1080 | 227–260 ms | ~320 ms | — | — |

(“Dense / big beads” = high `streak` + high `density`, or `size` above 70.)

Per-pixel cost comes mainly from two places: two texture samples per pixel when
dispersion is on, and building the three-layer height field plus its gradients at
full resolution (splat coverage grows with bead count). Dispersion 0 saves a
sample; highlight/rim both 0 skips the whole lighting block; frost adds a
half-res blur and one more sample and is the most expensive option.

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
tools/test_wasm.js     fake host: ABI, 8 checks, 10 cases, per-frame timing
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

Ten render cases in total (default, dense, round beads, all streams, long
streams, mist only, frost, bicubic, no light, transparent edges), written to
`tests/out/`.

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

- Streams are analytic ridges in a 2D height field, not a fluid sim. Forks, bead
  nodes and width variation are procedural; watch the same patch long enough and
  the repetition shows.
- Beads are elliptical domes merged into irregular puddles by field accumulation
  and a soft isosurface; there is no surface-tension or fluid simulation, so they
  never truly stretch
  into irregular puddles or filaments.
- Very dense streams overlap into a single sheet; lower `density` when `streak`
  is high and let the tracks carry the frame.
- Dispersion takes two samples; above 60 % the green channel is an average of the
  two and colour drifts slightly.
- `Edges = transparent` with large displacement samples outside the frame and
  leaves notches at the water's edge.
- Frost is a half-res blur mix, not a BRDF; high `frost` erases tracks and bead
  detail alike.
- Lighting is additive; on a near-white background the rim approaches 1.0 but the
  whole drop is not overexposed.
- Per-pixel cost is high; 1080p real-time preview is unrealistic (see above).

---

## License

AGPL-3.0-or-later (GNU Affero General Public License, version 3 or later). See
`LICENSE` in the repository root.
