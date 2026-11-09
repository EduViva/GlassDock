// Post-it, Lista de tarefas e Beber água.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use super::*;
use crate::ui::*;

// ---------------------------------------------------------------------------
// Post-it
// ---------------------------------------------------------------------------

const NOTE_COLORS: [(u32, u32); 6] = [
    (0xFFE27A, 0xF7C948),
    (0xFFB3CF, 0xF78FB3),
    (0xA8D8FF, 0x7CC0F5),
    (0xB5EBA6, 0x8FD67C),
    (0xD3B8FF, 0xB894F5),
    (0xFF9C9C, 0xF57373),
];
const NOTE_NAMES: [&str; 6] = ["Amarelo", "Rosa", "Azul", "Verde", "Roxo", "Vermelho"];

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct NoteCfg {
    text: String,
    color: u32,
}

pub struct Note {
    c: NoteCfg,
}

impl Note {
    pub fn load(v: &Value) -> Note {
        Note { c: jget(v) }
    }
}

impl Widget for Note {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        let (a, b) = NOTE_COLORS[self.c.color as usize % NOTE_COLORS.len()];
        let rad = t.rad() * 0.7;
        t.c.fill_rrect_vgrad(t.r, rad, &[(0.0, Color::hex(a)), (1.0, Color::hex(b))]);
        // dobrinha no canto
        let f = t.s * 0.16;
        t.c.fill_poly(&[(t.r.r() - f, t.r.b()), (t.r.r(), t.r.b() - f), (t.r.r() - f, t.r.b() - f)], Color(0.0, 0.0, 0.0, 0.12));
        let inner = t.r.inset_xy(t.s * 0.12, t.s * 0.1);
        let txt = if self.c.text.trim().is_empty() { "Escreva um recado…" } else { self.c.text.trim() };
        let col = Color(0.12, 0.1, 0.05, if self.c.text.trim().is_empty() { 0.45 } else { 0.9 });
        t.c.push_clip(inner);
        t.c.text_wrap(txt, inner, t.fs(0.135), W_MEDIUM, col, Align::Left);
        t.c.pop_axis_clip();
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (320.0, 280.0)
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        panel_header(ui, "Post-it", "");
        let r = ui.row(170.0);
        if ui.text_input(id("txt"), r, &mut self.c.text, "Escreva um recado…", true).changed {
            sh.save = true;
        }
        ui.gap(8.0);
        let r = ui.row(28.0);
        let cols: Vec<Color> = NOTE_COLORS.iter().map(|c| Color::hex(c.0)).collect();
        if ui.swatches(id("c"), r, &cols, &mut self.c.color) {
            sh.save = true;
        }
        if ui.st.focus.is_none() {
            ui.st.focus = Some(id("txt"));
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        ui.caption("COR");
        let r = ui.row(30.0);
        let cols: Vec<Color> = NOTE_COLORS.iter().map(|c| Color::hex(c.0)).collect();
        if ui.swatches(id("c"), r, &cols, &mut self.c.color) {
            sh.save = true;
        }
        let r = ui.row(18.0);
        ui.c.text(NOTE_NAMES[self.c.color as usize % 6], r, 11.5 * ui.s, W_NORMAL, TXT2, Align::Left);
    }
}

