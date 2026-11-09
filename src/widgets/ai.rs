// Uso de IA: Claude Code (limites do plano e tokens), Codex e OpenCode.
// Tudo lido dos arquivos locais de cada ferramenta, numa thread de fundo,
// no máximo uma vez por minuto e só enquanto o widget existe.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use super::*;
use crate::ui::*;

/// "2026-11-09T13:00:00.123Z" (ou com fuso) → segundos UNIX
pub fn parse_iso(s: &str) -> Option<i64> {
    let s = s.trim();
    let y: i32 = s.get(0..4)?.parse().ok()?;
    let mo: u32 = s.get(5..7)?.parse().ok()?;
    let d: u32 = s.get(8..10)?.parse().ok()?;
    let h: i64 = s.get(11..13)?.parse().ok()?;
    let mi: i64 = s.get(14..16)?.parse().ok()?;
    let se: i64 = s.get(17..19).and_then(|x| x.parse().ok()).unwrap_or(0);
    let mut ts = days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + se;
    let tail = &s[19.min(s.len())..];
    if let Some(p) = tail.rfind(['+', '-']) {
        let tz = &tail[p..];
        if tz.len() >= 6 {
            let sign = if tz.starts_with('-') { -1 } else { 1 };
            let th: i64 = tz.get(1..3).and_then(|x| x.parse().ok()).unwrap_or(0);
            let tm: i64 = tz.get(4..6).and_then(|x| x.parse().ok()).unwrap_or(0);
            ts -= sign * (th * 3600 + tm * 60);
        }
    }
    Some(ts)
}

fn ts_any(v: &Value) -> Option<i64> {
    if let Some(n) = v.as_i64() {
        return Some(if n > 100_000_000_000 { n / 1000 } else { n });
    }
    if let Some(f) = v.as_f64() {
        return Some(f as i64);
    }
    v.as_str().and_then(parse_iso)
}

fn files_recursive(dir: &Path, ext: &str, since: i64, out: &mut Vec<(PathBuf, u64, i64)>, depth: u32) {
    if depth > 6 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(md) = e.metadata() else { continue };
        let p = e.path();
        if md.is_dir() {
            files_recursive(&p, ext, since, out, depth + 1);
        } else if p.extension().map(|x| x.to_string_lossy().eq_ignore_ascii_case(ext)).unwrap_or(false) {
            let mt = md.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
            if mt >= since {
                out.push((p, md.len(), mt));
            }
        }
    }
}

/// Cache incremental de JSONL: só lê o que foi acrescentado.
#[derive(Default)]
pub struct JsonlCache {
    files: HashMap<PathBuf, (u64, Vec<Value>)>,
}

impl JsonlCache {
    fn read(&mut self, path: &Path, len: u64, keep: &dyn Fn(&Value) -> Option<Value>) -> &Vec<Value> {
        let entry = self.files.entry(path.to_path_buf()).or_insert((0, vec![]));
        if len < entry.0 {
            *entry = (0, vec![]);
        }
        if len > entry.0 {
            if let Ok(mut f) = std::fs::File::open(path) {
                if f.seek(SeekFrom::Start(entry.0)).is_ok() {
                    let mut buf = String::new();
                    if f.read_to_string(&mut buf).is_ok() {
                        // só linhas completas
                        let complete = buf.rfind('\n').map(|i| i + 1).unwrap_or(0);
                        for line in buf[..complete].lines() {
                            if line.len() < 10 {
                                continue;
                            }
                            if let Ok(v) = serde_json::from_str::<Value>(line) {
                                if let Some(k) = keep(&v) {
                                    entry.1.push(k);
                                }
                            }
                        }
                        entry.0 += complete as u64;
                    }
                }
            }
        }
        &entry.1
    }
}

fn window_start(kind: u32) -> i64 {
    let now = unix_now();
    match kind {
        1 => local_to_unix(&local_now().date_only()),
        2 => now - 7 * 86400,
        3 => now - 30 * 86400,
        _ => now - 5 * 3600,
    }
}

const WINDOWS: [&str; 3] = ["Últimas 5 horas", "Hoje", "Últimos 7 dias"];

fn reset_text(at: Option<i64>) -> String {
    match at {
        Some(t) if t > unix_now() => format!("zera em {}", fmt_hm(t - unix_now())),
        Some(_) => "zerou".into(),
        None => String::new(),
    }
}

