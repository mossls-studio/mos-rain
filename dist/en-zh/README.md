# mos_rain — Rain on Glass for EffectCraft

Rain on a window, for real: beads are born out of nothing, condense on the glass,
grow heavy and occasionally slide a little; where the water piles up they turn
into long streams with a head in front and a trail behind, running down the
frame; a fine mist appears all at once and fades out slowly. Beads, streams and
mist droplets are all particles in a full-resolution analytic field — overlaps
add up into saddle-shaped puddles, and the rim is a hard contact line of finite
slope. Dark contours, wet highlights, lit-side edge lines and dispersion together
suggest the thickness of water.

A pixel effect: it rewrites the layer's pixels and does not touch transform
properties.

| Field | Value |
| --- | --- |
| Effect ID | `mos.rain` |
| Display name | mos_rain |
| Category | MOS Deform |
| Version | 3.0.0 |
| Author | MOSSLS |
| Plug-in ABI | 1 (`ec_api_version` = 1) |
| manifest schema | 2 (parameter groups) |
| Render params | 22 f64 (19 UI params; the color expands to 4 components) |
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

Each build is a standalone wasm; the manifest is embedded in its data section.

## Parameters

19 UI parameters in four groups. `tint` expands to r / g / b / a, so `ec_render`
receives 22 f64 values; indices 0–21 follow the table order. (The `tint` alpha
component is passed in but not used by the shader.)

### Rain

| # | Param | id | Type | Default | Range | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Refraction % | `amount` | slider | 140 | 0 – 300 | Overall displacement; scaled by each drop's local `s` |
| 2 | Drop size % | `size` | slider | 45 | 0 – 100 | Base radius `rad_nom`, the scale of beads and streams |
| 3 | Density % | `density` | slider | 40 | 0 – 100 | Keep ratio `0.10+0.90×density`, then a 4×4 coarse-grid thinning |
| 4 | Streams % | `streak` | slider | 50 | 0 – 100 | Stream count `2+12×streak`, up to 20 |
| 5 | Stream length % | `trail` | slider | 60 | 0 – 100 | Trail length cap `h×(0.30+0.50×trail)` |
| 6 | Fine mist % | `mist` | slider | 60 | 0 – 100 | Keep ratio of the mist grid; 0 leaves beads and streams only |
| 7 | Flow speed % | `speed` | slider | 20 | 0 – 200 | Trail growth `35+2.2×speed` px/s; the head's pace comes from the cycle |

### Glass

| # | Param | id | Type | Default | Range | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 8 | Dispersion % | `dispersion` | slider | 12 | 0 – 100 | R / B offset (±10 % × dispersion), rainbow at the rim |
| 9 | Highlight % | `highlight` | slider | 95 | 0 – 300 | Wet base glint `^8` + sharp rim `^16` |
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

### Random

| # | Param | id | Type | Default | Range | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 18 | Distribution seed | `seed` | slider | 0 | 0 – 100 (integer) | Every hash is xored with a `seed×1000` salt; a new seed re-rolls the whole rain |
| 19 | Start time s | `time_offset` | slider | 4.0 | 0 – 20 (1 decimal) | Shifts the whole field in time: `t = time + start time`. 0 = frame 0 is empty; 4 s = the rain is already falling on open |

Everything is gated by the local scale field (`s`), so flat glass is never
brightened or blurred. When `amount`, `frost` and `tint_amount` are all 0 and
highlight/rim are off, the effect is skipped and the source is returned
unchanged.

## Usage

- **Rain over the whole frame**: apply to an **adjustment layer** (Layer ▸ New ▸
  Adjustment Layer) or a precomp so the entire composite is refracted. Applied to
  a single layer, only that layer's pixels move; dense beads may be clipped by the
  layer bounds.
- **Default (medium rain)**: 140 / 45 / 40 / 50 / 60 / 60 / 20, start time 4 s.
  Opens on rain that has already been falling for a while.
- **Heavy rain right away**: start time 5–8 s, streams 60–80. Long streams have
  had time to fill the frame while their heads keep moving down.
- **Watch it build from nothing**: start time 0. Frame 0 is empty; over the first
  3 s beads condense one by one, grow, and occasionally slide, and only later do
  stream heads and trails appear.
