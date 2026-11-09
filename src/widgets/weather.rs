// Clima pela Open-Meteo (sem chave e sem pedir localização).
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::mpsc::{channel, Receiver};

use super::*;
use crate::net;
use crate::ui::*;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct Cfg {
    city: String,
    lat: f64,
    lon: f64,
    fahrenheit: bool,
    feels: bool,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg { city: "São Paulo".into(), lat: -23.5475, lon: -46.6361, fahrenheit: false, feels: true }
    }
}

#[derive(Clone, Default, Debug)]
struct Day {
    dow: u32,
    code: i64,
    max: f64,
    min: f64,
    rain: i64,
}

#[derive(Clone, Default, Debug)]
struct Data {
    temp: f64,
    feels: f64,
    code: i64,
    day: bool,
    humidity: f64,
    wind: f64,
    days: Vec<Day>,
    hours: Vec<(u32, f64, i64)>,
}

enum Msg {
    Weather(Option<Data>),
    Search(Vec<(String, String, f64, f64)>),
}

pub struct Weather {
    c: Cfg,
    data: Option<Data>,
    rx: Option<Receiver<Msg>>,
    last: u64,
    search: String,
    results: Vec<(String, String, f64, f64)>,
    searching: bool,
    failed: bool,
}

pub fn describe(code: i64) -> &'static str {
    match code {
        0 => "Céu limpo",
        1 => "Quase limpo",
        2 => "Parcialmente nublado",
        3 => "Nublado",
        45 | 48 => "Neblina",
        51 | 53 | 55 => "Garoa",
        56 | 57 => "Garoa gelada",
        61 | 63 => "Chuva",
        65 => "Chuva forte",
        66 | 67 => "Chuva gelada",
        71 | 73 | 75 | 77 => "Neve",
        80 | 81 => "Pancadas de chuva",
        82 => "Temporal",
        85 | 86 => "Neve",
        95 => "Trovoadas",
        96 | 99 => "Trovoadas com granizo",
        _ => "—",
    }
}

/// Desenha o ícone do tempo de forma vetorial.
pub fn draw_icon(c: &Canvas, r: Rect, code: i64, day: bool) {
    let s = r.w.min(r.h);
    let cx = r.cx();
    let cy = r.cy();
    let sun = |x: f32, y: f32, rad: f32| {
        for i in 0..8 {
            let a = (i as f32 * 45.0).to_radians();
            c.line(x + a.cos() * rad * 1.35, y + a.sin() * rad * 1.35, x + a.cos() * rad * 1.75, y + a.sin() * rad * 1.75, Color::hex(0xFFD60A), (s * 0.05).max(1.0));
        }
        c.fill_circle(x, y, rad, Color::hex(0xFFD60A));
    };
    let moon = |x: f32, y: f32, rad: f32| {
        c.fill_circle(x, y, rad, Color::hex(0xF2F2F7));
        c.fill_circle(x + rad * 0.45, y - rad * 0.35, rad * 0.85, Color::hex(0x2C2C40));
    };
    let cloud = |x: f32, y: f32, w: f32, col: Color| {
        c.fill_circle(x - w * 0.22, y + w * 0.05, w * 0.22, col);
        c.fill_circle(x + w * 0.05, y - w * 0.08, w * 0.3, col);
        c.fill_circle(x + w * 0.3, y + w * 0.06, w * 0.2, col);
        c.fill_rrect(Rect::new(x - w * 0.42, y + w * 0.02, w * 0.9, w * 0.25), w * 0.12, col);
    };
    let clear = matches!(code, 0 | 1);
    let partly = code == 2;
    if clear || partly {
        if day {
            sun(cx - if partly { s * 0.1 } else { 0.0 }, cy - if partly { s * 0.08 } else { 0.0 }, s * if partly { 0.21 } else { 0.25 });
        } else {
            moon(cx - if partly { s * 0.1 } else { 0.0 }, cy - if partly { s * 0.08 } else { 0.0 }, s * 0.29);
        }
        if partly {
            cloud(cx + s * 0.06, cy + s * 0.1, s * 0.8, Color::hex(0xF2F2F7));
        }
        return;
    }
    let grey = if matches!(code, 95..=99) { Color::hex(0x8E8E9A) } else { Color::hex(0xD8D8E0) };
    cloud(cx, cy - s * 0.08, s * 0.95, grey);
    match code {
        45 | 48 => {
            for i in 0..3 {
                let y = cy + s * (0.2 + i as f32 * 0.1);
                c.line(cx - s * 0.32, y, cx + s * 0.32, y, Color(1.0, 1.0, 1.0, 0.7), (s * 0.04).max(1.0));
            }
        }
        51..=67 | 80..=82 => {
            for i in 0..3 {
                let x = cx - s * 0.2 + i as f32 * s * 0.2;
                c.line(x, cy + s * 0.18, x - s * 0.06, cy + s * 0.36, Color::hex(0x64B5FF), (s * 0.05).max(1.2));
            }
        }
        71..=77 | 85 | 86 => {
            for i in 0..3 {
                let x = cx - s * 0.2 + i as f32 * s * 0.2;
                c.fill_circle(x, cy + s * 0.27, s * 0.045, WHITE);
            }
        }
        95..=99 => {
            c.fill_poly(&[(cx + s * 0.02, cy + s * 0.1), (cx - s * 0.12, cy + s * 0.3), (cx, cy + s * 0.3), (cx - s * 0.06, cy + s * 0.45), (cx + s * 0.14, cy + s * 0.22), (cx + s * 0.02, cy + s * 0.22)], Color::hex(0xFFD60A));
        }
        _ => {}
    }
}