fn limit_tile(t: &Tile, label: &str, pct: Option<f32>, reset: Option<i64>, accent: Color) {
    t.card();
    let inner = t.inner();
    match pct {
        None => {
            t.c.text(label, Rect::new(inner.x, inner.y, inner.w, inner.h * 0.45), t.fs(0.13), W_SEMIBOLD, accent, Align::Center);
            t.c.text("—", Rect::new(inner.x, inner.y + inner.h * 0.4, inner.w, inner.h * 0.55), t.fs(0.26), W_SEMIBOLD, WHITE.a(0.6), Align::Center);
        }
        Some(p) => {
            if t.wide() {
                let d = inner.h;
                let w = (t.s * 0.07).max(2.0);
                t.c.ring(inner.x + d / 2.0, inner.cy(), d / 2.0 - w, p, Color(1.0, 1.0, 1.0, 0.12), usage_color(p).mix(accent, 0.4), w);
                t.fit_display(&format!("{:.0}%", p * 100.0), Rect::new(inner.x, inner.cy() - d * 0.18, d, d * 0.36), t.fs(0.17), W_SEMIBOLD, WHITE, Align::Center);
                let r = Rect::new(inner.x + d + t.s * 0.1, inner.y, inner.w - d - t.s * 0.1, inner.h);
                t.fit_text(label, Rect::new(r.x, r.y, r.w, r.h * 0.5), t.fs(0.15), W_SEMIBOLD, WHITE, Align::Left);
                t.fit_text(&reset_text(reset), Rect::new(r.x, r.y + r.h * 0.5, r.w, r.h * 0.42), t.fs(0.12), W_NORMAL, WHITE.a(0.6), Align::Left);
            } else {
                let w = (t.s * 0.075).max(2.0);
                let d = inner.h.min(inner.w);
                t.c.ring(inner.cx(), inner.cy(), d / 2.0 - w / 2.0, p, Color(1.0, 1.0, 1.0, 0.12), usage_color(p).mix(accent, 0.4), w);
                t.fit_display(&format!("{:.0}%", p * 100.0), inner.centered(d * 0.7, d * 0.34), t.fs(0.2), W_SEMIBOLD, WHITE, Align::Center);
                t.fit_text(label, Rect::new(inner.x, inner.cy() + d * 0.13, inner.w, d * 0.2), t.fs(0.1), W_SEMIBOLD, accent, Align::Center);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Claude
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct ClaudeCfg {
    count: u32,
    budget_m: i64,
    prev_cmd: String,
}

#[derive(Default, Clone)]
struct ClaudeData {
    five: Option<(f32, Option<i64>)>,
    week: Option<(f32, Option<i64>)>,
    tokens: u64,
    found_dirs: Vec<String>,
    status_age: Option<i64>,
}

pub struct Claude {
    c: ClaudeCfg,
    data: ClaudeData,
    rx: Option<Receiver<(ClaudeData, JsonlCache)>>,
    cache: Option<JsonlCache>,
    last: u64,
    msg: String,
}

pub fn claude_dirs() -> Vec<PathBuf> {
    let mut v = vec![];
    if let Ok(d) = std::env::var("CLAUDE_CONFIG_DIR") {
        for p in d.split(';') {
            v.push(PathBuf::from(p));
        }
    }
    v.push(home_dir().join(".claude"));
    v.push(home_dir().join(".config").join("claude"));
    v.retain(|p| p.exists());
    v.dedup();
    v
}

pub fn status_file() -> PathBuf {
    data_dir().join("claude-status.json")
}

fn find_limit(v: &Value, keys: &[&str]) -> Option<(f32, Option<i64>)> {
    match v {
        Value::Object(m) => {
            for (k, val) in m {
                let kl = k.to_lowercase();
                if keys.iter().any(|x| kl == *x) {
                    if let Some(o) = val.as_object() {
                        let pct = ["used_percentage", "used_percent", "utilization", "percent", "percentage", "used"]
                            .iter()
                            .find_map(|f| o.get(*f).and_then(|x| x.as_f64()));
                        if let Some(p) = pct {
                            let p = if p > 1.0 { p / 100.0 } else { p };
                            let reset = ["resets_at", "reset_at", "resetsAt"].iter().find_map(|f| o.get(*f).and_then(ts_any));
                            return Some((p as f32, reset));
                        }
                    } else if let Some(p) = val.as_f64() {
                        return Some(((if p > 1.0 { p / 100.0 } else { p }) as f32, None));
                    }
                }
                if let Some(r) = find_limit(val, keys) {
                    return Some(r);
                }
            }
            None
        }
        Value::Array(a) => a.iter().find_map(|x| find_limit(x, keys)),
        _ => None,
    }
}

fn scan_claude(count: u32, mut cache: JsonlCache) -> (ClaudeData, JsonlCache) {
    let mut d = ClaudeData::default();
    if let Ok(s) = std::fs::read_to_string(status_file()) {
        if let Ok(v) = serde_json::from_str::<Value>(&s) {
            d.five = find_limit(&v, &["five_hour", "5h", "five_hours", "session"]);
            d.week = find_limit(&v, &["seven_day", "7d", "weekly", "week"]);
            d.status_age = v.get("_glassdock_ts").and_then(|x| x.as_i64()).map(|t| unix_now() - t);
        }
    }
    let since = window_start(count);
    let mut seen: HashSet<String> = HashSet::new();
    for dir in claude_dirs() {
        d.found_dirs.push(dir.to_string_lossy().to_string());
        let mut files = vec![];
        files_recursive(&dir.join("projects"), "jsonl", since - 3600, &mut files, 0);
        for (p, len, _) in files {
            let entries = cache.read(&p, len, &|v: &Value| {
                let u = v.pointer("/message/usage")?;
                let ts = v.get("timestamp").and_then(ts_any)?;
                let tok = ["input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
                    .iter()
                    .map(|k| u.get(*k).and_then(|x| x.as_u64()).unwrap_or(0))
                    .sum::<u64>();
                let idk = format!(
                    "{}:{}",
                    v.pointer("/message/id").and_then(|x| x.as_str()).unwrap_or(""),
                    v.get("requestId").and_then(|x| x.as_str()).unwrap_or("")
                );
                Some(serde_json::json!([ts, tok, idk]))
            });
            for e in entries {
                let ts = e[0].as_i64().unwrap_or(0);
                if ts < since {
                    continue;
                }
                let k = e[2].as_str().unwrap_or("").to_string();
                if k.len() > 2 && !seen.insert(k) {
                    continue;
                }
                d.tokens += e[1].as_u64().unwrap_or(0);
            }
        }
    }
    (d, cache)
}

impl Claude {
    pub fn load(v: &Value) -> Claude {
        let mut c = Claude { c: jget(v), data: ClaudeData::default(), rx: None, cache: Some(JsonlCache::default()), last: 0, msg: String::new() };
        c.refresh();
        c
    }
    fn refresh(&mut self) {
        if self.rx.is_some() {
            return;
        }
        let Some(cache) = self.cache.take() else { return };
        self.last = tick_ms();
        let count = self.c.count;
        let (tx, rx) = channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(scan_claude(count, cache));
            crate::app::post(crate::app::WM_APP_POLL, 0, 0);
        });
    }

    /// Liga a status line do Claude Code ao GlassDock (encadeando a anterior).
    fn configure(&mut self) -> Result<String, String> {
        let dir = home_dir().join(".claude");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.json");
        let mut v: Value = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Object(Default::default()));
        let exe = exe_path().to_string_lossy().to_string();
        let ours = format!("\"{}\" --claude-statusline", exe);
        let prev = v.pointer("/statusLine/command").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if !prev.is_empty() && !prev.contains("--claude-statusline") {
            self.c.prev_cmd = prev.clone();
            let _ = std::fs::write(data_dir().join("claude-prev-statusline.txt"), &prev);
        }
        if let Some(o) = v.as_object_mut() {
            o.insert("statusLine".into(), serde_json::json!({"type": "command", "command": ours, "padding": 0}));
        }
        let s = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
        if path.exists() {
            let _ = std::fs::copy(&path, dir.join("settings.json.glassdock-backup"));
        }
        std::fs::write(&path, s).map_err(|e| e.to_string())?;
        Ok(if prev.is_empty() { "Pronto. Os limites aparecem depois da próxima mensagem no Claude Code.".into() } else { "Pronto. A sua status line anterior continua aparecendo.".into() })
    }
}

impl Widget for Claude {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn styles(&self) -> &'static [&'static str] {
        &["Limite de 5 horas", "Limite semanal", "Tokens"]
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        Some(60000)
    }
    fn tick(&mut self, _sh: &mut Shared) -> bool {
        if tick_ms().saturating_sub(self.last) >= 55_000 {
            self.refresh();
        }
        false
    }
    fn poll(&mut self, _sh: &mut Shared) -> bool {
        if let Some(rx) = &self.rx {
            if let Ok((d, cache)) = rx.try_recv() {
                self.data = d;
                self.cache = Some(cache);
                self.rx = None;
                return true;
            }
        }
        false
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        let accent = Color::hex(0xD97757);
        match t.style {
            0 => limit_tile(t, "5 horas", self.data.five.map(|x| x.0), self.data.five.and_then(|x| x.1), accent),
            1 => limit_tile(t, "Semana", self.data.week.map(|x| x.0), self.data.week.and_then(|x| x.1), accent),
            _ => {
                t.card();
                let inner = t.inner();
                let tok = fmt_tokens(self.data.tokens);
                if t.wide() {
                    t.c.glyph(0xE945, Rect::new(inner.x, inner.y, inner.h * 0.6, inner.h * 0.5), t.fs(0.16), accent);
                    t.fit_display(&tok, Rect::new(inner.x, inner.y, inner.w, inner.h * 0.5), t.fs(0.22), W_SEMIBOLD, WHITE, Align::Right);
                    if self.c.budget_m > 0 {
                        let f = self.data.tokens as f32 / (self.c.budget_m as f32 * 1e6);
                        bar(t.c, Rect::new(inner.x, inner.y + inner.h * 0.62, inner.w, inner.h * 0.16), f, Color(1.0, 1.0, 1.0, 0.12), usage_color(f));
                    } else {
                        t.fit_text(&format!("tokens · {}", WINDOWS[self.c.count as usize % 3].to_lowercase()), Rect::new(inner.x, inner.y + inner.h * 0.52, inner.w, inner.h * 0.42), t.fs(0.12), W_NORMAL, WHITE.a(0.6), Align::Left);
                    }
                } else {
                    t.fit_display(&tok, Rect::new(inner.x, inner.y, inner.w, inner.h * 0.6), t.fs(0.22), W_SEMIBOLD, WHITE, Align::Center);
                    t.c.text("tokens", Rect::new(inner.x, inner.y + inner.h * 0.58, inner.w, inner.h * 0.36), t.fs(0.11), W_SEMIBOLD, accent, Align::Center);
                }
            }
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (330.0, 330.0)
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        panel_header(ui, "Uso do Claude", "Lido do Claude Code neste PC, sem login.");
        let accent = Color::hex(0xD97757);
        for (label, v) in [("Limite de 5 horas", self.data.five), ("Limite semanal", self.data.week)] {
            let r = ui.row(22.0);
            ui.c.text(label, r, 12.5 * ui.s, W_MEDIUM, TXT, Align::Left);
            match v {
                Some((p, reset)) => {
                    ui.c.text(&format!("{:.0}% · {}", p * 100.0, reset_text(reset)), r, 12.0 * ui.s, W_NORMAL, TXT2, Align::Right);
                    let r = ui.row(14.0);
                    bar(ui.c, Rect::new(r.x, r.y + 2.0, r.w, 8.0 * ui.s), p, Color(1.0, 1.0, 1.0, 0.12), usage_color(p).mix(accent, 0.3));
                }
                None => {
                    ui.c.text("sem dados", r, 12.0 * ui.s, W_NORMAL, TXT3, Align::Right);
                    ui.gap(4.0);
                }
            }
            ui.gap(6.0);
        }
        let r = ui.row(22.0);
        ui.c.text(&format!("Tokens · {}", WINDOWS[self.c.count as usize % 3].to_lowercase()), r, 12.5 * ui.s, W_MEDIUM, TXT, Align::Left);
        ui.c.text(&fmt_thousands(self.data.tokens), r, 12.5 * ui.s, W_SEMIBOLD, TXT, Align::Right);
        ui.gap(8.0);
        if self.data.five.is_none() && self.data.week.is_none() {
            ui.note("Os limites vêm da status line do Claude Code. Clique em Configurar uma vez: o GlassDock passa a ler o uso por ela (a sua status line atual continua aparecendo).");
            let r = ui.row(32.0);
            if ui.button_k(id("cfg"), Rect::new(r.x, r.y, 120.0 * ui.s, r.h), "Configurar", Btn::Primary) {
                self.msg = match self.configure() {
                    Ok(m) => m,
                    Err(e) => format!("Não consegui: {}", e),
                };
                sh.save = true;
            }
            if !self.msg.is_empty() {
                let m = self.msg.clone();
                ui.note(&m);
            }
        }
        if self.data.found_dirs.is_empty() {
            ui.note("Não encontrei o Claude Code (pasta .claude) neste PC.");
        } else {
            let dirs = self.data.found_dirs.join(", ");
            ui.note(&format!("Procurei em: {}", dirs));
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        if ui.labeled_segmented(id("cnt"), "Contar tokens", &["5 horas", "Hoje", "7 dias"], &mut self.c.count) {
            sh.save = true;
            self.cache = Some(JsonlCache::default());
            self.rx = None;
            self.refresh();
        }
        if ui.stepper(id("bud"), "Orçamento (milhões de tokens, 0 = nenhum)", &mut self.c.budget_m, 0, 100, 5, "") {
            sh.save = true;
        }
        let r = ui.row(34.0);
        if ui.button(id("cfg2"), Rect::new(r.x, r.y + 2.0, 220.0 * ui.s, r.h - 4.0), "Configurar a status line de novo") {
            self.msg = self.configure().unwrap_or_else(|e| e);
        }
        if !self.msg.is_empty() {
            let m = self.msg.clone();
            ui.note(&m);
        }
    }
}

/// Modo `--claude-statusline`: guarda o JSON recebido e imprime uma linha.
pub fn statusline_main() {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    if let Ok(mut v) = serde_json::from_str::<Value>(&input) {
        if let Some(o) = v.as_object_mut() {
            o.insert("_glassdock_ts".into(), Value::from(unix_now()));
        }
        write_json_atomic(&status_file(), &v);
    }
    let prev = std::fs::read_to_string(data_dir().join("claude-prev-statusline.txt")).unwrap_or_default();
    use std::io::Write;
    if !prev.trim().is_empty() {
        let child = std::process::Command::new("cmd")
            .args(["/C", prev.trim()])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn();
        if let Ok(mut ch) = child {
            if let Some(mut si) = ch.stdin.take() {
                let _ = si.write_all(input.as_bytes());
            }
            if let Ok(out) = ch.wait_with_output() {
                let _ = std::io::stdout().write_all(&out.stdout);
                return;
            }
        }
    }
    let v: Value = serde_json::from_str(&input).unwrap_or(Value::Null);
    let model = v.pointer("/model/display_name").and_then(|x| x.as_str()).unwrap_or("Claude");
    let dir = v.pointer("/workspace/current_dir").and_then(|x| x.as_str()).map(|d| file_stem(d)).unwrap_or_default();
    let five = find_limit(&v, &["five_hour"]).map(|x| format!(" · 5h {:.0}%", x.0 * 100.0)).unwrap_or_default();
    let _ = write!(std::io::stdout(), "{} · {}{}", model, dir, five);
}

// ---------------------------------------------------------------------------
// Codex
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct CodexCfg {
    count: u32,
}

#[derive(Default, Clone)]
struct CodexData {
    primary: Option<(f32, Option<i64>, i64)>,
    secondary: Option<(f32, Option<i64>, i64)>,
    tokens: u64,
    found: bool,
}

pub struct Codex {
    c: CodexCfg,
    data: CodexData,
    rx: Option<Receiver<(CodexData, JsonlCache)>>,
    cache: Option<JsonlCache>,
    last: u64,
}

fn codex_dir() -> PathBuf {
    std::env::var("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|_| home_dir().join(".codex"))
}

fn scan_codex(count: u32, mut cache: JsonlCache) -> (CodexData, JsonlCache) {
    let mut d = CodexData::default();
    let dir = codex_dir().join("sessions");
    d.found = dir.exists();
    let since = window_start(count).min(unix_now() - 8 * 86400);
    let tok_since = window_start(count);
    let mut files = vec![];
    files_recursive(&dir, "jsonl", since, &mut files, 0);
    let mut latest: Option<(i64, Value)> = None;
    for (p, len, _) in files {
        let entries = cache.read(&p, len, &|v: &Value| {
            let pl = v.get("payload")?;
            if pl.get("type")?.as_str()? != "token_count" {
                return None;
            }
            let ts = v.get("timestamp").and_then(ts_any)?;
            let last = pl.pointer("/info/last_token_usage/total_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
            Some(serde_json::json!([ts, last, pl.get("rate_limits").cloned().unwrap_or(Value::Null)]))
        });
        for e in entries {
            let ts = e[0].as_i64().unwrap_or(0);
            if ts >= tok_since {
                d.tokens += e[1].as_u64().unwrap_or(0);
            }
            if !e[2].is_null() && latest.as_ref().map(|l| ts > l.0).unwrap_or(true) {
                latest = Some((ts, e[2].clone()));
            }
        }
    }
    if let Some((ts, rl)) = latest {
        let read = |k: &str| -> Option<(f32, Option<i64>, i64)> {
            let o = rl.get(k)?;
            let p = o.get("used_percent")?.as_f64()? as f32 / 100.0;
            let win = o.get("window_minutes").and_then(|x| x.as_i64()).unwrap_or(0);
            let reset = o.get("resets_at").and_then(ts_any).or_else(|| o.get("resets_in_seconds").and_then(|x| x.as_i64()).map(|s| ts + s));
            Some((p, reset, win))
        };
        d.primary = read("primary");
        d.secondary = read("secondary");
    }
    (d, cache)
}

impl Codex {
    pub fn load(v: &Value) -> Codex {
        let mut c = Codex { c: jget(v), data: CodexData::default(), rx: None, cache: Some(JsonlCache::default()), last: 0 };
        c.refresh();
        c
    }
    fn refresh(&mut self) {
        if self.rx.is_some() {
            return;
        }
        let Some(cache) = self.cache.take() else { return };
        self.last = tick_ms();
        let count = self.c.count;
        let (tx, rx) = channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(scan_codex(count, cache));
            crate::app::post(crate::app::WM_APP_POLL, 0, 0);
        });
    }
}

fn win_label(min: i64, fallback: &str) -> String {
    if min >= 60 * 24 * 6 {
        "Semana".into()
    } else if min > 0 {
        format!("{}h", (min + 59) / 60)
    } else {
        fallback.into()
    }
}

impl Widget for Codex {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        Some(60000)
    }
    fn tick(&mut self, _sh: &mut Shared) -> bool {
        if tick_ms().saturating_sub(self.last) >= 55_000 {
            self.refresh();
        }
        false
    }
    fn poll(&mut self, _sh: &mut Shared) -> bool {
        if let Some(rx) = &self.rx {
            if let Ok((d, cache)) = rx.try_recv() {
                self.data = d;
                self.cache = Some(cache);
                self.rx = None;
                return true;
            }
        }
        false
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        let accent = Color::hex(0x10A37F);
        if !t.wide() {
            return limit_tile(t, "Codex", self.data.primary.map(|x| x.0), self.data.primary.and_then(|x| x.1), accent);
        }
        t.card();
        let inner = t.inner();
        let rows = [(self.data.primary, "5h"), (self.data.secondary, "Semana")];
        let lh = inner.h / 2.0;
        for (i, (v, fb)) in rows.iter().enumerate() {
            let r = Rect::new(inner.x, inner.y + lh * i as f32, inner.w, lh);
            let label = v.map(|x| win_label(x.2, fb)).unwrap_or_else(|| fb.to_string());
            t.c.text(&label, Rect::new(r.x, r.y, r.w * 0.4, r.h * 0.55), t.fs(0.11), W_SEMIBOLD, accent.mix(WHITE, 0.3), Align::Left);
            let pct = v.map(|x| format!("{:.0}%", x.0 * 100.0)).unwrap_or_else(|| "—".into());
            t.c.text(&pct, Rect::new(r.x, r.y, r.w, r.h * 0.55), t.fs(0.11), W_SEMIBOLD, WHITE, Align::Right);
            bar(t.c, Rect::new(r.x, r.y + r.h * 0.6, r.w, r.h * 0.2), v.map(|x| x.0).unwrap_or(0.0), Color(1.0, 1.0, 1.0, 0.12), usage_color(v.map(|x| x.0).unwrap_or(0.0)));
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (320.0, 250.0)
    }
    fn panel(&mut self, ui: &mut Ui, _sh: &mut Shared) {
        panel_header(ui, "Uso do Codex", "Lido de ~/.codex/sessions neste PC.");
        for (v, fb) in [(self.data.primary, "Limite de 5 horas"), (self.data.secondary, "Limite semanal")] {
            let r = ui.row(22.0);
            ui.c.text(fb, r, 12.5 * ui.s, W_MEDIUM, TXT, Align::Left);
            match v {
                Some((p, reset, _)) => {
                    ui.c.text(&format!("{:.0}% · {}", p * 100.0, reset_text(reset)), r, 12.0 * ui.s, W_NORMAL, TXT2, Align::Right);
                    let r = ui.row(14.0);
                    bar(ui.c, Rect::new(r.x, r.y + 2.0, r.w, 8.0 * ui.s), p, Color(1.0, 1.0, 1.0, 0.12), usage_color(p));
                }
                None => ui.c.text("sem dados", r, 12.0 * ui.s, W_NORMAL, TXT3, Align::Right),
            }
            ui.gap(6.0);
        }
        let r = ui.row(22.0);
        ui.c.text(&format!("Tokens · {}", WINDOWS[self.c.count as usize % 3].to_lowercase()), r, 12.5 * ui.s, W_MEDIUM, TXT, Align::Left);
        ui.c.text(&fmt_thousands(self.data.tokens), r, 12.5 * ui.s, W_SEMIBOLD, TXT, Align::Right);
        if !self.data.found {
            ui.gap(6.0);
            ui.note("Não encontrei o Codex neste PC (pasta .codex/sessions).");
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        if ui.labeled_segmented(id("cnt"), "Contar tokens", &["5 horas", "Hoje", "7 dias"], &mut self.c.count) {
            sh.save = true;
            self.cache = Some(JsonlCache::default());
            self.rx = None;
            self.refresh();
        }
    }
}

// ---------------------------------------------------------------------------
// OpenCode
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct OcCfg {
    count: u32,
}

impl Default for OcCfg {
    fn default() -> Self {
        OcCfg { count: 1 }
    }
}

#[derive(Default, Clone)]
struct OcData {
    by_provider: Vec<(String, f64, u64)>,
    cost: f64,
    tokens: u64,
    found: bool,
}

pub struct OpenCode {
    c: OcCfg,
    data: OcData,
    rx: Option<Receiver<(OcData, HashMap<PathBuf, (i64, Option<(String, f64, u64, i64)>)>)>>,
    cache: Option<HashMap<PathBuf, (i64, Option<(String, f64, u64, i64)>)>>,
    last: u64,
}

fn opencode_dirs() -> Vec<PathBuf> {
    let mut v = vec![];
    if let Ok(x) = std::env::var("XDG_DATA_HOME") {
        v.push(PathBuf::from(x).join("opencode").join("storage"));
    }
    v.push(home_dir().join(".local").join("share").join("opencode").join("storage"));
    if let Ok(x) = std::env::var("APPDATA") {
        v.push(PathBuf::from(x).join("opencode").join("storage"));
    }
    if let Ok(x) = std::env::var("LOCALAPPDATA") {
        v.push(PathBuf::from(x).join("opencode").join("storage"));
    }
    v.retain(|p| p.exists());
    v
}

type OcCache = HashMap<PathBuf, (i64, Option<(String, f64, u64, i64)>)>;

fn scan_opencode(count: u32, mut cache: OcCache) -> (OcData, OcCache) {
    let mut d = OcData::default();
    let since = window_start(count);
    let mut agg: HashMap<String, (f64, u64)> = HashMap::new();
    for dir in opencode_dirs() {
        d.found = true;
        let mut files = vec![];
        files_recursive(&dir.join("message"), "json", since - 3600, &mut files, 0);
        for (p, _len, mt) in files {
            let ent = cache.entry(p.clone()).or_insert((-1, None));
            if ent.0 != mt {
                let parsed = std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).and_then(|v| {
                    if v.get("role")?.as_str()? != "assistant" {
                        return None;
                    }
                    let prov = v.get("providerID").and_then(|x| x.as_str()).unwrap_or("?").to_string();
                    let cost = v.get("cost").and_then(|x| x.as_f64()).unwrap_or(0.0);
                    let t = v.get("tokens")?;
                    let tok = ["input", "output", "reasoning"].iter().map(|k| t.get(*k).and_then(|x| x.as_u64()).unwrap_or(0)).sum::<u64>()
                        + t.pointer("/cache/read").and_then(|x| x.as_u64()).unwrap_or(0)
                        + t.pointer("/cache/write").and_then(|x| x.as_u64()).unwrap_or(0);
                    let created = v.pointer("/time/created").and_then(ts_any).unwrap_or(mt);
                    Some((prov, cost, tok, created))
                });
                *ent = (mt, parsed);
            }
            if let Some((prov, cost, tok, created)) = &ent.1 {
                if *created >= since {
                    let a = agg.entry(prov.clone()).or_insert((0.0, 0));
                    a.0 += cost;
                    a.1 += tok;
                }
            }
        }
    }
    let mut v: Vec<(String, f64, u64)> = agg.into_iter().map(|(k, (c, t))| (k, c, t)).collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(b.2.cmp(&a.2)));
    d.cost = v.iter().map(|x| x.1).sum();
    d.tokens = v.iter().map(|x| x.2).sum();
    d.by_provider = v;
    (d, cache)
}

impl OpenCode {
    pub fn load(v: &Value) -> OpenCode {
        let mut c = OpenCode { c: jget(v), data: OcData::default(), rx: None, cache: Some(HashMap::new()), last: 0 };
        c.refresh();
        c
    }
    fn refresh(&mut self) {
        if self.rx.is_some() {
            return;
        }
        let Some(cache) = self.cache.take() else { return };
        self.last = tick_ms();
        let count = self.c.count;
        let (tx, rx) = channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(scan_opencode(count, cache));
            crate::app::post(crate::app::WM_APP_POLL, 0, 0);
        });
    }
}