- **Mist only**: drop size 5–10, density 0, streams 0, fine mist 100. Reads as a
  cold window just starting to fog.
- **Denser tracks**: streams 70–100, stream length 70–100, density 30–50. Give the
  density away to the tracks so beads do not bury them.
- **Frosted glass**: frost 30–70, with highlight/rim pulled back to 40–70. Frost
  flattens the relief and dims the highlights while blurring the water into a
  frosted streak; stay under 50 if you want the track structure to survive.
- **Cold glass**: tint pale blue/cyan, tint_amount 10–25; only the water is tinted.
- **Dispersion** 8–25 % looks natural; above 60 % the green channel starts to
  drift (see Known limits).
- **Another rainfall**: change `seed` to another integer and every bead, stream
  and mist droplet re-rolls its position and lifetime; `start time` picks which
  moment of that rain you are looking at.
- **Edges = transparent** suits transparent layers; with large refraction the
  water samples outside the frame and shows transparent notches. Lower
  `amount` to clean it up.

## Algorithm

**1. Full-resolution analytic field.** Water is only a few pixels wide; half
resolution would blur bead rims and stream cross-sections. The height, gradient
and scale fields all match the frame (`gw = w`, `gh = h`). The field value is* the height — there is no threshold cut. Beads and streams share
`cell = max(8, min(w,h) × 0.058)` and a base radius
`rad_nom = cell × (0.20 + 0.55 × size) × 0.82`; the mist grid is derived from
`cell` separately.

**2. Generative lifecycle.** Every bead, stream and mist droplet has a birth time
and a lifespan: `birth = hash × window`, `age = time + start time − birth`. When
`age < 0` nothing is drawn at all, so `start time = 0` leaves frame 0 as clean
glass, while 4 s opens on rain already in progress. The phase is
`fract(age / life)`, so each drop runs one full lap from birth to rebirth.

**3. Beads.** A jittered grid (spacing `cell`, about ±0.52 cell of jitter), kept
when `hash ≤ 0.10 + 0.90 × density` and additionally thinned by a 4×4 coarse grid
(`cm ≤ 0.30 + 0.70 × density`), so raising density first tightens the spacing,
then closes it up. Radius
`rad_nom × (0.38 + 0.95 × hash) × (0.72 + 0.55 × cm)`, with a 0.92–1.06
horizontal factor. Life is 2.4–6 s: a bead is fully opaque the moment it is born,
starts sliding at `ph > 0.55` (by `r × (0.35–1.30)` pixels), fades once
`ph > 0.82`, then is reborn at its origin. A sliding bead turns into a vertical
teardrop (vertical radius ×1.22–1.44).

**4. Long streams.** `2 + 12 × streak` of them (at most 20), each with its own
birth time and a `4.5–8` s cycle. The head sits at
`y_head = −margin + ph × (h + 2·margin)` with `margin = 40 + 4·rad_nom`, moving
steadily downward; behind it the trail grows to
`min(age × flow_speed, h × (0.30 + 0.50 × trail))`, where
`flow_speed = 35 + 2.2 × speed`. The trail is fixed relative to the head, so the
whole streak truly translates downward instead of swinging in place. During the
first 1/9 of the cycle the head grows from zero (`grow`): first a small bead,
and only once it has absorbed enough water does it drag a trail. The head is a
vertical teardrop (0.88× radius across, 1.14× down), and 3 bead nodes sit on the
trail; all of them move down with the head.

**5. Mist.** Grid `cell_m = max(3, cell × 0.16)`, about ±0.48 cell of jitter;
radius `cell_m × (0.09 + 0.25 × hash)` (roughly 1–3 px), life 1.6–4.6 s. A mist
droplet is fully opaque when born, then fades linearly with `1 − ph` (nothing is
drawn past `ph` ≈ 0.92). Mist does not join the additive fusion of beads and
streams: it is blended per pixel after the main height field is built, and
positions that already hold water (height > 0.40 or scale > 3 px) are skipped so
no jaggies appear on the hard rims.

**6. Fusion and hard edges.** Beads, streams and trail nodes all write into one
field with `+=`, so overlapping heights add up and two bodies of water bulge into
a saddle-shaped puddle. Every bead is a paraboloid dome `(1 − q²)` of finite
slope: it stays curved from the center all the way to the contact line, which
keeps a slope at the rim — the contact line is therefore a **hard edge**, with no
"soft ramp plus a single cut". The field is finally clamped to 1.15. Beads that
sit in a stream band whose head has already passed are skipped (they are taken
away by the stream), so beads near a track do not compete with it.

