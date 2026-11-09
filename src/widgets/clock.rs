// Relógio digital, analógico e mundial (mesmo widget, ajustes diferentes).
use serde::{Deserialize, Serialize};
use serde_json::Value;
use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::Globalization::*;
use windows::Win32::System::Time::*;

use super::*;
use crate::ui::*;

pub const CITIES: &[(&str, &str, &str)] = &[
    ("", "Este PC", ""),
    ("London", "Londres", "GMT Standard Time"),
    ("NewYork", "Nova York", "Eastern Standard Time"),
    ("Tokyo", "Tóquio", "Tokyo Standard Time"),
    ("Paris", "Paris", "Romance Standard Time"),
    ("Berlin", "Berlim", "W. Europe Standard Time"),
    ("Lisbon", "Lisboa", "GMT Standard Time"),
    ("SaoPaulo", "São Paulo", "E. South America Standard Time"),
    ("LosAngeles", "Los Angeles", "Pacific Standard Time"),
    ("Chicago", "Chicago", "Central Standard Time"),
    ("Dubai", "Dubai", "Arabian Standard Time"),
    ("Singapore", "Singapura", "Singapore Standard Time"),
    ("Sydney", "Sydney", "AUS Eastern Standard Time"),
    ("Delhi", "Nova Délhi", "India Standard Time"),
];

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
struct Cfg {
    tz: String,
    hours: u32,
    seconds: bool,
    analog: bool,
}

pub struct Clock {
    cfg: Cfg,
    tzi: Option<DYNAMIC_TIME_ZONE_INFORMATION>,
    tz_loaded_for: Option<String>,
}

pub fn find_tz(key: &str) -> Option<DYNAMIC_TIME_ZONE_INFORMATION> {
    unsafe {
        let mut i = 0u32;
        loop {
            let mut d = DYNAMIC_TIME_ZONE_INFORMATION::default();
            if EnumDynamicTimeZoneInformation(i, &mut d) != 0 {
                return None;
            }
            if from_wide(&d.TimeZoneKeyName) == key {
                return Some(d);
            }
            i += 1;
        }
    }
}

pub fn time_in(tzi: &Option<DYNAMIC_TIME_ZONE_INFORMATION>) -> DateTime {
    match tzi {
        None => local_now(),
        Some(d) => unsafe {
            let utc = windows::Win32::System::SystemInformation::GetSystemTime();
            let mut out = SYSTEMTIME::default();
            let tzi = TIME_ZONE_INFORMATION {
                Bias: d.Bias,
                StandardName: d.StandardName,
                StandardDate: d.StandardDate,
                StandardBias: d.StandardBias,
                DaylightName: d.DaylightName,
                DaylightDate: d.DaylightDate,
                DaylightBias: d.DaylightBias,
            };
            if SystemTimeToTzSpecificLocalTime(Some(&tzi), &utc, &mut out).is_ok() {
                DateTime::from_systemtime(&out)
            } else {
                local_now()
            }
        },
    }
}

pub fn system_24h() -> bool {
    unsafe {
        let mut buf = [0u16; 4];
        let n = GetLocaleInfoEx(windows::core::PCWSTR::null(), LOCALE_ITIME, Some(&mut buf));
        n > 0 && buf[0] == '1' as u16
    }
}

pub fn fmt_time(dt: &DateTime, mode: u32, secs: bool) -> (String, String) {
    let h24 = match mode {
        1 => true,
        2 => false,
        _ => system_24h(),
    };
    let (h, suffix) = if h24 {
        (dt.hour, String::new())
    } else {
        let h = if dt.hour % 12 == 0 { 12 } else { dt.hour % 12 };
        (h, if dt.hour < 12 { "AM".into() } else { "PM".into() })
    };
    let t = if secs { format!("{}:{:02}:{:02}", if h24 { format!("{:02}", h) } else { h.to_string() }, dt.minute, dt.second) } else { format!("{}:{:02}", if h24 { format!("{:02}", h) } else { h.to_string() }, dt.minute) };
    (t, suffix)
}

pub fn city_name(key: &str) -> &'static str {
    CITIES.iter().find(|c| c.0 == key).map(|c| c.1).unwrap_or("Este PC")
}