impl Weather {
    pub fn load(v: &Value) -> Weather {
        let mut w = Weather { c: jget(v), data: None, rx: None, last: 0, search: String::new(), results: vec![], searching: false, failed: false };
        if std::env::var("GLASSDOCK_TEST_FAKE_WEATHER").is_ok() {
            // só para inspeção visual sem internet
            w.data = Some(Data { temp: 21.0, feels: 22.0, code: 1, day: true, humidity: 60.0, wind: 8.0, days: (0..7).map(|i| Day { dow: i, code: [1, 2, 3, 61, 0, 95, 71][i as usize], max: 26.0, min: 17.0, rain: 30 }).collect(), hours: (0..6).map(|h| (12 + h, 21.0, 2)).collect() });
            return w;
        }
        w.fetch();
        w
    }

    fn fetch(&mut self) {
        if self.rx.is_some() {
            return;
        }
        self.last = tick_ms();
        let (lat, lon) = (self.c.lat, self.c.lon);
        let (tx, rx) = channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let url = format!(
                "https://api.open-meteo.com/v1/forecast?latitude={:.4}&longitude={:.4}&current=temperature_2m,apparent_temperature,weather_code,is_day,relative_humidity_2m,wind_speed_10m&hourly=temperature_2m,weather_code&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max&timezone=auto&forecast_days=7",
                lat, lon
            );
            let d = net::get_json(&url).and_then(|j| parse(&j));
            let _ = tx.send(Msg::Weather(d));
            crate::app::post(crate::app::WM_APP_POLL, 0, 0);
        });
    }

    fn search_city(&mut self) {
        let q = self.search.trim().to_string();
        if q.len() < 2 {
            return;
        }
        self.searching = true;
        let (tx, rx) = channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let url = format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=6&language=pt&format=json", net::url_encode(&q));
            let mut out = vec![];
            if let Some(j) = net::get_json(&url) {
                if let Some(arr) = j["results"].as_array() {
                    for r in arr {
                        let name = r["name"].as_str().unwrap_or("").to_string();
                        let region = [r["admin1"].as_str().unwrap_or(""), r["country"].as_str().unwrap_or("")].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(", ");
                        out.push((name, region, r["latitude"].as_f64().unwrap_or(0.0), r["longitude"].as_f64().unwrap_or(0.0)));
                    }
                }
            }
            let _ = tx.send(Msg::Search(out));
            crate::app::post(crate::app::WM_APP_POLL, 0, 0);
        });
    }

    fn deg(&self, c: f64) -> String {
        if self.c.fahrenheit {
            format!("{:.0}°", c * 9.0 / 5.0 + 32.0)
        } else {
            format!("{:.0}°", c)
        }
    }
}

