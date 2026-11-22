# mos_rain — Realistic Rain & Custom Liquid Map Refraction for EffectCraft

v5.0.0 upgrades `mos_rain` to an **External Effect Helper** (`effectcraft-helper-mos-rain.exe`, Plug-in API 2) so it supports a native **`layer`** picker (`custom_layer`): extract contours and internal shading from any composition layer by **Luminance** or **Alpha**, and drive convex-lens refraction displacement, 3-channel dispersion, dark meniscus contact rims, caustics, and directional specular lighting directly from that custom edge & light map — either solo or blended seamlessly with procedural raindrops.

| Field | Value |
| --- | --- |
| Effect ID | `mos.rain` |
| Name | `mos_rain` (Chinese UI: `mos_雨滴`) |
| Category | `MOS Deform` (Chinese UI: `MOS 形变`) |
| Version | 5.0.0 |
| Author | MOSSLS |
| Interface | Plug-in API 2 (External Effect Helper protocol v1) |
| Parameters | 29 scalar `f64` parameters across 6 groups |

## Build & Install

```powershell
.\build.ps1
```

`build.ps1` compiles the release helper binary to `dist\effectcraft-helper-mos-rain.exe`, installs it into `%APPDATA%\EffectCraft\helpers\`, disables any legacy `%APPDATA%\EffectCraft\Plug-ins\mos_rain.wasm` (moving it to `Plug-ins-disabled\`) to prevent duplicate-ID conflicts, and installs `%APPDATA%\EffectCraft\Scripts\Startup\mos-load-helpers.jsx` for automatic scanning on launch.

- Restart EffectCraft, or click **Effect ▸ Scan for Effect Helpers...** in an already-open window.

## Parameters (29 across 6 groups)

- **Custom Map** (`自定义贴图`): `custom_layer`, `custom_mode` (`Custom only` / `Blend with procedural` / `Procedural only`), `custom_channel` (`Luminance` / `Alpha`), `custom_invert`, `custom_threshold` (0–95%), `custom_bevel` (1–80 px), `custom_disp` (0–300%)
- **Large Drops**: `big_density`, `big_size`, `big_phase`, `big_disp`
- **Small Drops**: `small_density`, `small_size`, `small_phase`, `small_disp`
- **Streaks**: `streak_density`, `streak_size`, `streak_phase`, `streak_disp`
- **Flow & Optics**: `flow_speed`, `displacement`, `dispersion`, `highlight`, `light_angle`, `bg_blur`
- **Output**: `seed`, `sampling`, `edge_mode`, `mix`

## Note on Custom Layer Transforms

EffectCraft's `layer_input` callback supplies the referenced layer's own rendered pixels (with its masks and effects applied) without its composition Position/Scale/Rotation transforms. If you animate the transform of your custom map, precompose it at composition size first.

## Test & Verification

```powershell
python tools/test_helper.py
```

See `tools/engine_check.jsx` for real EffectCraft CLI rendering via `effectcraft-cli run --commands -` (`effect.helpers.scan` + `script.run`).

## License

AGPL-3.0-or-later. See `LICENSE`.
