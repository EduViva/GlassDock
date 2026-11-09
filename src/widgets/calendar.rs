// Calendário: a data no ícone, o mês no painel e, se você colar o endereço
// ICS da sua agenda (Google, Outlook, iCloud), os próximos compromissos.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::mpsc::{channel, Receiver};

use super::*;
use crate::ui::*;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct Cfg {
    lookahead: u32,
    all_day: bool,
    urls: Vec<String>,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg { lookahead: 2, all_day: true, urls: vec![] }
    }
}

#[derive(Clone, Debug)]
pub struct Event {
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    pub title: String,
    pub cal: usize,
}

const CAL_COLORS: [u32; 5] = [0xFF453A, 0x0A84FF, 0x30D158, 0xFF9F0A, 0xBF5AF2];

pub struct Calendar {
    c: Cfg,
    events: Vec<Event>,
    rx: Option<Receiver<Vec<Event>>>,
    last_fetch: u64,
    month_off: i32,
    new_url: String,
    error: bool,
}

impl Calendar {
    pub fn load(v: &Value) -> Calendar {
        let mut c = Calendar { c: jget(v), events: vec![], rx: None, last_fetch: 0, month_off: 0, new_url: String::new(), error: false };
        c.fetch();
        c
    }

    fn fetch(&mut self) {
        if self.c.urls.is_empty() || self.rx.is_some() {
            return;
        }
        self.last_fetch = tick_ms();
        let urls = self.c.urls.clone();
        let (tx, rx) = channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let mut all = vec![];
            let now = unix_now();
            for (i, u) in urls.iter().enumerate() {
                let u = u.trim().replacen("webcal://", "https://", 1);
                if let Some(b) = crate::net::get(&u) {
                    let text = String::from_utf8_lossy(&b).to_string();
                    all.extend(parse_ics(&text, i, now - 86400 * 2, now + 86400 * 45));
                }
            }
            all.sort_by_key(|e| e.start);
            all.dedup_by(|a, b| a.start == b.start && a.title == b.title);
            let _ = tx.send(all);
            crate::app::post(crate::app::WM_APP_POLL, 0, 0);
        });
    }

    fn horizon(&self) -> i64 {
        match self.c.lookahead {
            0 => {
                let n = local_now();
                local_to_unix(&DateTime::from_local_secs(n.date_only().local_secs() + 86400))
            }
            1 => unix_now() + 3 * 86400,
            2 => unix_now() + 7 * 86400,
            _ => unix_now() + 31 * 86400,
        }
    }

    fn upcoming(&self) -> Vec<&Event> {
        let now = unix_now();
        let h = self.horizon();
        self.events.iter().filter(|e| e.end > now && e.start < h && (self.c.all_day || !e.all_day)).collect()
    }

    fn date_block(&self, t: &Tile, r: Rect) {
        let n = local_now();
        let rad = t.rad() * (r.w / t.s).min(1.0);
        t.c.fill_rrect(r, rad, Color(0.98, 0.98, 0.99, 1.0));
        let head = Rect::new(r.x, r.y + r.h * 0.08, r.w, r.h * 0.26);
        t.c.text(DIAS_CURTO[n.dow as usize], head, r.h * 0.17, W_BOLD, RED, Align::Center);
        let num = Rect::new(r.x, r.y + r.h * 0.28, r.w, r.h * 0.66);
        t.c.display(&n.day.to_string(), num, r.h * 0.52, W_NORMAL, Color::rgba(28, 28, 30, 1.0), Align::Center);
    }
}