fn parse(j: &Value) -> Option<Data> {
    let cur = &j["current"];
    let mut d = Data {
        temp: cur["temperature_2m"].as_f64()?,
        feels: cur["apparent_temperature"].as_f64().unwrap_or(0.0),
        code: cur["weather_code"].as_i64().unwrap_or(0),
        day: cur["is_day"].as_i64().unwrap_or(1) == 1,
        humidity: cur["relative_humidity_2m"].as_f64().unwrap_or(0.0),
        wind: cur["wind_speed_10m"].as_f64().unwrap_or(0.0),
        days: vec![],
        hours: vec![],
    };
    let daily = &j["daily"];
    if let Some(times) = daily["time"].as_array() {
        for (i, t) in times.iter().enumerate() {
            let s = t.as_str().unwrap_or("");
            let p: Vec<i64> = s.split('-').filter_map(|x| x.parse().ok()).collect();
            let dow = if p.len() == 3 { weekday_from_days(days_from_civil(p[0] as i32, p[1] as u32, p[2] as u32)) } else { 0 };
            d.days.push(Day {
                dow,
                code: daily["weather_code"][i].as_i64().unwrap_or(0),
                max: daily["temperature_2m_max"][i].as_f64().unwrap_or(0.0),
                min: daily["temperature_2m_min"][i].as_f64().unwrap_or(0.0),
                rain: daily["precipitation_probability_max"][i].as_i64().unwrap_or(0),
            });
        }
    }
    let hourly = &j["hourly"];
    let cur_time = cur["time"].as_str().unwrap_or("").get(0..13).unwrap_or("").to_string();
    if let Some(times) = hourly["time"].as_array() {
        let start = times.iter().position(|t| t.as_str().unwrap_or("").starts_with(&cur_time)).unwrap_or(0);
        for i in start..(start + 6).min(times.len()) {
            let h: u32 = times[i].as_str().unwrap_or("").get(11..13).and_then(|x| x.parse().ok()).unwrap_or(0);
            d.hours.push((h, hourly["temperature_2m"][i].as_f64().unwrap_or(0.0), hourly["weather_code"][i].as_i64().unwrap_or(0)));
        }
    }
    Some(d)
}