// ---------------------------------------------------------------------------
// Lista de tarefas
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct TaskItem {
    text: String,
    done: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct TasksCfg {
    items: Vec<TaskItem>,
    show_done: bool,
}

pub struct Tasks {
    c: TasksCfg,
    input: String,
}

impl Tasks {
    pub fn load(v: &Value) -> Tasks {
        Tasks { c: jget(v), input: String::new() }
    }
    fn pending(&self) -> usize {
        self.c.items.iter().filter(|i| !i.done).count()
    }
}

impl Widget for Tasks {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        t.card();
        let inner = t.inner();
        let n = self.pending();
        if !t.wide() {
            t.c.glyph(0xE73E, Rect::new(inner.x, inner.y, inner.w, inner.h * 0.55), t.fs(0.3), ACCENT);
            t.fit_display(&n.to_string(), Rect::new(inner.x, inner.y + inner.h * 0.5, inner.w, inner.h * 0.5), t.fs(0.26), W_SEMIBOLD, WHITE, Align::Center);
            return;
        }
        let first = self.c.items.iter().filter(|i| !i.done).take(2).collect::<Vec<_>>();
        if first.is_empty() {
            t.fit_text("Tudo feito ✓", Rect::new(inner.x, inner.y, inner.w, inner.h * 0.5), t.fs(0.16), W_SEMIBOLD, WHITE, Align::Left);
            t.fit_text("Clique para anotar", Rect::new(inner.x, inner.y + inner.h * 0.5, inner.w, inner.h * 0.45), t.fs(0.13), W_NORMAL, WHITE.a(0.6), Align::Left);
            return;
        }
        let lh = inner.h / 2.0;
        for (i, it) in first.iter().enumerate() {
            let r = Rect::new(inner.x, inner.y + lh * i as f32, inner.w, lh);
            let d = t.s * 0.13;
            t.c.stroke_circle(r.x + d / 2.0 + 1.0, r.cy(), d / 2.0, ACCENT, 1.5);
            t.fit_text(&it.text, Rect::new(r.x + d + t.s * 0.08, r.y, r.w - d - t.s * 0.08, r.h), t.fs(0.135), W_MEDIUM, WHITE, Align::Left);
        }
        if n > 2 {
            t.c.text(&format!("+{}", n - 2), Rect::new(inner.x, inner.y, inner.w, lh), t.fs(0.11), W_SEMIBOLD, WHITE.a(0.5), Align::Right);
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        let rows = self.c.items.len().clamp(1, 9) as f32;
        (320.0, 150.0 + rows * 34.0)
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        panel_header(ui, "Tarefas", &format!("{} pendentes", self.pending()));
        let r = ui.row(34.0);
        let res = ui.text_input(id("new"), Rect::new(r.x, r.y, r.w, r.h - 2.0), &mut self.input, "Nova tarefa… (Enter)", false);
        if res.submitted && !self.input.trim().is_empty() {
            self.c.items.insert(0, TaskItem { text: self.input.trim().to_string(), done: false });
            self.input.clear();
            sh.save = true;
        }
        if ui.st.focus.is_none() {
            ui.st.focus = Some(id("new"));
        }
        ui.gap(6.0);
        let list_h = ui.area.b() - ui.y - 40.0 * ui.s;
        let area = Rect::new(ui.area.x, ui.y, ui.area.w, list_h.max(34.0 * ui.s));
        let content_h = self.c.items.len() as f32 * 34.0 * ui.s;
        let off = ui.begin_scroll(id("scroll"), area, content_h);
        let mut remove = None;
        for (i, it) in self.c.items.iter_mut().enumerate() {
            let r = Rect::new(area.x, area.y + i as f32 * 34.0 * ui.s - off, area.w - 6.0 * ui.s, 34.0 * ui.s);
            if r.b() < area.y || r.y > area.b() {
                continue;
            }
            let cb = Rect::new(r.x, r.cy() - 9.0 * ui.s, 18.0 * ui.s, 18.0 * ui.s);
            if ui.checkbox(idn("cb", i as u64), cb, &mut it.done, ACCENT) {
                sh.save = true;
            }
            let col = if it.done { TXT3 } else { TXT };
            ui.c.text(&it.text, Rect::new(r.x + 28.0 * ui.s, r.y, r.w - 60.0 * ui.s, r.h), 12.5 * ui.s, W_NORMAL, col, Align::Left);
            if it.done {
                let (tw, _) = ui.c.g.measure(&it.text, 12.5 * ui.s, W_NORMAL);
                ui.c.line(r.x + 28.0 * ui.s, r.cy(), r.x + 28.0 * ui.s + tw.min(r.w - 60.0 * ui.s), r.cy(), TXT3, 1.0);
            }
            if ui.mouse_in(r) && ui.icon_btn(idn("del", i as u64), Rect::new(r.r() - 26.0 * ui.s, r.cy() - 13.0 * ui.s, 26.0 * ui.s, 26.0 * ui.s), 0xE711, 9.0, TXT2) {
                remove = Some(i);
            }
        }
        ui.end_scroll();
        ui.y = area.b() + 6.0 * ui.s;
        if let Some(i) = remove {
            self.c.items.remove(i);
            sh.save = true;
        }
        let r = ui.row(30.0);
        let done = self.c.items.len() - self.pending();
        if done > 0 && ui.button_k(id("clear"), Rect::new(r.x, r.y, 160.0 * ui.s, r.h), &format!("Limpar concluídas ({})", done), Btn::Ghost) {
            self.c.items.retain(|i| !i.done);
            sh.save = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Beber água
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct WaterCfg {
    goal: i64,
    glass: i64,
    glass_minutes: i64,
    oz: bool,
    count_on_icon: bool,
    style: u32,
    days: BTreeMap<i64, i64>,
    last_drink: i64,
}

impl Default for WaterCfg {
    fn default() -> Self {
        WaterCfg { goal: 2000, glass: 250, glass_minutes: 60, oz: false, count_on_icon: true, style: 0, days: BTreeMap::new(), last_drink: 0 }
    }
}

pub struct Water {
    c: WaterCfg,
}

impl Water {
    pub fn load(v: &Value) -> Water {
        Water { c: jget(v) }
    }
    fn today(&self) -> i64 {
        *self.c.days.get(&local_now().days()).unwrap_or(&0)
    }
    fn fmt_amount(&self, ml: i64) -> String {
        if self.c.oz {
            format!("{:.0} oz", ml as f64 / 29.5735)
        } else if ml >= 1000 {
            format!("{:.1} L", ml as f64 / 1000.0)
        } else {
            format!("{} ml", ml)
        }
    }
    fn drink(&mut self, delta: i64) {
        let d = local_now().days();
        let v = self.c.days.entry(d).or_insert(0);
        *v = (*v + delta).max(0);
        if delta > 0 {
            self.c.last_drink = unix_now();
        }
        let cut = d - 30;
        self.c.days.retain(|k, _| *k >= cut);
    }
    /// Nível do aquário: esvazia conforme o tempo desde o último copo.
    fn aquarium_level(&self) -> f32 {
        if self.c.last_drink == 0 {
            return 0.0;
        }
        let el = (unix_now() - self.c.last_drink) as f32 / (self.c.glass_minutes.max(5) * 60) as f32;
        (1.0 - el).clamp(0.0, 1.0)
    }
}

impl Widget for Water {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn styles(&self) -> &'static [&'static str] {
        &["Aquário", "Copo"]
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        Some(60000)
    }
    fn on_click(&mut self, _fx: f32, _fy: f32, _u: u32, _sh: &mut Shared) -> Click {
        Click::Panel
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        let today = self.today();
        let prog = today as f32 / self.c.goal.max(1) as f32;
        let level = if t.style == 0 { self.aquarium_level() } else { prog.min(1.0) };
        t.card_color(Color::hex(0x103A5C), Color::hex(0x0A2238));
        let r = t.r;
        let rad = t.rad();
        // água
        let wh = r.h * level;
        if wh > 1.0 {
            t.c.push_rrect_clip(r, rad);
            t.c.fill_rect(Rect::new(r.x, r.b() - wh, r.w, wh), Color::hex(0x2F9BFF).a(0.75));
            // onda
            let wy = r.b() - wh;
            let mut pts = vec![];
            let steps = 16;
            for i in 0..=steps {
                let x = r.x + r.w * i as f32 / steps as f32;
                let y = wy + (i as f32 * 0.8 + (local_now().minute as f32)).sin() * t.s * 0.025;
                pts.push((x, y));
            }
            pts.push((r.r(), wy + t.s * 0.05));
            pts.push((r.x, wy + t.s * 0.05));
            t.c.fill_poly(&pts, Color::hex(0x6CC4FF).a(0.85));
            if t.style == 0 {
                // peixinho
                let fx = r.x + r.w * (0.3 + 0.4 * ((local_now().minute % 10) as f32 / 10.0));
                let fy = (r.b() - wh * 0.45).min(r.b() - t.s * 0.15);
                t.c.fill_ellipse(fx, fy, t.s * 0.08, t.s * 0.05, ORANGE);
                t.c.fill_poly(&[(fx - t.s * 0.07, fy), (fx - t.s * 0.13, fy - t.s * 0.05), (fx - t.s * 0.13, fy + t.s * 0.05)], ORANGE);
            }
            t.c.pop_clip();
        }
        let inner = t.inner();
        if self.c.count_on_icon {
            let glasses = today / self.c.glass.max(1);
            let goal_g = (self.c.goal + self.c.glass - 1) / self.c.glass.max(1);
            let txt = if t.wide() { format!("{} de {} copos", glasses, goal_g) } else { format!("{}/{}", glasses, goal_g) };
            t.fit_display(&txt, Rect::new(inner.x, inner.y, inner.w, inner.h * 0.5), t.fs(0.2), W_SEMIBOLD, WHITE, Align::Center);
        }
        if t.wide() {
            t.fit_text(&self.fmt_amount(today), Rect::new(inner.x, inner.y + inner.h * 0.5, inner.w, inner.h * 0.4), t.fs(0.14), W_MEDIUM, WHITE.a(0.8), Align::Center);
        } else {
            t.c.glyph(0xEB42, Rect::new(inner.x, inner.y + inner.h * 0.45, inner.w, inner.h * 0.5), t.fs(0.22), WHITE.a(0.85));
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (300.0, 320.0)
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let today = self.today();
        panel_header(ui, "Beber água", &format!("{} de {}", self.fmt_amount(today), self.fmt_amount(self.c.goal)));
        let r = ui.row(16.0);
        bar(ui.c, Rect::new(r.x, r.y + 3.0, r.w, 10.0 * ui.s), today as f32 / self.c.goal.max(1) as f32, Color(1.0, 1.0, 1.0, 0.12), Color::hex(0x2F9BFF));
        ui.gap(10.0);
        let r = ui.row(40.0);
        let w = (r.w - 8.0 * ui.s) / 2.0;
        if ui.button_k(id("drink"), Rect::new(r.x, r.y, w, r.h), &format!("Beber {}", self.fmt_amount(self.c.glass)), Btn::Primary) {
            self.drink(self.c.glass);
            sh.save = true;
        }
        if ui.button(id("undo"), Rect::new(r.x + w + 8.0 * ui.s, r.y, w, r.h), "Desfazer um copo") {
            self.drink(-self.c.glass);
            sh.save = true;
        }
        ui.gap(12.0);
        let r = ui.row(18.0);
        ui.c.text("ESTA SEMANA", r, 11.0 * ui.s, W_SEMIBOLD, TXT2, Align::Left);
        let chart = ui.row(130.0);
        let today_d = local_now().days();
        let bw = chart.w / 7.0;
        for i in 0..7i64 {
            let d = today_d - 6 + i;
            let v = *self.c.days.get(&d).unwrap_or(&0);
            let f = (v as f32 / self.c.goal.max(1) as f32).min(1.2);
            let maxh = chart.h - 22.0 * ui.s;
            let h = (maxh * f / 1.2).max(3.0);
            let br = Rect::new(chart.x + bw * i as f32 + bw * 0.22, chart.y + maxh - h, bw * 0.56, h);
            ui.c.fill_rrect(br, 4.0 * ui.s, if f >= 1.0 { GREEN } else { Color::hex(0x2F9BFF).a(if d == today_d { 1.0 } else { 0.6 }) });
            let dow = weekday_from_days(d);
            ui.c.text(&DIAS_CURTO[dow as usize][..1], Rect::new(chart.x + bw * i as f32, chart.y + maxh + 2.0, bw, 20.0 * ui.s), 11.0 * ui.s, W_NORMAL, if d == today_d { TXT } else { TXT2 }, Align::Center);
        }
        let goal_y = chart.y + (chart.h - 22.0 * ui.s) * (1.0 - 1.0 / 1.2);
        ui.c.line(chart.x, goal_y, chart.r(), goal_y, Color(1.0, 1.0, 1.0, 0.25), 1.0);
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let mut ch = false;
        ch |= ui.stepper(id("goal"), "Meta do dia", &mut self.c.goal, 500, 6000, 100, " ml");
        ch |= ui.stepper(id("glass"), "Tamanho do copo", &mut self.c.glass, 50, 1000, 50, " ml");
        ch |= ui.stepper(id("dur"), "Quanto dura um copo (Aquário)", &mut self.c.glass_minutes, 10, 240, 5, " min");
        ch |= ui.row_toggle(id("oz"), "Mostrar em onças", &mut self.c.oz);
        ch |= ui.row_toggle(id("cnt"), "Copos no ícone", &mut self.c.count_on_icon);
        if ch {
            sh.save = true;
        }
    }
}
