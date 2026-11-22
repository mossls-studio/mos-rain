// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 MOSSLS

//! effectcraft-helper-mos-rain: EffectCraft external effect helper providing `mos_雨滴` (`mos.rain`).
//!
//! Speaks EffectCraft external helper protocol v1 (`@ec-helper ` JSON lines over stdin/stdout +
//! premultiplied RGBA f32 frame exchange file) so `mos_雨滴` can read another layer in the
//! composition via `layer_input` and run natively across CPU threads.

mod blur;
mod lang;
mod rain;

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAGIC: &str = "@ec-helper ";
const PROTOCOL_VERSION: u32 = 1;
const HELPER_VERSION: &str = "5.0.0";
const EFFECT_ID: &str = "mos.rain";

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
struct ImageRef {
    offset: u64,
    width: u32,
    height: u32,
    #[serde(default)]
    #[allow(dead_code)]
    origin: [f64; 2],
    #[serde(default = "one")]
    scale: f64,
}

fn one() -> f64 {
    1.0
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
#[allow(dead_code)]
enum Req {
    Hello {
        version: u32,
        frames: String,
    },
    Describe,
    Load {
        path: String,
    },
    Padding {
        effect: String,
        instance: u64,
        time: f64,
        scale: f64,
        layer_size: [f64; 2],
        flat: Vec<f64>,
        #[serde(default)]
        texts: Vec<String>,
    },
    Render {
        effect: String,
        instance: u64,
        time: f64,
        frame: ImageRef,
        layer_size: [f64; 2],
        flat: Vec<f64>,
        #[serde(default)]
        texts: Vec<String>,
        #[serde(default)]
        frame_rate: f64,
        #[serde(default)]
        frame_range: Option<(f64, f64)>,
    },
    Image {
        image: Option<ImageRef>,
    },
    Params {
        #[serde(default)]
        values: Option<(Vec<f64>, Vec<String>)>,
    },
    Destroy {
        instance: u64,
    },
    Ping,
    Shutdown,
}

struct Host {
    out: Arc<Mutex<std::io::Stdout>>,
    rx: Receiver<Result<Req, String>>,
    frames: Option<File>,
    io_buf: Vec<u8>,
    depth: u32,
    cache: Option<LayerCache>,
    layer_failure: Option<String>,
}

/// Cached layer input image, keyed by `(param, layer_id, time, target dimensions)`.
struct LayerCache {
    param: String,
    layer_id: i64,
    time: f64,
    target_w: usize,
    target_h: usize,
    at: Instant,
    width: usize,
    height: usize,
    px: Vec<f32>,
}

fn main() {
    log_line(&format!(
        "start: pid {}, layer timeout {} s, layer cache {} s",
        std::process::id(),
        layer_timeout().as_secs(),
        layer_cache_ttl().as_secs()
    ));
    let mut host = Host {
        out: Arc::new(Mutex::new(std::io::stdout())),
        rx: spawn_reader(),
        frames: None,
        io_buf: Vec::new(),
        depth: 0,
        cache: None,
        layer_failure: None,
    };
    loop {
        let req = match host.next_req() {
            Some(Ok(r)) => r,
            Some(Err(e)) => {
                log_line(&format!("bad request: {e}"));
                continue;
            }
            None => break,
        };
        match req {
            Req::Hello { version, frames } => {
                if version != PROTOCOL_VERSION {
                    host.send(json!({
                        "op": "error",
                        "message": format!("this helper speaks protocol {PROTOCOL_VERSION}, not {version}")
                    }));
                    break;
                }
                match OpenOptions::new().read(true).write(true).open(&frames) {
                    Ok(f) => {
                        host.frames = Some(f);
                        log_line(&format!("hello: frames={frames}"));
                        host.send(json!({
                            "op": "hello",
                            "version": PROTOCOL_VERSION,
                            "name": lang::effect_name(),
                            "helper_version": HELPER_VERSION,
                            "loads": []
                        }));
                    }
                    Err(e) => {
                        host.send(json!({
                            "op": "error",
                            "message": format!("can't open the frame exchange file: {e}")
                        }));
                        break;
                    }
                }
            }
            Req::Describe => {
                let mut manifest = rain::manifest();
                lang::localize_manifest(&mut manifest);
                host.send(json!({
                    "op": "effects",
                    "effects": [{"manifest": manifest, "padding": false}],
                    "errors": []
                }));
            }
            Req::Load { path } => {
                log_line(&format!("load (ignored): {path}"));
                host.send(json!({
                    "op": "effects",
                    "effects": [],
                    "errors": ["this helper does not load files"]
                }));
            }
            Req::Padding { .. } => host.send(json!({"op": "padding", "pixels": 0})),
            Req::Render {
                effect,
                time,
                frame,
                flat,
                ..
            } => {
                if effect != EFFECT_ID {
                    host.send(json!({
                        "op": "error",
                        "message": format!("unknown effect `{effect}`")
                    }));
                    continue;
                }
                match host.render_frame(&frame, time, &flat) {
                    Ok(()) => host.send(json!({"op": "done"})),
                    Err(e) => {
                        log_line(&format!("render error: {e}"));
                        host.send(json!({"op": "error", "message": e}));
                    }
                }
            }
            Req::Image { .. } | Req::Params { .. } => {
                log_line("unexpected image/params reply");
            }
            Req::Destroy { .. } => {}
            Req::Ping => host.send(json!({"op": "pong"})),
            Req::Shutdown => break,
        }
    }
}

impl Host {
    fn send(&self, v: serde_json::Value) {
        if let Ok(mut out) = self.out.lock() {
            let _ = writeln!(out, "{MAGIC}{v}");
            let _ = out.flush();
        }
    }

    fn next_req(&mut self) -> Option<Result<Req, String>> {
        self.rx.recv().ok()
    }

    fn render_frame(&mut self, frame: &ImageRef, time: f64, flat: &[f64]) -> Result<(), String> {
        self.depth += 1;
        let result = self.render_inner(frame, time, flat);
        self.depth -= 1;
        result
    }

    fn render_inner(&mut self, frame: &ImageRef, time: f64, flat: &[f64]) -> Result<(), String> {
        let started = Instant::now();
        let w = frame.width as usize;
        let h = frame.height as usize;
        let mut px = self.read_image(frame)?;
        log_line(&format!(
            "render {}x{} scale={} time={:.3} flat_len={}",
            w,
            h,
            frame.scale,
            time,
            flat.len()
        ));
        let dump = if debug_on() && std::env::temp_dir().join("mos_rain_dump").exists() {
            let dir = std::env::temp_dir().join("mos-rain-frames");
            let _ = std::fs::create_dir_all(&dir);
            dump_pam(&dir.join("in.pam"), w, h, &px);
            Some(dir)
        } else {
            None
        };
        rain::render(self, &mut px, w, h, frame.scale, time, flat)?;
        if let Some(msg) = self.layer_failure.take() {
            log_line(&format!("render failed: {msg}"));
            return Err(msg);
        }
        if let Some(dir) = dump {
            dump_pam(&dir.join("out.pam"), w, h, &px);
        }
        self.write_image(frame, &px)?;
        log_line(&format!(
            "render {w}x{h} done in {} ms",
            started.elapsed().as_millis()
        ));
        Ok(())
    }

    fn read_image(&mut self, r: &ImageRef) -> Result<Vec<f32>, String> {
        let n = r.width as usize * r.height as usize * 4;
        let bytes = n * 4;
        self.io_buf.resize(bytes, 0);
        let file = self.frames.as_mut().ok_or("no frame exchange file")?;
        file.seek(SeekFrom::Start(r.offset))
            .map_err(|e| e.to_string())?;
        file.read_exact(&mut self.io_buf[..bytes])
            .map_err(|e| e.to_string())?;
        let mut out = vec![0.0f32; n];
        for (dst, chunk) in out
            .iter_mut()
            .zip(self.io_buf[..bytes].chunks_exact(4))
        {
            *dst = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        Ok(out)
    }

    fn write_image(&mut self, r: &ImageRef, px: &[f32]) -> Result<(), String> {
        let bytes = px.len() * 4;
        self.io_buf.resize(bytes, 0);
        for (dst, &v) in self.io_buf[..bytes].chunks_exact_mut(4).zip(px.iter()) {
            dst.copy_from_slice(&v.to_le_bytes());
        }
        let file = self.frames.as_mut().ok_or("no frame exchange file")?;
        file.seek(SeekFrom::Start(r.offset))
            .map_err(|e| e.to_string())?;
        file.write_all(&self.io_buf[..bytes])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn layer_timeout_failed(&mut self, param: &str, limit: Duration) {
        log_line(&format!("layer `{param}`: no answer within {} s", limit.as_secs()));
        self.layer_failure = Some(lang::layer_timeout(param, limit.as_secs()));
    }

    fn ask_layer(
        &mut self,
        param: &str,
        layer_id: i64,
        time: f64,
        target_w: usize,
        target_h: usize,
    ) -> Option<(usize, usize, Vec<f32>)> {
        let ttl = layer_cache_ttl();
        if !ttl.is_zero() {
            let hit = self.cache.as_ref().filter(|c| {
                c.param == param
                    && c.layer_id == layer_id
                    && c.time == time
                    && c.target_w == target_w
                    && c.target_h == target_h
                    && c.at.elapsed() < ttl
            });
            if let Some(c) = hit {
                log_line(&format!("layer `{param}`#{layer_id} cached {}x{}", c.width, c.height));
                return Some((c.width, c.height, c.px.clone()));
            }
        }
        let started = Instant::now();
        let limit = layer_timeout();
        self.send(json!({"op": "layer_input", "param": param, "time": time}));
        loop {
            let left = match limit.checked_sub(started.elapsed()) {
                Some(left) if !left.is_zero() => left,
                _ => {
                    self.layer_timeout_failed(param, limit);
                    return None;
                }
            };
            let req = match self.rx.recv_timeout(left) {
                Ok(r) => r,
                Err(RecvTimeoutError::Timeout) => {
                    self.layer_timeout_failed(param, limit);
                    return None;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    self.layer_failure = Some(lang::layer_closed(param));
                    return None;
                }
            };
            let req = match req {
                Ok(r) => r,
                Err(e) => {
                    log_line(&format!("bad request while waiting: {e}"));
                    continue;
                }
            };
            match req {
                Req::Image { image } => {
                    let r = image?;
                    let (w, h) = (r.width as usize, r.height as usize);
                    let px = match self.read_image(&r) {
                        Ok(p) => p,
                        Err(e) => {
                            log_line(&format!("layer image read failed: {e}"));
                            return None;
                        }
                    };
                    log_line(&format!(
                        "layer `{param}`#{layer_id} {w}x{h} scale={} after {} ms",
                        r.scale,
                        started.elapsed().as_millis()
                    ));
                    if !ttl.is_zero() {
                        self.cache = Some(LayerCache {
                            param: param.to_string(),
                            layer_id,
                            time,
                            target_w,
                            target_h,
                            at: Instant::now(),
                            width: w,
                            height: h,
                            px: px.clone(),
                        });
                    }
                    return Some((w, h, px));
                }
                Req::Render {
                    effect,
                    time,
                    frame,
                    flat,
                    ..
                } => {
                    let out = if effect == EFFECT_ID {
                        self.render_frame(&frame, time, &flat)
                    } else {
                        Err(format!("unknown effect `{effect}`"))
                    };
                    match out {
                        Ok(()) => self.send(json!({"op": "done"})),
                        Err(e) => self.send(json!({"op": "error", "message": e})),
                    }
                }
                Req::Padding { .. } => self.send(json!({"op": "padding", "pixels": 0})),
                Req::Ping => self.send(json!({"op": "pong"})),
                Req::Shutdown => return None,
                other => log_line(&format!("ignored while waiting: {other:?}")),
            }
        }
    }
}

impl rain::Layers for Host {
    fn layer_image(
        &mut self,
        param: &str,
        layer_id: i64,
        time: f64,
        target_w: usize,
        target_h: usize,
    ) -> Option<(usize, usize, Vec<f32>)> {
        self.ask_layer(param, layer_id, time, target_w, target_h)
    }
}

fn spawn_reader() -> Receiver<Result<Req, String>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                    break;
                }
            }
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            if tx
                .send(serde_json::from_str::<Req>(t).map_err(|e| format!("{e} in {t}")))
                .is_err()
            {
                break;
            }
        }
    });
    rx
}