fn when_text(e: &Event) -> String {
    let now = unix_now();
    if e.start <= now {
        return "agora".into();
    }
    let d = e.start - now;
    if !e.all_day && d < 3600 {
        return format!("em {} min", (d + 59) / 60);
    }
    let st = unix_to_local(e.start);
    let today = local_now().days();
    let day = st.days();
    let hm = if e.all_day { "dia todo".to_string() } else { format!("{:02}:{:02}", st.hour, st.minute) };
    if day == today {
        if e.all_day {
            "hoje".into()
        } else {
            format!("hoje {}", hm)
        }
    } else if day == today + 1 {
        format!("amanhã {}", hm)
    } else {
        format!("{} {} · {}", DIAS_CURTO[st.dow as usize].to_lowercase(), st.day, hm)
    }
}

impl Widget for Calendar {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn styles(&self) -> &'static [&'static str] {
        &["Próximo evento", "Data + próximo", "Data + agenda", "Data"]
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        Some(60000)
    }
    fn tick(&mut self, _sh: &mut Shared) -> bool {
        if !self.c.urls.is_empty() && tick_ms().saturating_sub(self.last_fetch) > 15 * 60 * 1000 {
            self.fetch();
        }
        true
    }
    fn poll(&mut self, _sh: &mut Shared) -> bool {
        if let Some(rx) = &self.rx {
            if let Ok(ev) = rx.try_recv() {
                self.error = ev.is_empty() && !self.c.urls.is_empty();
                self.events = ev;
                self.rx = None;
                return true;
            }
        }
        false
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        let style = if t.units < 2 { 3 } else { t.style };
        if style == 3 {
            // ícone de calendário
            let r = t.r;
            t.c.fill_rrect(r, t.rad(), Color(0.99, 0.99, 1.0, 1.0));
            let n = local_now();
            t.c.text(DIAS_CURTO[n.dow as usize], Rect::new(r.x, r.y + r.h * 0.07, r.w, r.h * 0.24), r.h * 0.17, W_SEMIBOLD, RED, Align::Center);
            t.c.display(&n.day.to_string(), Rect::new(r.x, r.y + r.h * 0.26, r.w, r.h * 0.66), r.h * 0.5, W_NORMAL, Color::rgba(28, 28, 30, 1.0), Align::Center);
            if !self.upcoming().is_empty() {
                t.c.fill_circle(r.cx(), r.b() - r.h * 0.09, r.h * 0.035, RED);
            }
            return;
        }
        t.card();
        let inner = t.inner();
        let ev = self.upcoming();
        let mut area = inner;
        if style == 1 || style == 2 {
            let d = inner.h;
            self.date_block(t, Rect::new(inner.x, inner.y, d, d));
            area = Rect::new(inner.x + d + t.s * 0.14, inner.y, inner.w - d - t.s * 0.14, inner.h);
        }
        if ev.is_empty() {
            let msg = if self.c.urls.is_empty() { "Sem agenda" } else { "Nada por agora" };
            t.fit_text(msg, Rect::new(area.x, area.y, area.w, area.h * 0.5), t.fs(0.16), W_SEMIBOLD, WHITE, Align::Left);
            let n = local_now();
            t.fit_text(&format!("{} de {}", n.day, MESES[(n.month - 1) as usize]), Rect::new(area.x, area.y + area.h * 0.48, area.w, area.h * 0.4), t.fs(0.13), W_NORMAL, WHITE.a(0.6), Align::Left);
            return;
        }
        if style == 2 {
            let n = ev.len().min(if inner.h > t.s * 0.7 { 3 } else { 2 });
            let lh = area.h / n as f32;
            for (i, e) in ev.iter().take(n).enumerate() {
                let r = Rect::new(area.x, area.y + lh * i as f32, area.w, lh);
                let col = Color::hex(CAL_COLORS[e.cal % CAL_COLORS.len()]);
                t.c.fill_rrect(Rect::new(r.x, r.y + lh * 0.18, t.s * 0.04, lh * 0.64), t.s * 0.02, col);
                let tr = Rect::new(r.x + t.s * 0.09, r.y, r.w - t.s * 0.09, r.h);
                let when = when_text(e);
                let (ww, _) = t.c.g.measure(&when, t.fs(0.12), W_NORMAL);
                t.fit_text(&e.title, Rect::new(tr.x, tr.y, tr.w - ww - t.s * 0.06, tr.h), t.fs(0.13), W_MEDIUM, WHITE, Align::Left);
                t.c.text(&when, tr, t.fs(0.12), W_NORMAL, WHITE.a(0.6), Align::Right);
            }
            return;
        }
        let e = ev[0];
        let col = Color::hex(CAL_COLORS[e.cal % CAL_COLORS.len()]);
        t.c.fill_rrect(Rect::new(area.x, area.y + area.h * 0.12, t.s * 0.05, area.h * 0.76), t.s * 0.025, col);
        let tr = Rect::new(area.x + t.s * 0.12, area.y, area.w - t.s * 0.12, area.h);
        t.fit_text(&e.title, Rect::new(tr.x, tr.y, tr.w, tr.h * 0.55), t.fs(0.165), W_SEMIBOLD, WHITE, Align::Left);
        t.fit_text(&when_text(e), Rect::new(tr.x, tr.y + tr.h * 0.5, tr.w, tr.h * 0.42), t.fs(0.135), W_NORMAL, col.mix(WHITE, 0.45), Align::Left);
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        let n = self.upcoming().len().min(6) as f32;
        (320.0, 300.0 + if n > 0.0 { 30.0 + n * 40.0 } else { 40.0 })
    }
    fn panel(&mut self, ui: &mut Ui, _sh: &mut Shared) {
        let n = local_now();
        let mut y = n.year;
        let mut m = n.month as i32 + self.month_off;
        while m < 1 {
            m += 12;
            y -= 1;
        }
        while m > 12 {
            m -= 12;
            y += 1;
        }
        let m = m as u32;
        let r = ui.row(32.0);
        ui.c.text(&format!("{} {}", capitalize(MESES[(m - 1) as usize]), y), Rect::new(r.x, r.y, r.w - 90.0 * ui.s, r.h), 15.0 * ui.s, W_SEMIBOLD, TXT, Align::Left);
        let b = 28.0 * ui.s;
        if ui.icon_btn(id("prev"), Rect::new(r.r() - 3.0 * b - 8.0, r.cy() - b / 2.0, b, b), 0xE76B, 11.0, TXT) {
            self.month_off -= 1;
        }
        if ui.icon_btn(id("today"), Rect::new(r.r() - 2.0 * b - 4.0, r.cy() - b / 2.0, b, b), 0xE8D1, 11.0, TXT) {
            self.month_off = 0;
        }
        if ui.icon_btn(id("next"), Rect::new(r.r() - b, r.cy() - b / 2.0, b, b), 0xE76C, 11.0, TXT) {
            self.month_off += 1;
        }
        ui.gap(4.0);
        let r = ui.row(20.0);
        let cw = r.w / 7.0;
        for (i, d) in ["D", "S", "T", "Q", "Q", "S", "S"].iter().enumerate() {
            ui.c.text(d, Rect::new(r.x + cw * i as f32, r.y, cw, r.h), 11.0 * ui.s, W_SEMIBOLD, TXT3, Align::Center);
        }
        let first = days_from_civil(y, m, 1);
        let start_dow = weekday_from_days(first);
        let dim = days_in_month(y, m);
        let today = n.days();
        let grid = ui.row(6.0 * 36.0);
        let ch = grid.h / 6.0;
        let ev_days: std::collections::HashSet<i64> = self.events.iter().map(|e| unix_to_local(e.start).days()).collect();
        for d in 0..dim {
            let cell = start_dow + d;
            let (row, col) = (cell / 7, cell % 7);
            let cr = Rect::new(grid.x + cw * col as f32, grid.y + ch * row as f32, cw, ch);
            let day = first + d as i64;
            let is_today = day == today;
            if is_today {
                ui.c.fill_circle(cr.cx(), cr.cy(), ch * 0.4, RED);
            }
            let col_txt = if is_today { WHITE } else if col == 0 || col == 6 { TXT2 } else { TXT };
            ui.c.text(&(d + 1).to_string(), cr, 13.0 * ui.s, if is_today { W_BOLD } else { W_NORMAL }, col_txt, Align::Center);
            if ev_days.contains(&day) && !is_today {
                ui.c.fill_circle(cr.cx(), cr.b() - ch * 0.12, 2.0 * ui.s, Color(1.0, 1.0, 1.0, 0.5));
            }
        }
        ui.separator();
        let ev: Vec<Event> = self.upcoming().into_iter().cloned().collect();
        if ev.is_empty() {
            if self.c.urls.is_empty() {
                ui.note("Para ver seus compromissos, cole o endereço ICS da sua agenda em Editar widget (botão direito).");
            } else if self.error {
                ui.note("Não consegui ler a agenda. Confira o endereço ICS.");
            } else {
                ui.note("Nenhum compromisso nos próximos dias.");
            }
            return;
        }
        let r = ui.row(22.0);
        ui.c.text("PRÓXIMOS", r, 11.0 * ui.s, W_SEMIBOLD, TXT2, Align::Left);
        for e in ev.iter().take(6) {
            let r = ui.row(40.0);
            let col = Color::hex(CAL_COLORS[e.cal % CAL_COLORS.len()]);
            ui.c.fill_rrect(Rect::new(r.x, r.y + 6.0 * ui.s, 4.0 * ui.s, r.h - 12.0 * ui.s), 2.0 * ui.s, col);
            ui.c.text(&e.title, Rect::new(r.x + 12.0 * ui.s, r.y + 3.0 * ui.s, r.w - 12.0 * ui.s, r.h * 0.5), 12.5 * ui.s, W_MEDIUM, TXT, Align::Left);
            let st = unix_to_local(e.start);
            let en = unix_to_local(e.end);
            let span = if e.all_day { "Dia todo".to_string() } else { format!("{:02}:{:02} – {:02}:{:02}", st.hour, st.minute, en.hour, en.minute) };
            ui.c.text(&format!("{} · {}", when_text(e), span), Rect::new(r.x + 12.0 * ui.s, r.y + r.h * 0.5, r.w - 12.0 * ui.s, r.h * 0.45), 11.5 * ui.s, W_NORMAL, TXT2, Align::Left);
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        if ui.labeled_segmented(id("la"), "Olhar adiante", &["Hoje", "3 dias", "Semana", "Mês"], &mut self.c.lookahead) {
            sh.save = true;
        }
        if ui.row_toggle(id("ad"), "Eventos de dia inteiro", &mut self.c.all_day) {
            sh.save = true;
        }
        ui.caption("AGENDAS (ENDEREÇO ICS)");
        ui.note("No Google Agenda: Configurações › sua agenda › Endereço secreto no formato iCal. No Outlook: Calendário › Compartilhar › Publicar › ICS.");
        let mut remove = None;
        for (i, u) in self.c.urls.clone().iter().enumerate() {
            let r = ui.row(30.0);
            let col = Color::hex(CAL_COLORS[i % CAL_COLORS.len()]);
            ui.c.fill_circle(r.x + 6.0 * ui.s, r.cy(), 5.0 * ui.s, col);
            ui.c.text(u, Rect::new(r.x + 18.0 * ui.s, r.y, r.w - 60.0 * ui.s, r.h), 11.5 * ui.s, W_NORMAL, TXT2, Align::Left);
            if ui.icon_btn(idn("rm", i as u64), Rect::new(r.r() - 28.0 * ui.s, r.y + 1.0, 28.0 * ui.s, 28.0 * ui.s), 0xE711, 10.0, TXT2) {
                remove = Some(i);
            }
        }
        if let Some(i) = remove {
            self.c.urls.remove(i);
            self.events.retain(|e| e.cal != i);
            sh.save = true;
        }
        let r = ui.row(34.0);
        let bw = 84.0 * ui.s;
        let res = ui.text_input(id("nu"), Rect::new(r.x, r.y + 2.0, r.w - bw - 8.0, r.h - 4.0), &mut self.new_url, "https://…/basic.ics", false);
        let add = ui.button_k(id("add"), Rect::new(r.r() - bw, r.y + 2.0, bw, r.h - 4.0), "Adicionar", Btn::Primary);
        if (add || res.submitted) && self.new_url.trim().len() > 8 {
            self.c.urls.push(self.new_url.trim().to_string());
            self.new_url.clear();
            sh.save = true;
            self.rx = None;
            self.fetch();
        }
        if !self.c.urls.is_empty() {
            let r = ui.row(34.0);
            if ui.button(id("ref"), Rect::new(r.x, r.y + 2.0, 140.0 * ui.s, r.h - 4.0), "Atualizar agora") {
                self.rx = None;
                self.fetch();
            }
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Leitor de ICS (VEVENT + RRULE simples)
// ---------------------------------------------------------------------------

fn parse_ics_time(prop: &str, val: &str) -> Option<(i64, bool)> {
    let all_day = prop.contains("VALUE=DATE") && !prop.contains("VALUE=DATE-TIME") || val.len() == 8;
    let v = val.trim();
    let y: i32 = v.get(0..4)?.parse().ok()?;
    let mo: u32 = v.get(4..6)?.parse().ok()?;
    let d: u32 = v.get(6..8)?.parse().ok()?;
    let (h, mi, s) = if v.len() >= 15 {
        (v.get(9..11)?.parse().ok()?, v.get(11..13)?.parse().ok()?, v.get(13..15)?.parse().ok()?)
    } else {
        (0, 0, 0)
    };
    let dt = DateTime { year: y, month: mo, day: d, hour: h, minute: mi, second: s, ..Default::default() };
    let ts = if v.ends_with('Z') { dt.local_secs() } else { local_to_unix(&dt) };
    Some((ts, all_day))
}

fn parse_ics(text: &str, cal: usize, from: i64, to: i64) -> Vec<Event> {
    let mut lines: Vec<String> = vec![];
    for raw in text.split('\n') {
        let l = raw.trim_end_matches('\r');
        if (l.starts_with(' ') || l.starts_with('\t')) && !lines.is_empty() {
            lines.last_mut().unwrap().push_str(&l[1..]);
        } else {
            lines.push(l.to_string());
        }
    }
    let mut out = vec![];
    let mut inside = false;
    let mut cur: Vec<(String, String)> = vec![];
    for l in lines {
        if l == "BEGIN:VEVENT" {
            inside = true;
            cur.clear();
            continue;
        }
        if l == "END:VEVENT" {
            inside = false;
            expand_event(&cur, cal, from, to, &mut out);
            continue;
        }
        if inside {
            if let Some(p) = l.find(':') {
                cur.push((l[..p].to_string(), l[p + 1..].to_string()));
            }
        }
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("\\,", ",").replace("\\;", ";").replace("\\n", " ").replace("\\N", " ").replace("\\\\", "\\")
}

fn expand_event(props: &[(String, String)], cal: usize, from: i64, to: i64, out: &mut Vec<Event>) {
    let get = |k: &str| props.iter().find(|(p, _)| p == k || p.starts_with(&format!("{};", k)));
    if let Some((_, st)) = get("STATUS") {
        if st.eq_ignore_ascii_case("CANCELLED") {
            return;
        }
    }
    if props.iter().any(|(p, _)| p.starts_with("RECURRENCE-ID")) {
        // instância alterada de uma série: tratada como evento avulso
    }
    let Some((sp, sv)) = get("DTSTART") else { return };
    let Some((start, all_day)) = parse_ics_time(sp, sv) else { return };
    let end = get("DTEND").and_then(|(p, v)| parse_ics_time(p, v)).map(|x| x.0).unwrap_or(start + if all_day { 86400 } else { 3600 });
    let dur = (end - start).max(0);
    let title = get("SUMMARY").map(|(_, v)| unescape(v)).unwrap_or_else(|| "(sem título)".into());
    let exdates: Vec<i64> = props
        .iter()
        .filter(|(p, _)| p.starts_with("EXDATE"))
        .flat_map(|(p, v)| v.split(',').filter_map(|x| parse_ics_time(p, x).map(|t| t.0)).collect::<Vec<_>>())
        .collect();
    let mut push = |s: i64| {
        if s + dur > from && s < to && !exdates.contains(&s) {
            out.push(Event { start: s, end: s + dur, all_day, title: title.clone(), cal });
        }
    };
    let Some((_, rrule)) = get("RRULE") else {
        push(start);
        return;
    };
    let mut freq = "";
    let mut interval = 1i64;
    let mut count: Option<i64> = None;
    let mut until: Option<i64> = None;
    let mut byday: Vec<u32> = vec![];
    for part in rrule.split(';') {
        let mut kv = part.splitn(2, '=');
        let k = kv.next().unwrap_or("");
        let v = kv.next().unwrap_or("");
        match k {
            "FREQ" => freq = v,
            "INTERVAL" => interval = v.parse().unwrap_or(1).max(1),
            "COUNT" => count = v.parse().ok(),
            "UNTIL" => until = parse_ics_time("", v).map(|t| t.0),
            "BYDAY" => {
                for d in v.split(',') {
                    let code = &d[d.len().saturating_sub(2)..];
                    if let Some(i) = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"].iter().position(|x| *x == code) {
                        byday.push(i as u32);
                    }
                }
            }
            _ => {}
        }
    }
    let base = unix_to_local(start);
    let tod = (base.hour * 3600 + base.minute * 60 + base.second) as i64;
    let mut n = 0i64;
    let limit = until.unwrap_or(i64::MAX).min(to);
    let mut emit = |s: i64, n: &mut i64| -> bool {
        if s > limit {
            return false;
        }
        if let Some(c) = count {
            if *n >= c {
                return false;
            }
        }
        *n += 1;
        push(s);
        true
    };
    match freq {
        "DAILY" => {
            let mut s = start;
            for _ in 0..2000 {
                if !emit(s, &mut n) {
                    break;
                }
                s += 86400 * interval;
            }
        }
        "WEEKLY" => {
            let days = if byday.is_empty() { vec![base.dow] } else { byday.clone() };
            let week0 = base.days() - base.dow as i64;
            'outer: for w in 0..1000i64 {
                let wk = week0 + w * 7 * interval;
                for d in 0..7u32 {
                    if !days.contains(&d) {
                        continue;
                    }
                    let day = wk + d as i64;
                    if day < base.days() {
                        continue;
                    }
                    let s = local_to_unix(&DateTime::from_local_secs(day * 86400 + tod));
                    if !emit(s, &mut n) {
                        break 'outer;
                    }
                }
            }
        }
        "MONTHLY" => {
            let (mut y, mut m) = (base.year, base.month as i32);
            for _ in 0..600 {
                if base.day <= days_in_month(y, m as u32) {
                    let day = days_from_civil(y, m as u32, base.day);
                    let s = local_to_unix(&DateTime::from_local_secs(day * 86400 + tod));
                    if !emit(s, &mut n) {
                        break;
                    }
                }
                m += interval as i32;
                while m > 12 {
                    m -= 12;
                    y += 1;
                }
            }
        }
        "YEARLY" => {
            let mut y = base.year;
            for _ in 0..200 {
                if base.day <= days_in_month(y, base.month) {
                    let day = days_from_civil(y, base.month, base.day);
                    let s = local_to_unix(&DateTime::from_local_secs(day * 86400 + tod));
                    if !emit(s, &mut n) {
                        break;
                    }
                }
                y += interval as i32;
            }
        }
        _ => push(start),
    }
}