impl Widget for Weather {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn styles(&self) -> &'static [&'static str] {
        &["Agora", "Condições", "Previsão"]
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        Some(60000)
    }
    fn tick(&mut self, _sh: &mut Shared) -> bool {
        if tick_ms().saturating_sub(self.last) > 20 * 60 * 1000 || (self.data.is_none() && tick_ms().saturating_sub(self.last) > 60 * 1000) {
            self.fetch();
        }
        false
    }
    fn poll(&mut self, _sh: &mut Shared) -> bool {
        let Some(rx) = &self.rx else { return false };
        match rx.try_recv() {
            Ok(Msg::Weather(d)) => {
                self.failed = d.is_none();
                if d.is_some() {
                    self.data = d;
                }
                self.rx = None;
                true
            }
            Ok(Msg::Search(r)) => {
                self.results = r;
                self.searching = false;
                self.rx = None;
                true
            }
            Err(_) => false,
        }
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        let day = self.data.as_ref().map(|d| d.day).unwrap_or(true);
        if day {
            t.card_color(Color::hex(0x3A8DDE), Color::hex(0x1F5FA8));
        } else {
            t.card_color(Color::hex(0x2C2C40), Color::hex(0x15152A));
        }
        let inner = t.inner();
        let Some(d) = self.data.clone() else {
            t.c.glyph(0xE753, inner, t.fs(0.3), WHITE.a(0.8));
            return;
        };
        let style = if t.units < 2 { 0 } else { t.style };
        match style {
            2 if t.units >= 3 => {
                let n = d.days.len().min(5);
                let w = inner.w / n as f32;
                for (i, dd) in d.days.iter().take(n).enumerate() {
                    let r = Rect::new(inner.x + w * i as f32, inner.y, w, inner.h);
                    t.c.text(if i == 0 { "Hoje" } else { &DIAS_CURTO[dd.dow as usize][..3] }, Rect::new(r.x, r.y, r.w, r.h * 0.26), t.fs(0.11), W_SEMIBOLD, WHITE.a(0.8), Align::Center);
                    draw_icon(t.c, Rect::new(r.x, r.y + r.h * 0.24, r.w, r.h * 0.42), dd.code, true);
                    t.c.text(&format!("{} {}", self.deg(dd.max), self.deg(dd.min)), Rect::new(r.x, r.y + r.h * 0.7, r.w, r.h * 0.3), t.fs(0.11), W_MEDIUM, WHITE, Align::Center);
                }
            }
            1 | 2 => {
                let ic = inner.h;
                draw_icon(t.c, Rect::new(inner.x, inner.y, ic, ic), d.code, d.day);
                let r = Rect::new(inner.x + ic + t.s * 0.06, inner.y, inner.w - ic - t.s * 0.06, inner.h);
                t.fit_display(&self.deg(d.temp), Rect::new(r.x, r.y, r.w * 0.5, r.h * 0.55), t.fs(0.3), W_SEMIBOLD, WHITE, Align::Left);
                if d.days.first().is_some() && r.w > t.s * 0.9 {
                    let dd = &d.days[0];
                    t.c.text(&format!("↑{} ↓{}", self.deg(dd.max), self.deg(dd.min)), Rect::new(r.x + r.w * 0.45, r.y, r.w * 0.55, r.h * 0.55), t.fs(0.12), W_MEDIUM, WHITE.a(0.85), Align::Right);
                }
                let sub = if self.c.feels { format!("{} · sensação {}", self.c.city, self.deg(d.feels)) } else { format!("{} · {}", self.c.city, describe(d.code)) };
                t.fit_text(&sub, Rect::new(r.x, r.y + r.h * 0.52, r.w, r.h * 0.42), t.fs(0.125), W_NORMAL, WHITE.a(0.85), Align::Left);
            }
            _ => {
                // ícone grande em cima, temperatura embaixo
                let ir = Rect::new(t.r.x, t.r.y + t.s * 0.04, t.r.w, t.r.h * 0.6);
                draw_icon(t.c, ir, d.code, d.day);
                t.fit_display(&self.deg(d.temp), Rect::new(inner.x, t.r.y + t.r.h * 0.6, inner.w, t.r.h * 0.34), t.fs(0.25), W_SEMIBOLD, WHITE, Align::Center);
            }
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        if self.data.is_none() {
            (340.0, 130.0 + self.results.len() as f32 * 32.0)
        } else {
            (340.0, 440.0)
        }
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let Some(d) = self.data.clone() else {
            panel_header(ui, &self.c.city, if self.failed { "Sem conexão com o serviço de clima." } else { "Carregando…" });
            self.city_picker(ui, sh);
            return;
        };
        let r = ui.row(84.0);
        draw_icon(ui.c, Rect::new(r.x, r.y, r.h, r.h), d.code, d.day);
        ui.c.display(&self.deg(d.temp), Rect::new(r.x + r.h + 8.0 * ui.s, r.y, 110.0 * ui.s, r.h * 0.7), 40.0 * ui.s, W_SEMIBOLD, TXT, Align::Left);
        ui.c.text(describe(d.code), Rect::new(r.x + r.h + 8.0 * ui.s, r.y + r.h * 0.62, r.w, r.h * 0.35), 12.5 * ui.s, W_NORMAL, TXT2, Align::Left);
        ui.c.text(&self.c.city, Rect::new(r.x, r.y, r.w, r.h * 0.4), 13.5 * ui.s, W_SEMIBOLD, TXT, Align::Right);
        ui.c.text(&format!("Sensação {}", self.deg(d.feels)), Rect::new(r.x, r.y + r.h * 0.35, r.w, r.h * 0.3), 11.5 * ui.s, W_NORMAL, TXT2, Align::Right);
        ui.c.text(&format!("Umidade {:.0}% · Vento {:.0} km/h", d.humidity, d.wind), Rect::new(r.x, r.y + r.h * 0.62, r.w, r.h * 0.35), 11.5 * ui.s, W_NORMAL, TXT2, Align::Right);
        ui.gap(6.0);
        // próximas horas
        let r = ui.row(70.0);
        let n = d.hours.len().max(1);
        let w = r.w / n as f32;
        for (i, (h, tmp, code)) in d.hours.iter().enumerate() {
            let cr = Rect::new(r.x + w * i as f32, r.y, w, r.h);
            ui.c.text(if i == 0 { "Agora".to_string() } else { format!("{}h", h) }.as_str(), Rect::new(cr.x, cr.y, cr.w, 16.0 * ui.s), 11.0 * ui.s, W_NORMAL, TXT2, Align::Center);
            draw_icon(ui.c, Rect::new(cr.x, cr.y + 16.0 * ui.s, cr.w, 30.0 * ui.s), *code, (6..19).contains(h));
            ui.c.text(&self.deg(*tmp), Rect::new(cr.x, cr.y + 48.0 * ui.s, cr.w, 18.0 * ui.s), 12.0 * ui.s, W_SEMIBOLD, TXT, Align::Center);
        }
        ui.separator();
        let (gmin, gmax) = d.days.iter().fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x.min), b.max(x.max)));
        for (i, dd) in d.days.iter().enumerate() {
            let r = ui.row(30.0);
            ui.c.text(if i == 0 { "Hoje" } else { DIAS[dd.dow as usize] }, Rect::new(r.x, r.y, 90.0 * ui.s, r.h), 12.5 * ui.s, W_NORMAL, TXT, Align::Left);
            draw_icon(ui.c, Rect::new(r.x + 92.0 * ui.s, r.y + 3.0, 26.0 * ui.s, r.h - 6.0), dd.code, true);
            if dd.rain >= 20 {
                ui.c.text(&format!("{}%", dd.rain), Rect::new(r.x + 120.0 * ui.s, r.y, 34.0 * ui.s, r.h), 10.5 * ui.s, W_NORMAL, Color::hex(0x64B5FF), Align::Left);
            }
            let bx = r.x + 160.0 * ui.s;
            let bw = r.w - 160.0 * ui.s - 76.0 * ui.s;
            let span = (gmax - gmin).max(1.0);
            let a = ((dd.min - gmin) / span) as f32;
            let b = ((dd.max - gmin) / span) as f32;
            let track = Rect::new(bx + 30.0 * ui.s, r.cy() - 2.5 * ui.s, bw - 30.0 * ui.s, 5.0 * ui.s);
            ui.c.fill_rrect(track, 2.5 * ui.s, Color(1.0, 1.0, 1.0, 0.1));
            ui.c.fill_rrect_grad(Rect::new(track.x + track.w * a, track.y, (track.w * (b - a)).max(5.0), track.h), 2.5 * ui.s, 90.0, &[(0.0, TEAL), (1.0, ORANGE)]);
            ui.c.text(&self.deg(dd.max), Rect::new(r.r() - 34.0 * ui.s, r.y, 34.0 * ui.s, r.h), 12.0 * ui.s, W_SEMIBOLD, TXT, Align::Right);
            ui.c.text(&self.deg(dd.min), Rect::new(bx - 6.0 * ui.s, r.y, 34.0 * ui.s, r.h), 12.0 * ui.s, W_NORMAL, TXT2, Align::Right);
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        self.city_picker(ui, sh);
        let mut f = self.c.fahrenheit as u32;
        if ui.labeled_segmented(id("unit"), "Unidade", &["Celsius", "Fahrenheit"], &mut f) {
            self.c.fahrenheit = f == 1;
            sh.save = true;
        }
        if ui.row_toggle(id("feels"), "Mostrar a sensação térmica", &mut self.c.feels) {
            sh.save = true;
        }
    }
}