fn env_secs(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map_or(default, |v| v as u64)
}

fn layer_timeout() -> Duration {
    Duration::from_secs(env_secs("MOS_RAIN_LAYER_TIMEOUT", 20).max(1))
}

fn layer_cache_ttl() -> Duration {
    Duration::from_secs(env_secs("MOS_RAIN_LAYER_CACHE_SECS", 3))
}

fn log_line(msg: &str) {
    if !debug_on() {
        return;
    }
    let path = std::env::temp_dir().join("effectcraft-mos-rain.log");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{msg}");
    }
}

fn debug_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::temp_dir().join("mos_rain_dump").exists())
}

fn dump_pam(path: &std::path::Path, w: usize, h: usize, px: &[f32]) {
    let mut out = Vec::with_capacity(w * h * 4 + 64);
    out.extend_from_slice(
        format!("P7\nWIDTH {w}\nHEIGHT {h}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n")
            .as_bytes(),
    );
    for i in 0..w * h {
        let a = px[i * 4 + 3].clamp(0.0, 1.0);
        for c in 0..3 {
            let v = if a > 1e-4 { px[i * 4 + c] / a } else { 0.0 };
            out.push((v.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
        out.push((a * 255.0).round() as u8);
    }
    let _ = std::fs::write(path, out);
}