pub fn draw_analog(c: &Canvas, cx: f32, cy: f32, r: f32, dt: &DateTime, secs: bool, dark: bool) {
    let face = if dark { Color::rgba(28, 28, 32, 1.0) } else { Color::rgba(250, 250, 252, 1.0) };
    let ink = if dark { WHITE } else { Color::rgba(20, 20, 24, 1.0) };
    c.fill_circle(cx, cy, r, face);
    c.stroke_circle(cx, cy, r - 0.5, Color(0.5, 0.5, 0.55, 0.35), 1.0);
    for i in 0..12 {
        let a = (i as f32 * 30.0).to_radians();
        let (s, co) = (a.sin(), -a.cos());
        let l = if i % 3 == 0 { 0.16 } else { 0.08 };
        c.line(cx + s * r * (0.86 - l), cy + co * r * (0.86 - l), cx + s * r * 0.86, cy + co * r * 0.86, ink.a(if i % 3 == 0 { 0.9 } else { 0.45 }), (r * 0.045).max(1.0));
    }
    let hf = ((dt.hour % 12) as f32 + dt.minute as f32 / 60.0) / 12.0;
    let mf = (dt.minute as f32 + dt.second as f32 / 60.0) / 60.0;
    let hand = |f: f32, len: f32, w: f32, col: Color| {
        let a = (f * 360.0).to_radians();
        c.line(cx - a.sin() * r * 0.1, cy + a.cos() * r * 0.1, cx + a.sin() * r * len, cy - a.cos() * r * len, col, w);
    };
    hand(hf, 0.5, (r * 0.09).max(1.5), ink);
    hand(mf, 0.74, (r * 0.065).max(1.2), ink);
    if secs {
        hand(dt.second as f32 / 60.0, 0.8, (r * 0.03).max(1.0), ORANGE);
    }
    c.fill_circle(cx, cy, (r * 0.06).max(1.5), if secs { ORANGE } else { ink });
}

impl Clock {
    pub fn load(v: &Value) -> Clock {
        let cfg: Cfg = jget(v);
        Clock { cfg, tzi: None, tz_loaded_for: None }
    }
    fn now(&mut self) -> DateTime {
        if self.tz_loaded_for.as_deref() != Some(&self.cfg.tz) {
            let key = CITIES.iter().find(|c| c.0 == self.cfg.tz).map(|c| c.2).unwrap_or("");
            self.tzi = if key.is_empty() { None } else { find_tz(key) };
            self.tz_loaded_for = Some(self.cfg.tz.clone());
        }
        time_in(&self.tzi)
    }
}