**7. Scale field and refraction.** Each write also records a scale (max): mist
stores its radius, beads their long semi-axis, streams `1.5 × half-width`, stream
heads their head radius. At render time, per pixel:

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
dispersion ≤ 5 the second sample is skipped. The chromatic samples are only a few
pixels apart, so they stay bilinear — bicubic is reserved for the main sample.

**9. Frost.** The source is downsampled 2×, blurred with a separable box pass
(radius `frost × 0.035 × min_dim × 0.5`), then sampled at the refracted
coordinate and blended into the sharp result. The mix is gated by the local
`gate`, so only water goes soft. Frost also scales highlight/rim by
`1 − 0.75 × frost` and normals by `1 − 0.35 × frost`, flattening the relief.

**10. Lighting.**

- **Dark contour**: once the steep-normal term `n2` passes 0.90 it is darkened
  with a smoothstep (less so toward the light), giving beads a black edge and
  volume instead of a ring of white bubbles. It is gated too, so mist does not
  turn into a dark film.
- **Highlight**: Blinn half-vector, `^8` for the wet base and `^16` for the sharp
  rim, added and scaled by `highlight`. Slope distance is deliberately not used,
  which avoids a single needle-point glint in one direction.
- **Lit-side edge line**: slopes with `n2 > 1.60` multiplied by
  `clamp01(2.2 × half-vector)` — cylindrical streams have no y normal, so plain
  Blinn never lights them.
- **Fresnel**: Schlick's fifth power weighted by `0.4 + 1.2 × half-vector`, just
  enough directionality, no full white ring.
- Added light is attenuated by background luminance (`1 − 0.60 × luma`) so bright
  areas do not burn to pure white.

**11. Output.** Tint applies to water only, mixed by `gate`; RGB is clamped to
`[0, a]` in premultiplied alpha, then blended with the source by `mix`.

## Performance

The host runs plug-ins in the **wasmi interpreter**; the same kernel is about an
order of magnitude faster in Node/V8. Numbers below are V8 (Node 22, Windows 11),
one `ec_render` call at 960×540 / 1920×1080, excluding PNG encoding and
compositing:

| Resolution | Default | Dense / big beads |
| --- | --- | --- |
| 960×540 | 60–70 ms | 75–95 ms |
| 1920×1080 | 230–270 ms | — |

("Dense / big beads" = high `streak` + high `density`, or `size` above 70.)

Per-pixel cost comes mainly from two places: two texture samples per pixel when
dispersion is on, and building the full-resolution height field plus its
gradients (splat coverage grows with particle count). Dispersion 0 saves a
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
tools/test_wasm.js     fake host: ABI, 8 checks, 15 cases, per-frame timing
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
3. manifest carries 19 UI params;
4. `mix = 0` returns the source byte-for-byte;
5. rendering the same params twice is byte-identical;
6. default params change > 20 % of pixels;
7. `streak` from 50 to 0 visibly changes the image;
8. all effect amounts 0 returns the source.

Fourteen render cases in total (default, dense, round beads, all streams, long
streams, mist only, frost, bicubic, no light, transparent edges, plus 4 motion
frames at `time` = 0 / 0.5 / 1.0 / 1.5 s), written to `tests/out/`.

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

- Each particle is an analytic function of time, not a stateful fluid; "absorbing"
  only means beads the stream passes over are skipped — the head does not actually
  gain volume.
- Beads are reborn at their origin when their lifecycle ends; at very slow
  playback the loop becomes visible.
- Stream bands are analytic ridges; forking is not implemented yet (v3
  prioritises flow and fusion).
- Hard edges come from finite-slope domes; at extreme `amount` the rim
  displacement can leave a sampling gap of about 1 px.
- Dispersion takes two samples; above 60 % the green channel is an average of the
  two and colour drifts slightly.
- `Edges = transparent` with large displacement samples outside the frame and
  leaves notches at the water's edge.

---

## License

AGPL-3.0-or-later (GNU Affero General Public License, version 3 or later). See
`LICENSE` in the repository root.
