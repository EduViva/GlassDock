// Timer de foco, Pomodoro, Contagem regressiva e Progresso do tempo.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::*;
use crate::ui::*;

const TIMER_COLORS: [u32; 6] = [0xFF9F0A, 0x0A84FF, 0x30D158, 0xFF375F, 0xBF5AF2, 0x40C8E0];

// ---------------------------------------------------------------------------
// Timer
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct TimerCfg {
    quick: Vec<u32>,
    color: u32,
    duration: u32,
    end: i64,
    remaining: u32,
    running: bool,
    custom: i64,
}

impl Default for TimerCfg {
    fn default() -> Self {
        TimerCfg { quick: vec![5, 10, 15, 25, 45], color: 0, duration: 25 * 60, end: 0, remaining: 25 * 60, running: false, custom: 30 }
    }
}

pub struct Timer {
    c: TimerCfg,
}

impl Timer {
    pub fn load(v: &Value) -> Timer {
        Timer { c: jget(v) }
    }
    fn left(&self) -> i64 {
        if self.c.running {
            (self.c.end - unix_now()).max(0)
        } else {
            self.c.remaining as i64
        }
    }
    fn start(&mut self, secs: u32) {
        self.c.duration = secs;
        self.c.remaining = secs;
        self.c.end = unix_now() + secs as i64;
        self.c.running = true;
    }
    fn toggle(&mut self) {
        if self.c.running {
            self.c.remaining = self.left() as u32;
            self.c.running = false;
        } else {
            if self.c.remaining == 0 {
                self.c.remaining = self.c.duration;
            }
            self.c.end = unix_now() + self.c.remaining as i64;
            self.c.running = true;
        }
    }
    fn reset(&mut self) {
        self.c.running = false;
        self.c.remaining = self.c.duration;
    }
    fn color(&self) -> Color {
        Color::hex(TIMER_COLORS[self.c.color as usize % TIMER_COLORS.len()])
    }
}

pub fn ring_tile(t: &Tile, frac: f32, col: Color, center: &str, sub: &str) {
    let inner = t.inner();
    let d = inner.h.min(inner.w);
    let cx = if t.wide() { inner.x + d / 2.0 } else { inner.cx() };
    let cy = inner.cy();
    let w = (t.s * 0.075).max(2.0);
    t.c.ring(cx, cy, d / 2.0 - w / 2.0, frac, Color(1.0, 1.0, 1.0, 0.14), col, w);
    let tr = Rect::new(cx - d / 2.0 + w, cy - d * 0.22, d - 2.0 * w, d * 0.32);
    t.fit_display(center, tr, t.fs(0.18), W_SEMIBOLD, WHITE, Align::Center);
    if !sub.is_empty() {
        t.fit_text(sub, Rect::new(cx - d / 2.0 + w, cy + d * 0.08, d - 2.0 * w, d * 0.2), t.fs(0.11), W_MEDIUM, WHITE.a(0.6), Align::Center);
    }
}