impl Widget for OpenCode {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        Some(60000)
    }
    fn tick(&mut self, _sh: &mut Shared) -> bool {
        if tick_ms().saturating_sub(self.last) >= 55_000 {
            self.refresh();
        }
        false
    }
    fn poll(&mut self, _sh: &mut Shared) -> bool {
        if let Some(rx) = &self.rx {
            if let Ok((d, cache)) = rx.try_recv() {
                self.data = d;
                self.cache = Some(cache);
                self.rx = None;
                return true;
            }
        }
        false
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        t.card();
        let inner = t.inner();
        let accent = Color::hex(0xF5A623);
        let cost = format!("${:.2}", self.data.cost);
        if t.wide() {
            t.fit_display(&cost, Rect::new(inner.x, inner.y, inner.w * 0.55, inner.h * 0.58), t.fs(0.24), W_SEMIBOLD, WHITE, Align::Left);
            t.fit_text(&format!("{} tokens", fmt_tokens(self.data.tokens)), Rect::new(inner.x + inner.w * 0.5, inner.y, inner.w * 0.5, inner.h * 0.58), t.fs(0.12), W_MEDIUM, WHITE.a(0.7), Align::Right);
            let top = self.data.by_provider.first().map(|x| x.0.clone()).unwrap_or_else(|| "OpenCode".into());
            t.fit_text(&format!("{} · {}", top, ["5h", "hoje", "7 dias", "30 dias"][self.c.count as usize % 4]), Rect::new(inner.x, inner.y + inner.h * 0.56, inner.w, inner.h * 0.4), t.fs(0.12), W_NORMAL, accent, Align::Left);
        } else {
            t.fit_display(&cost, Rect::new(inner.x, inner.y, inner.w, inner.h * 0.6), t.fs(0.22), W_SEMIBOLD, WHITE, Align::Center);
            t.c.text("OpenCode", Rect::new(inner.x, inner.y + inner.h * 0.58, inner.w, inner.h * 0.36), t.fs(0.1), W_SEMIBOLD, accent, Align::Center);
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (320.0, 150.0 + self.data.by_provider.len().clamp(1, 8) as f32 * 30.0)
    }
    fn panel(&mut self, ui: &mut Ui, _sh: &mut Shared) {
        panel_header(ui, "Uso do OpenCode", ["Últimas 5 horas", "Hoje", "Últimos 7 dias", "Últimos 30 dias"][self.c.count as usize % 4]);
        let r = ui.row(30.0);
        ui.c.display(&format!("${:.2}", self.data.cost), r, 22.0 * ui.s, W_SEMIBOLD, TXT, Align::Left);
        ui.c.text(&format!("{} tokens", fmt_thousands(self.data.tokens)), r, 12.5 * ui.s, W_NORMAL, TXT2, Align::Right);
        ui.separator();
        for (p, c, t) in self.data.by_provider.iter().take(8) {
            let r = ui.row(30.0);
            ui.c.text(p, r, 12.5 * ui.s, W_MEDIUM, TXT, Align::Left);
            ui.c.text(&format!("${:.2} · {}", c, fmt_tokens(*t)), r, 12.0 * ui.s, W_NORMAL, TXT2, Align::Right);
        }
        if !self.data.found {
            ui.note("Não encontrei o OpenCode neste PC (pasta .local/share/opencode).");
        } else if self.data.by_provider.is_empty() {
            ui.note("Nenhum uso neste período.");
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        if ui.labeled_segmented(id("cnt"), "Período", &["5 horas", "Hoje", "7 dias", "30 dias"], &mut self.c.count) {
            sh.save = true;
            self.cache = Some(HashMap::new());
            self.rx = None;
            self.refresh();
        }
    }
}