impl Weather {
    fn city_picker(&mut self, ui: &mut Ui, sh: &mut Shared) {
        ui.caption(&format!("CIDADE: {}", self.c.city.to_uppercase()));
        let r = ui.row(34.0);
        let bw = 80.0 * ui.s;
        let res = ui.text_input(id("city"), Rect::new(r.x, r.y + 2.0, r.w - bw - 8.0, r.h - 4.0), &mut self.search, "Buscar cidade…", false);
        let go = ui.button_k(id("go"), Rect::new(r.r() - bw, r.y + 2.0, bw, r.h - 4.0), if self.searching { "…" } else { "Buscar" }, Btn::Primary);
        if go || res.submitted {
            self.search_city();
        }
        let mut chosen = None;
        for (i, (name, region, lat, lon)) in self.results.iter().enumerate() {
            let r = ui.row(32.0);
            let (hov, clk) = ui.hit(idn("res", i as u64), r);
            if hov {
                ui.c.fill_rrect(r, 6.0 * ui.s, FILL);
            }
            ui.c.text(name, Rect::new(r.x + 8.0 * ui.s, r.y, r.w * 0.45, r.h), 12.5 * ui.s, W_MEDIUM, TXT, Align::Left);
            ui.c.text(region, Rect::new(r.x + r.w * 0.45, r.y, r.w * 0.55 - 8.0 * ui.s, r.h), 11.5 * ui.s, W_NORMAL, TXT2, Align::Right);
            if clk {
                chosen = Some((name.clone(), *lat, *lon));
            }
        }
        if let Some((n, lat, lon)) = chosen {
            self.c.city = n;
            self.c.lat = lat;
            self.c.lon = lon;
            self.results.clear();
            self.search.clear();
            self.data = None;
            self.rx = None;
            self.fetch();
            sh.save = true;
        }
    }
}