impl Widget for Timer {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        if self.c.running {
            Some(1000)
        } else {
            None
        }
    }
    fn tick(&mut self, sh: &mut Shared) -> bool {
        if self.c.running && self.left() <= 0 {
            self.c.running = false;
            self.c.remaining = 0;
            sh.toast("Timer", &format!("{} terminaram.", fmt_hm(self.c.duration as i64)));
            sh.save = true;
            return true;
        }
        self.c.running
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        t.card();
        let left = self.left();
        let frac = if self.c.duration > 0 { left as f32 / self.c.duration as f32 } else { 0.0 };
        let sub = if self.c.running { "" } else if left == 0 { "fim" } else { "pausado" };
        ring_tile(t, frac, self.color(), &fmt_duration_short(left), sub);
        if t.wide() {
            let inner = t.inner();
            let d = inner.h;
            let area = Rect::new(inner.x + d + t.s * 0.1, inner.y, inner.w - d - t.s * 0.1, inner.h);
            let bw = (area.w / 2.0 - t.s * 0.05).min(t.s * 0.62);
            let b1 = Rect::new(area.x, area.cy() - bw / 2.0, bw, bw);
            let b2 = Rect::new(area.x + bw + t.s * 0.1, area.cy() - bw / 2.0, bw, bw);
            t.c.fill_circle(b1.cx(), b1.cy(), bw / 2.0, self.color().a(0.28));
            t.c.glyph(if self.c.running { 0xE769 } else { 0xE768 }, b1, bw * 0.42, self.color().mix(WHITE, 0.4));
            t.c.fill_circle(b2.cx(), b2.cy(), bw / 2.0, Color(1.0, 1.0, 1.0, 0.12));
            t.c.glyph(0xE72C, b2, bw * 0.4, WHITE.a(0.85));
        }
    }
    fn on_click(&mut self, fx: f32, _fy: f32, units: u32, sh: &mut Shared) -> Click {
        if units >= 2 {
            let split = 1.0 / units as f32 + 0.05;
            if fx > split {
                let rel = (fx - split) / (1.0 - split);
                if rel < 0.5 {
                    self.toggle();
                } else {
                    self.reset();
                }
                sh.save = true;
                return Click::Handled;
            }
        }
        Click::Panel
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (290.0, 330.0)
    }
    fn panel_interval(&self, _sh: &Shared) -> Option<u32> {
        if self.c.running {
            Some(1000)
        } else {
            None
        }
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        panel_header(ui, "Timer de foco", if self.c.running { "Contando" } else { "Parado" });
        let r = ui.row(130.0);
        let left = self.left();
        let frac = if self.c.duration > 0 { left as f32 / self.c.duration as f32 } else { 0.0 };
        ui.c.ring(r.cx(), r.cy(), r.h * 0.44, frac, Color(1.0, 1.0, 1.0, 0.12), self.color(), 6.0 * ui.s);
        ui.c.display(&fmt_duration_short(left), r.centered(r.h, 40.0 * ui.s), 28.0 * ui.s, W_SEMIBOLD, TXT, Align::Center);
        ui.gap(8.0);
        let r = ui.row(34.0);
        let w = (r.w - 8.0 * ui.s) / 2.0;
        if ui.button_k(id("tgl"), Rect::new(r.x, r.y, w, r.h), if self.c.running { "Pausar" } else { "Iniciar" }, Btn::Primary) {
            self.toggle();
            sh.save = true;
        }
        if ui.button(id("rst"), Rect::new(r.x + w + 8.0 * ui.s, r.y, w, r.h), "Zerar") {
            self.reset();
            sh.save = true;
        }
        ui.gap(10.0);
        let r = ui.row(30.0);
        let n = self.c.quick.len().max(1);
        let w = r.w / n as f32;
        for (i, m) in self.c.quick.clone().iter().enumerate() {
            let br = Rect::new(r.x + w * i as f32 + 2.0, r.y, w - 4.0, r.h);
            if ui.button(idn("q", i as u64), br, &format!("{} min", m)) {
                self.start(m * 60);
                sh.save = true;
            }
        }
        ui.gap(6.0);
        let mut cm = self.c.custom;
        if ui.stepper(id("custom"), "Personalizado", &mut cm, 1, 600, 1, " min") {
            self.c.custom = cm;
            sh.save = true;
        }
        let r = ui.row(32.0);
        if ui.button(id("startc"), r, &format!("Iniciar {} min", self.c.custom)) {
            self.start(self.c.custom as u32 * 60);
            sh.save = true;
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        ui.caption("COR");
        let r = ui.row(30.0);
        let cols: Vec<Color> = TIMER_COLORS.iter().map(|c| Color::hex(*c)).collect();
        if ui.swatches(id("col"), r, &cols, &mut self.c.color) {
            sh.save = true;
        }
        ui.caption("DURAÇÕES RÁPIDAS (MIN)");
        for i in 0..self.c.quick.len() {
            let mut v = self.c.quick[i] as i64;
            if ui.stepper(idn("qd", i as u64), &format!("Botão {}", i + 1), &mut v, 1, 600, 1, " min") {
                self.c.quick[i] = v as u32;
                sh.save = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pomodoro
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct PomoCfg {
    focus: i64,
    short: i64,
    long: i64,
    every: i64,
    auto_breaks: bool,
    sound: bool,
    phase: u32,
    end: i64,
    remaining: i64,
    running: bool,
    done: u32,
    day: i64,
}

impl Default for PomoCfg {
    fn default() -> Self {
        PomoCfg { focus: 25, short: 5, long: 15, every: 4, auto_breaks: true, sound: true, phase: 0, end: 0, remaining: 25 * 60, running: false, done: 0, day: 0 }
    }
}

pub struct Pomodoro {
    c: PomoCfg,
}

impl Pomodoro {
    pub fn load(v: &Value) -> Pomodoro {
        Pomodoro { c: jget(v) }
    }
    fn phase_len(&self, p: u32) -> i64 {
        60 * match p {
            0 => self.c.focus,
            1 => self.c.short,
            _ => self.c.long,
        }
    }
    fn left(&self) -> i64 {
        if self.c.running {
            (self.c.end - unix_now()).max(0)
        } else {
            self.c.remaining
        }
    }
    fn phase_name(&self) -> &'static str {
        match self.c.phase {
            0 => "Foco",
            1 => "Pausa curta",
            _ => "Pausa longa",
        }
    }
    fn color(&self) -> Color {
        if self.c.phase == 0 {
            Color::hex(0xFF6347)
        } else {
            GREEN
        }
    }
    fn today_check(&mut self) {
        let d = local_now().days();
        if self.c.day != d {
            self.c.day = d;
            self.c.done = 0;
        }
    }
    fn set_phase(&mut self, p: u32, run: bool) {
        self.c.phase = p;
        self.c.remaining = self.phase_len(p);
        self.c.running = run;
        if run {
            self.c.end = unix_now() + self.c.remaining;
        }
    }
    fn advance(&mut self, sh: &mut Shared, finished: bool) {
        self.today_check();
        if self.c.phase == 0 {
            if finished {
                self.c.done += 1;
            }
            let long = self.c.every > 0 && self.c.done > 0 && self.c.done as i64 % self.c.every == 0;
            self.set_phase(if long { 2 } else { 1 }, self.c.auto_breaks);
            if finished {
                sh.toasts.push(Toast { title: "Pomodoro".into(), body: "Hora da pausa.".into(), sound: self.c.sound });
            }
        } else {
            self.set_phase(0, false);
            if finished {
                sh.toasts.push(Toast { title: "Pomodoro".into(), body: "A pausa acabou. Bora focar?".into(), sound: self.c.sound });
            }
        }
    }
    fn toggle(&mut self) {
        if self.c.running {
            self.c.remaining = self.left();
            self.c.running = false;
        } else {
            if self.c.remaining <= 0 {
                self.c.remaining = self.phase_len(self.c.phase);
            }
            self.c.end = unix_now() + self.c.remaining;
            self.c.running = true;
        }
    }
}

impl Widget for Pomodoro {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        if self.c.running {
            Some(1000)
        } else {
            None
        }
    }
    fn tick(&mut self, sh: &mut Shared) -> bool {
        if self.c.running && self.left() <= 0 {
            self.advance(sh, true);
            sh.save = true;
            return true;
        }
        self.c.running
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        self.today_check();
        t.card();
        let left = self.left();
        let frac = left as f32 / self.phase_len(self.c.phase).max(1) as f32;
        ring_tile(t, frac, self.color(), &fmt_duration_short(left), "");
        // ciclos do dia em pontinhos
        let inner = t.inner();
        let d = inner.h.min(inner.w);
        let cx = if t.wide() { inner.x + d / 2.0 } else { inner.cx() };
        let n = self.c.every.clamp(1, 8) as usize;
        let dot = t.s * 0.05;
        let total_w = n as f32 * dot * 2.6;
        let done_in_cycle = if self.c.every > 0 { (self.c.done as i64 % self.c.every) as usize } else { 0 };
        for i in 0..n {
            let x = cx - total_w / 2.0 + dot * 1.3 + i as f32 * dot * 2.6;
            let y = inner.cy() + d * 0.2;
            t.c.fill_circle(x, y, dot, if i < done_in_cycle { self.color() } else { Color(1.0, 1.0, 1.0, 0.22) });
        }
        if t.wide() {
            let area = Rect::new(inner.x + d + t.s * 0.12, inner.y, inner.w - d - t.s * 0.12, inner.h);
            t.fit_text(self.phase_name(), Rect::new(area.x, area.y, area.w, area.h * 0.5), t.fs(0.17), W_SEMIBOLD, WHITE, Align::Left);
            t.fit_text(&format!("{} hoje", self.c.done), Rect::new(area.x, area.y + area.h * 0.45, area.w, area.h * 0.45), t.fs(0.14), W_NORMAL, WHITE.a(0.6), Align::Left);
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (290.0, 300.0)
    }
    fn panel_interval(&self, _sh: &Shared) -> Option<u32> {
        if self.c.running {
            Some(1000)
        } else {
            None
        }
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        self.today_check();
        panel_header(ui, "Pomodoro", &format!("{} · {} sessões hoje", self.phase_name(), self.c.done));
        let r = ui.row(130.0);
        let left = self.left();
        let frac = left as f32 / self.phase_len(self.c.phase).max(1) as f32;
        ui.c.ring(r.cx(), r.cy(), r.h * 0.44, frac, Color(1.0, 1.0, 1.0, 0.12), self.color(), 6.0 * ui.s);
        ui.c.display(&fmt_duration_short(left), r.centered(r.h, 40.0 * ui.s), 28.0 * ui.s, W_SEMIBOLD, TXT, Align::Center);
        ui.gap(10.0);
        let r = ui.row(34.0);
        let w = (r.w - 16.0 * ui.s) / 3.0;
        if ui.button_k(id("tg"), Rect::new(r.x, r.y, w, r.h), if self.c.running { "Pausar" } else { "Iniciar" }, Btn::Primary) {
            self.toggle();
            sh.save = true;
        }
        if ui.button(id("skip"), Rect::new(r.x + w + 8.0 * ui.s, r.y, w, r.h), "Pular") {
            self.advance(sh, false);
            sh.save = true;
        }
        if ui.button(id("zero"), Rect::new(r.x + 2.0 * (w + 8.0 * ui.s), r.y, w, r.h), "Zerar") {
            self.set_phase(0, false);
            sh.save = true;
        }
        ui.gap(10.0);
        let r = ui.row(30.0);
        let w = r.w / 3.0;
        for (i, n) in ["Foco", "Pausa", "Longa"].iter().enumerate() {
            let k = if self.c.phase == i as u32 { Btn::Primary } else { Btn::Ghost };
            if ui.button_k(idn("ph", i as u64), Rect::new(r.x + w * i as f32 + 2.0, r.y, w - 4.0, r.h), n, k) {
                self.set_phase(i as u32, false);
                sh.save = true;
            }
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let mut ch = false;
        ch |= ui.stepper(id("f"), "Foco", &mut self.c.focus, 1, 120, 1, " min");
        ch |= ui.stepper(id("s"), "Pausa", &mut self.c.short, 1, 120, 1, " min");
        ch |= ui.stepper(id("l"), "Pausa longa", &mut self.c.long, 1, 120, 1, " min");
        ch |= ui.stepper(id("e"), "Focos antes da pausa longa", &mut self.c.every, 2, 8, 1, "");
        ch |= ui.row_toggle(id("ab"), "Começar as pausas sozinho", &mut self.c.auto_breaks);
        ch |= ui.row_toggle(id("snd"), "Som no fim", &mut self.c.sound);
        if ch {
            if !self.c.running {
                self.c.remaining = self.phase_len(self.c.phase);
            }
            sh.save = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Contagem regressiva
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct CdCfg {
    title: String,
    date: String,
    time: String,
    keep: bool,
}

impl Default for CdCfg {
    fn default() -> Self {
        let n = local_now();
        let t = DateTime::from_local_secs(n.date_only().local_secs() + 30 * 86400);
        CdCfg { title: "Viagem".into(), date: format!("{:02}/{:02}/{}", t.day, t.month, t.year), time: "00:00".into(), keep: false }
    }
}

pub struct Countdown {
    c: CdCfg,
}

fn parse_date(d: &str, t: &str) -> Option<DateTime> {
    let p: Vec<i64> = d.split(['/', '-', '.']).filter_map(|x| x.trim().parse().ok()).collect();
    if p.len() != 3 {
        return None;
    }
    let (day, mon, year) = if p[0] > 31 { (p[2], p[1], p[0]) } else { (p[0], p[1], p[2]) };
    let q: Vec<i64> = t.split(':').filter_map(|x| x.trim().parse().ok()).collect();
    let (h, m) = (q.first().copied().unwrap_or(0), q.get(1).copied().unwrap_or(0));
    if !(1..=12).contains(&mon) || day < 1 || day > 31 {
        return None;
    }
    let mut dt = DateTime { year: year as i32, month: mon as u32, day: day as u32, hour: h.clamp(0, 23) as u32, minute: m.clamp(0, 59) as u32, ..Default::default() };
    dt.dow = weekday_from_days(dt.days());
    Some(dt)
}

impl Countdown {
    pub fn load(v: &Value) -> Countdown {
        Countdown { c: jget(v) }
    }
    fn secs_left(&self) -> Option<i64> {
        let t = parse_date(&self.c.date, &self.c.time)?;
        Some(t.local_secs() - local_now().local_secs())
    }
}

impl Widget for Countdown {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        match self.secs_left() {
            Some(s) if s.abs() < 86400 => Some(1000),
            _ => Some(60000),
        }
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        t.card_color(Color::hex(0x5E5CE6), Color::hex(0x3A2E8C));
        let inner = t.inner();
        let left = self.secs_left();
        let (big, small) = match left {
            None => ("?".to_string(), "data inválida".to_string()),
            Some(s) if s < 0 && !self.c.keep => ("0".into(), "chegou!".into()),
            Some(s) if s.abs() >= 86400 => {
                let d = (s.abs() as f64 / 86400.0).ceil() as i64;
                (d.to_string(), if s < 0 { "dias atrás".into() } else if d == 1 { "dia".into() } else { "dias".into() })
            }
            Some(s) => (fmt_duration_short(s.abs()), if s < 0 { "atrás".into() } else { "faltam".into() }),
        };
        if t.wide() {
            let left_w = inner.w * 0.42;
            t.fit_display(&big, Rect::new(inner.x, inner.y, left_w, inner.h), t.fs(0.46), W_BOLD, WHITE, Align::Center);
            let rr = Rect::new(inner.x + left_w + t.s * 0.08, inner.y, inner.w - left_w - t.s * 0.08, inner.h);
            t.fit_text(&self.c.title, Rect::new(rr.x, rr.y, rr.w, rr.h * 0.55), t.fs(0.17), W_SEMIBOLD, WHITE, Align::Left);
            t.fit_text(&small, Rect::new(rr.x, rr.y + rr.h * 0.5, rr.w, rr.h * 0.42), t.fs(0.14), W_NORMAL, WHITE.a(0.7), Align::Left);
        } else {
            t.fit_display(&big, Rect::new(inner.x, inner.y, inner.w, inner.h * 0.66), t.fs(0.4), W_BOLD, WHITE, Align::Center);
            t.fit_text(&small, Rect::new(inner.x, inner.y + inner.h * 0.62, inner.w, inner.h * 0.36), t.fs(0.13), W_MEDIUM, WHITE.a(0.75), Align::Center);
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (290.0, 200.0)
    }
    fn panel_interval(&self, _sh: &Shared) -> Option<u32> {
        Some(1000)
    }
    fn panel(&mut self, ui: &mut Ui, _sh: &mut Shared) {
        panel_header(ui, &self.c.title, &format!("{} às {}", self.c.date, self.c.time));
        let s = self.secs_left().unwrap_or(0);
        let a = s.abs();
        let (d, h, m, sec) = (a / 86400, (a / 3600) % 24, (a / 60) % 60, a % 60);
        let r = ui.row(70.0);
        let w = r.w / 4.0;
        for (i, (v, l)) in [(d, "dias"), (h, "horas"), (m, "min"), (sec, "seg")].iter().enumerate() {
            let cr = Rect::new(r.x + w * i as f32, r.y, w, r.h);
            ui.c.display(&v.to_string(), Rect::new(cr.x, cr.y, cr.w, cr.h * 0.62), 28.0 * ui.s, W_SEMIBOLD, TXT, Align::Center);
            ui.c.text(l, Rect::new(cr.x, cr.y + cr.h * 0.6, cr.w, cr.h * 0.35), 11.5 * ui.s, W_NORMAL, TXT2, Align::Center);
        }
        ui.gap(6.0);
        ui.note(if s < 0 { "A data já passou." } else { "Edite a data com o botão direito › Editar widget…" });
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        if ui.labeled_input(id("t"), "Nome", &mut self.c.title, "Viagem, lançamento…").changed {
            sh.save = true;
        }
        if ui.labeled_input(id("d"), "Data (dd/mm/aaaa)", &mut self.c.date, "25/12/2026").changed {
            sh.save = true;
        }
        if ui.labeled_input(id("h"), "Hora (hh:mm)", &mut self.c.time, "00:00").changed {
            sh.save = true;
        }
        if parse_date(&self.c.date, &self.c.time).is_none() {
            ui.note("⚠ Data inválida.");
        }
        if ui.row_toggle(id("k"), "Continuar contando depois que passar", &mut self.c.keep) {
            sh.save = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Progresso do tempo
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct ProgCfg {
    track: u32,
}

pub struct Progress {
    c: ProgCfg,
}

pub fn period_progress(kind: u32) -> f32 {
    let n = local_now();
    let secs_today = (n.hour * 3600 + n.minute * 60 + n.second) as f32;
    match kind {
        0 => secs_today / 86400.0,
        1 => {
            let dow = (n.dow + 6) % 7; // segunda = 0
            (dow as f32 * 86400.0 + secs_today) / (7.0 * 86400.0)
        }
        2 => ((n.day - 1) as f32 * 86400.0 + secs_today) / (days_in_month(n.year, n.month) as f32 * 86400.0),
        _ => {
            let start = days_from_civil(n.year, 1, 1);
            let len = days_from_civil(n.year + 1, 1, 1) - start;
            ((n.days() - start) as f32 * 86400.0 + secs_today) / (len as f32 * 86400.0)
        }
    }
}

const PERIODS: [&str; 4] = ["Dia", "Semana", "Mês", "Ano"];

impl Progress {
    pub fn load(v: &Value) -> Progress {
        Progress { c: jget(v) }
    }
}

impl Widget for Progress {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        Some(60000)
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        t.card();
        let p = period_progress(self.c.track);
        let label = PERIODS[self.c.track as usize % 4];
        if t.wide() {
            let inner = t.inner();
            t.fit_text(label, Rect::new(inner.x, inner.y, inner.w * 0.6, inner.h * 0.4), t.fs(0.15), W_SEMIBOLD, WHITE.a(0.8), Align::Left);
            t.fit_display(&format!("{:.0}%", p * 100.0), Rect::new(inner.x, inner.y, inner.w, inner.h * 0.4), t.fs(0.17), W_SEMIBOLD, WHITE, Align::Right);
            bar(t.c, Rect::new(inner.x, inner.y + inner.h * 0.55, inner.w, inner.h * 0.2), p, Color(1.0, 1.0, 1.0, 0.14), TEAL);
        } else {
            ring_tile(t, p, TEAL, &format!("{:.0}%", p * 100.0), label);
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (280.0, 220.0)
    }
    fn panel(&mut self, ui: &mut Ui, _sh: &mut Shared) {
        panel_header(ui, "Progresso do tempo", "");
        for i in 0..4u32 {
            let p = period_progress(i);
            let r = ui.row(22.0);
            ui.c.text(PERIODS[i as usize], r, 12.5 * ui.s, W_MEDIUM, TXT, Align::Left);
            ui.c.text(&format!("{:.1}%", p * 100.0), r, 12.5 * ui.s, W_SEMIBOLD, TXT2, Align::Right);
            let r = ui.row(14.0);
            bar(ui.c, Rect::new(r.x, r.y + 2.0, r.w, 8.0 * ui.s), p, Color(1.0, 1.0, 1.0, 0.12), if i == self.c.track { TEAL } else { Color(1.0, 1.0, 1.0, 0.5) });
            ui.gap(4.0);
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        if ui.labeled_segmented(id("tr"), "Acompanhar", &PERIODS, &mut self.c.track) {
            sh.save = true;
        }
    }
}
