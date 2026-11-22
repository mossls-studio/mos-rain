# mos_rain — Realistic Rain on Glass for EffectCraft

A complete from-scratch rewrite (v4.0.0) of rainy window glass: drops are gravity-sagged organic shapes with subtle surface-tension asymmetry rather than uniform spheres, acting as miniature convex lenses that invert and bend the background scene with 3-channel chromatic dispersion, thin meniscus dark contact lines, bottom caustic glow and crisp specular highlights. Large drops, small drops and long running streaks each have independent density, size, phase and displacement controls, flowing downward and wiping condensation along their paths.

| Field | Value |
| --- | --- |
| Effect ID | `mos.rain` |
| Name | mos_rain |
| Category | MOS Deform |
| Version | 4.0.0 |
| Author | MOSSLS |
| Plug-in ABI | 1 (`ec_api_version` = 1) |
| Manifest schema | 2 |
| Render parameters | 22 scalar f64 |

## Parameters

22 scalar UI parameters across 5 groups (`Large Drops`, `Small Drops`, `Streaks`, `Flow & Optics`, `Output`):

- **Large Drops**: `big_density` (0–100%), `big_size` (10–200%), `big_phase`, `big_disp` (0–300%)
- **Small Drops**: `small_density` (0–100%), `small_size` (10–200%), `small_phase`, `small_disp` (0–300%)
- **Streaks**: `streak_density` (0–100%), `streak_size` (10–200%), `streak_phase`, `streak_disp` (0–300%)
- **Flow & Optics**: `flow_speed` (0–300%), `displacement` (0–300%), `dispersion` (0–100%), `highlight` (0–300%), `light_angle`, `bg_blur` (0–100%)
- **Output**: `seed` (0–100), `sampling`, `edge_mode`, `mix` (0–100%)

## Build & Test

```powershell
node build.js
node tools/test_wasm.js
node tools/test_wasm.js --card=window
```

## License

AGPL-3.0-or-later. See `LICENSE`.