impl Widget for Clock {
    fn save(&self) -> Value {
        jput(&self.cfg)
    }
    fn styles(&self) -> &'static [&'static str] {
        &[]
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        Some(if self.cfg.seconds { 1000 } else { 60000 })
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        let dt = self.now();
        if self.cfg.analog && !t.wide() {
            t.card();
            draw_analog(t.c, t.r.cx(), t.r.cy(), t.r.h * 0.4, &dt, self.cfg.seconds, false);
            return;
        }
        t.card();
        let inner = t.inner();
        if self.cfg.analog {
            let d = inner.h;
            draw_analog(t.c, inner.x + d / 2.0, inner.cy(), d * 0.48, &dt, self.cfg.seconds, false);
            let tr = Rect::new(inner.x + d + t.s * 0.12, inner.y, inner.w - d - t.s * 0.12, inner.h);
            let (tm, suf) = fmt_time(&dt, self.cfg.hours, false);
            t.fit_display(&format!("{} {}", tm, suf).trim().to_string(), Rect::new(tr.x, tr.y, tr.w, tr.h * 0.6), t.fs(0.3), W_SEMIBOLD, WHITE, Align::Left);
            t.fit_text(city_name(&self.cfg.tz), Rect::new(tr.x, tr.y + tr.h * 0.55, tr.w, tr.h * 0.4), t.fs(0.15), W_NORMAL, WHITE.a(0.6), Align::Left);
            return;
        }
        let (tm, suf) = fmt_time(&dt, self.cfg.hours, self.cfg.seconds);
        let show_city = !self.cfg.tz.is_empty() || t.wide();
        let main_h = if show_city { inner.h * 0.66 } else { inner.h };
        let tr = Rect::new(inner.x, inner.y, inner.w, main_h);
        let text = if suf.is_empty() { tm.clone() } else { format!("{} {}", tm, suf) };
        t.fit_display(&text, tr, t.fs(if t.wide() { 0.42 } else { 0.3 }), W_SEMIBOLD, WHITE, Align::Center);
        if show_city {
            let label = if self.cfg.tz.is_empty() {
                let d = local_now();
                format!("{} {} {}", DIAS_CURTO[d.dow as usize], d.day, MESES_CURTO[(d.month - 1) as usize])
            } else {
                city_name(&self.cfg.tz).to_string()
            };
            t.fit_text(&label, Rect::new(inner.x, inner.y + main_h - t.s * 0.04, inner.w, inner.h - main_h), t.fs(0.15), W_MEDIUM, WHITE.a(0.62), Align::Center);
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (300.0, 330.0)
    }
    fn panel_interval(&self, _sh: &Shared) -> Option<u32> {
        Some(1000)
    }
    fn panel(&mut self, ui: &mut Ui, _sh: &mut Shared) {
        let dt = self.now();
        let face = ui.row(150.0);
        draw_analog(ui.c, face.cx(), face.cy(), face.h * 0.46, &dt, true, true);
        let (tm, suf) = fmt_time(&dt, self.cfg.hours, true);
        let r = ui.row(40.0);
        ui.c.display(&format!("{} {}", tm, suf).trim().to_string(), r, 30.0 * ui.s, W_SEMIBOLD, TXT, Align::Center);
        let r = ui.row(20.0);
        ui.c.text(&format!("{}, {} de {} de {}", DIAS[dt.dow as usize], dt.day, MESES[(dt.month - 1) as usize], dt.year), r, 12.5 * ui.s, W_NORMAL, TXT2, Align::Center);
        let r = ui.row(18.0);
        ui.c.text(city_name(&self.cfg.tz), r, 11.5 * ui.s, W_NORMAL, TXT3, Align::Center);
        ui.gap(8.0);
        ui.separator();
        // outros fusos
        for key in ["London", "NewYork", "Tokyo"] {
            if key == self.cfg.tz {
                continue;
            }
            let tz = find_tz(CITIES.iter().find(|c| c.0 == key).unwrap().2);
            let d = time_in(&tz);
            let r = ui.row(22.0);
            ui.c.text(city_name(key), r, 12.0 * ui.s, W_NORMAL, TXT2, Align::Left);
            ui.c.text(&fmt_time(&d, self.cfg.hours, false).0, r, 12.0 * ui.s, W_SEMIBOLD, TXT, Align::Right);
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        ui.caption("FUSO HORÁRIO");
        let cols = 3;
        let names: Vec<(&str, &str)> = CITIES.iter().map(|c| (c.0, c.1)).collect();
        for chunk in names.chunks(cols) {
            let r = ui.row(30.0);
            let w = r.w / cols as f32;
            for (i, (k, n)) in chunk.iter().enumerate() {
                let br = Rect::new(r.x + w * i as f32 + 2.0, r.y + 2.0, w - 4.0, r.h - 4.0);
                let kind = if self.cfg.tz == *k { Btn::Primary } else { Btn::Normal };
                if ui.button_k(idn("tz", i as u64 + (r.y as u64) * 31), br, n, kind) {
                    self.cfg.tz = k.to_string();
                    sh.save = true;
                }
            }
        }
        ui.gap(6.0);
        if ui.labeled_segmented(id("hours"), "Horas", &["Sistema", "24 h", "12 h"], &mut self.cfg.hours) {
            sh.save = true;
        }
        if ui.row_toggle(id("secs"), "Mostrar segundos", &mut self.cfg.seconds) {
            sh.save = true;
        }
        if ui.row_toggle(id("analog"), "Mostrador analógico", &mut self.cfg.analog) {
            sh.save = true;
        }
    }
}
