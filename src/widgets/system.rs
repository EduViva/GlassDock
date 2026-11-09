// Bateria, Atividade (CPU/memória), Rede e Layout do teclado.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use windows::Win32::Foundation::*;
use windows::Win32::Globalization::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::*;
use crate::ui::*;

// ---------------------------------------------------------------------------
// Bateria
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct BatCfg {
    pct_on_icon: bool,
}

impl Default for BatCfg {
    fn default() -> Self {
        BatCfg { pct_on_icon: true }
    }
}

pub struct BatteryW {
    c: BatCfg,
}

impl BatteryW {
    pub fn load(v: &Value) -> BatteryW {
        BatteryW { c: jget(v) }
    }
}

fn bat_color(b: &crate::status::Battery) -> Color {
    if b.charging || b.ac {
        GREEN
    } else if b.pct <= 10 {
        RED
    } else if b.pct <= 20 {
        ORANGE
    } else {
        WHITE
    }
}

pub fn draw_battery_shape(c: &Canvas, r: Rect, pct: u32, col: Color, charging: bool) {
    let body = Rect::new(r.x, r.y, r.w * 0.9, r.h);
    let rad = r.h * 0.28;
    c.stroke_rrect(body.inset(r.h * 0.05), rad, WHITE.a(0.55), (r.h * 0.09).max(1.2));
    let cap = Rect::new(body.r() + r.w * 0.02, r.cy() - r.h * 0.2, r.w * 0.06, r.h * 0.4);
    c.fill_rrect(cap, cap.w / 2.0, WHITE.a(0.55));
    let inner = body.inset(r.h * 0.18);
    let w = inner.w * (pct as f32 / 100.0).clamp(0.04, 1.0);
    c.fill_rrect(Rect::new(inner.x, inner.y, w, inner.h), rad * 0.6, col);
    if charging {
        c.glyph(0xE945, body, r.h * 0.7, Color(0.0, 0.0, 0.0, 0.75));
    }
}

impl Widget for BatteryW {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn styles(&self) -> &'static [&'static str] {
        &["Estado", "Anel", "Só o ícone"]
    }
    fn max_units(&self) -> u32 {
        2
    }
    fn draw(&mut self, t: &Tile, sh: &mut Shared) {
        let b = sh.status.battery;
        t.card();
        let inner = t.inner();
        if !b.present {
            t.c.glyph(0xE83E, inner, t.fs(0.34), WHITE.a(0.8));
            return;
        }
        let col = bat_color(&b);
        match t.style {
            1 => {
                let d = inner.h.min(inner.w);
                let w = (t.s * 0.08).max(2.0);
                t.c.ring(inner.cx(), inner.cy(), d / 2.0 - w, b.pct as f32 / 100.0, Color(1.0, 1.0, 1.0, 0.14), col, w);
                t.fit_display(&format!("{}", b.pct), inner.centered(d * 0.6, d * 0.4), t.fs(0.24), W_SEMIBOLD, WHITE, Align::Center);
                if b.charging {
                    t.c.glyph(0xE945, Rect::new(inner.x, inner.y + d * 0.62, inner.w, d * 0.25), t.fs(0.13), GREEN);
                }
            }
            2 => {
                let br = inner.centered(inner.w * 0.78, inner.w * 0.36);
                draw_battery_shape(t.c, br, b.pct, col, b.charging);
            }
            _ => {
                let br = Rect::new(inner.cx() - inner.w * 0.36, inner.y + inner.h * 0.14, inner.w * 0.72, inner.w * 0.3);
                draw_battery_shape(t.c, br, b.pct, col, b.charging);
                if self.c.pct_on_icon {
                    t.fit_display(&format!("{}%", b.pct), Rect::new(inner.x, inner.y + inner.h * 0.5, inner.w, inner.h * 0.48), t.fs(0.22), W_SEMIBOLD, WHITE, Align::Center);
                }
                if t.wide() {
                    // tempo restante ao lado
                }
            }
        }
    }
    fn panel_size(&self, sh: &Shared) -> (f32, f32) {
        (290.0, if sh.status.battery.present { 230.0 } else { 70.0 })
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let b = sh.status.battery;
        if !b.present {
            panel_header(ui, "Bateria", "Este PC não tem bateria.");
            return;
        }
        let state = if b.charging {
            "Carregando"
        } else if b.ac {
            "Na tomada"
        } else {
            "Na bateria"
        };
        panel_header(ui, "Bateria", state);
        let r = ui.row(60.0);
        ui.c.display(&format!("{}%", b.pct), Rect::new(r.x, r.y, r.w * 0.5, r.h), 40.0 * ui.s, W_SEMIBOLD, TXT, Align::Left);
        draw_battery_shape(ui.c, Rect::new(r.r() - 90.0 * ui.s, r.cy() - 18.0 * ui.s, 90.0 * ui.s, 36.0 * ui.s), b.pct, bat_color(&b), b.charging);
        ui.gap(6.0);
        let r = ui.row(22.0);
        let left = match b.secs_left {
            Some(s) if !b.ac => format!("Ainda dá {}", fmt_hm(s as i64)),
            _ if b.charging => "Carregando".into(),
            _ => "Calculando o tempo restante…".into(),
        };
        ui.c.text(&left, r, 12.5 * ui.s, W_NORMAL, TXT2, Align::Left);
        if b.saver {
            let r = ui.row(22.0);
            ui.c.text("Economia de bateria ligada", r, 12.5 * ui.s, W_NORMAL, ORANGE, Align::Left);
        }
        ui.gap(8.0);
        let r = ui.row(32.0);
        if ui.button(id("opt"), r, "Opções de energia e bateria") {
            shell_open("ms-settings:batterysaver");
            sh.close_panel = true;
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        if ui.row_toggle(id("p"), "Carga no ícone", &mut self.c.pct_on_icon) {
            sh.save = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Atividade
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct ActCfg {
    cpu: bool,
    mem: bool,
}

impl Default for ActCfg {
    fn default() -> Self {
        ActCfg { cpu: true, mem: true }
    }
}

pub struct Activity {
    c: ActCfg,
}

impl Activity {
    pub fn load(v: &Value) -> Activity {
        Activity { c: jget(v) }
    }
}

fn spark(c: &Canvas, r: Rect, data: &[f32], col: Color) {
    if data.len() < 2 {
        return;
    }
    let n = data.len();
    let pts: Vec<(f32, f32)> = data.iter().enumerate().map(|(i, v)| (r.x + r.w * i as f32 / (n - 1) as f32, r.b() - r.h * v.clamp(0.0, 1.0))).collect();
    let mut poly = pts.clone();
    poly.push((r.r(), r.b()));
    poly.push((r.x, r.b()));
    c.fill_poly(&poly, col.a(0.18));
    c.polyline(&pts, col, 1.5);
}

impl Widget for Activity {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn styles(&self) -> &'static [&'static str] {
        &["Barras", "Números", "Anéis"]
    }
    fn needs_sys(&self) -> bool {
        true
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        Some(2000)
    }
    fn draw(&mut self, t: &Tile, sh: &mut Shared) {
        t.card();
        let inner = t.inner();
        let mut items = vec![];
        if self.c.cpu {
            items.push(("CPU", sh.sys.cpu, TEAL));
        }
        if self.c.mem {
            items.push(("MEM", sh.sys.mem, PURPLE));
        }
        if items.is_empty() {
            return;
        }
        let n = items.len() as f32;
        match t.style {
            1 => {
                let w = inner.w / n;
                for (i, (l, v, col)) in items.iter().enumerate() {
                    let r = Rect::new(inner.x + w * i as f32, inner.y, w, inner.h);
                    t.fit_display(&format!("{:.0}%", v * 100.0), Rect::new(r.x, r.y, r.w, r.h * 0.62), t.fs(0.26), W_SEMIBOLD, usage_color(*v).mix(WHITE, 0.25), Align::Center);
                    t.c.text(l, Rect::new(r.x, r.y + r.h * 0.58, r.w, r.h * 0.38), t.fs(0.12), W_SEMIBOLD, col.mix(WHITE, 0.3), Align::Center);
                }
            }
            2 => {
                let d = inner.h.min(inner.w / n);
                let w = (t.s * 0.07).max(2.0);
                for (i, (_, v, col)) in items.iter().enumerate() {
                    let rr = d / 2.0 - w - i as f32 * w * 1.8;
                    t.c.ring(inner.cx(), inner.cy(), rr, *v, Color(1.0, 1.0, 1.0, 0.12), *col, w);
                }
                if let Some((_, v, _)) = items.first() {
                    t.fit_display(&format!("{:.0}", v * 100.0), inner.centered(d * 0.5, d * 0.3), t.fs(0.17), W_SEMIBOLD, WHITE, Align::Center);
                }
            }
            _ => {
                let lh = inner.h / n;
                for (i, (l, v, col)) in items.iter().enumerate() {
                    let r = Rect::new(inner.x, inner.y + lh * i as f32, inner.w, lh);
                    t.c.text(l, Rect::new(r.x, r.y, r.w, r.h * 0.5), t.fs(0.11), W_SEMIBOLD, WHITE.a(0.7), Align::Left);
                    t.c.text(&format!("{:.0}%", v * 100.0), Rect::new(r.x, r.y, r.w, r.h * 0.5), t.fs(0.11), W_SEMIBOLD, WHITE, Align::Right);
                    bar(t.c, Rect::new(r.x, r.y + r.h * 0.56, r.w, r.h * 0.22), *v, Color(1.0, 1.0, 1.0, 0.12), *col);
                }
            }
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (320.0, 262.0)
    }
    fn panel_interval(&self, _sh: &Shared) -> Option<u32> {
        Some(2000)
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        panel_header(ui, "Atividade", "");
        let cpu_hist: Vec<f32> = sh.sys.cpu_hist.iter().copied().collect();
        let mem_hist: Vec<f32> = sh.sys.mem_hist.iter().copied().collect();
        let r = ui.row(22.0);
        ui.c.text("Processador", r, 12.5 * ui.s, W_MEDIUM, TXT, Align::Left);
        ui.c.text(&format!("{:.0}%", sh.sys.cpu * 100.0), r, 12.5 * ui.s, W_SEMIBOLD, TEAL, Align::Right);
        let r = ui.row(56.0);
        ui.c.fill_rrect(r, 6.0 * ui.s, Color(1.0, 1.0, 1.0, 0.05));
        spark(ui.c, r.inset(4.0 * ui.s), &cpu_hist, TEAL);
        ui.gap(8.0);
        let r = ui.row(22.0);
        ui.c.text("Memória", r, 12.5 * ui.s, W_MEDIUM, TXT, Align::Left);
        let gb = |b: u64| b as f64 / (1024.0 * 1024.0 * 1024.0);
        ui.c.text(&format!("{:.1} de {:.1} GB", gb(sh.sys.mem_used), gb(sh.sys.mem_total)), r, 12.5 * ui.s, W_SEMIBOLD, PURPLE.mix(WHITE, 0.3), Align::Right);
        let r = ui.row(56.0);
        ui.c.fill_rrect(r, 6.0 * ui.s, Color(1.0, 1.0, 1.0, 0.05));
        spark(ui.c, r.inset(4.0 * ui.s), &mem_hist, PURPLE);
        ui.gap(10.0);
        let r = ui.row(32.0);
        if ui.button(id("tm"), r, "Abrir o Gerenciador de Tarefas") {
            shell_open("taskmgr.exe");
            sh.close_panel = true;
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let mut ch = ui.row_toggle(id("c"), "Processador", &mut self.c.cpu);
        ch |= ui.row_toggle(id("m"), "Memória", &mut self.c.mem);
        if ch {
            sh.save = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Rede
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct NetCfg {
    iface: u32,
}

pub struct Network {
    c: NetCfg,
}

impl Network {
    pub fn load(v: &Value) -> Network {
        Network { c: jget(v) }
    }
}

impl Widget for Network {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn styles(&self) -> &'static [&'static str] {
        &["Só os números", "Com gráfico"]
    }
    fn needs_sys(&self) -> bool {
        true
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        Some(2000)
    }
    fn draw(&mut self, t: &Tile, sh: &mut Shared) {
        sh.sys.iface = self.c.iface;
        t.card();
        let inner = t.inner();
        let rx = fmt_bytes_rate(sh.sys.rx_bps);
        let tx = fmt_bytes_rate(sh.sys.tx_bps);
        let txt_w = if t.style == 1 && t.units >= 2 { inner.w * 0.5 } else { inner.w };
        let half = inner.h / 2.0;
        let gl = t.fs(0.14);
        let row = |i: f32, g: u32, s: &str, col: Color| {
            let r = Rect::new(inner.x, inner.y + half * i, txt_w, half);
            t.c.glyph(g, Rect::new(r.x, r.y, gl * 1.2, r.h), gl, col);
            t.fit_text(s, Rect::new(r.x + gl * 1.4, r.y, r.w - gl * 1.4, r.h), t.fs(0.14), W_SEMIBOLD, WHITE, Align::Left);
        };
        row(0.0, 0xE74B, &rx, TEAL);
        row(1.0, 0xE74A, &tx, ORANGE);
        if t.style == 1 && t.units >= 2 {
            let gr = Rect::new(inner.x + txt_w + t.s * 0.06, inner.y, inner.w - txt_w - t.s * 0.06, inner.h);
            let max = sh.sys.net_hist.iter().map(|(a, b)| a.max(*b)).fold(1.0f64, f64::max);
            let rxs: Vec<f32> = sh.sys.net_hist.iter().map(|(a, _)| (*a / max) as f32).collect();
            let txs: Vec<f32> = sh.sys.net_hist.iter().map(|(_, b)| (*b / max) as f32).collect();
            spark(t.c, gr, &rxs, TEAL);
            spark(t.c, gr, &txs, ORANGE);
        }
    }
    fn panel_size(&self, sh: &Shared) -> (f32, f32) {
        (320.0, 250.0 + sh.sys.ifaces.len().min(5) as f32 * 32.0)
    }
    fn panel_interval(&self, _sh: &Shared) -> Option<u32> {
        Some(2000)
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let st = &sh.status;
        let sub = match st.net {
            crate::status::Net::Wifi(q) => format!("Wi-Fi {} · sinal {}%", st.ssid, q),
            crate::status::Net::Ethernet => "Cabo".into(),
            crate::status::Net::None => "Sem conexão".into(),
        };
        panel_header(ui, "Rede", &sub);
        let r = ui.row(40.0);
        let w = r.w / 2.0;
        ui.c.glyph(0xE74B, Rect::new(r.x, r.y, 20.0 * ui.s, r.h), 14.0 * ui.s, TEAL);
        ui.c.display(&fmt_bytes_rate(sh.sys.rx_bps), Rect::new(r.x + 22.0 * ui.s, r.y, w - 22.0 * ui.s, r.h), 18.0 * ui.s, W_SEMIBOLD, TXT, Align::Left);
        ui.c.glyph(0xE74A, Rect::new(r.x + w, r.y, 20.0 * ui.s, r.h), 14.0 * ui.s, ORANGE);
        ui.c.display(&fmt_bytes_rate(sh.sys.tx_bps), Rect::new(r.x + w + 22.0 * ui.s, r.y, w - 22.0 * ui.s, r.h), 18.0 * ui.s, W_SEMIBOLD, TXT, Align::Left);
        let r = ui.row(80.0);
        ui.c.fill_rrect(r, 6.0 * ui.s, Color(1.0, 1.0, 1.0, 0.05));
        let max = sh.sys.net_hist.iter().map(|(a, b)| a.max(*b)).fold(1.0f64, f64::max);
        let rxs: Vec<f32> = sh.sys.net_hist.iter().map(|(a, _)| (*a / max) as f32).collect();
        let txs: Vec<f32> = sh.sys.net_hist.iter().map(|(_, b)| (*b / max) as f32).collect();
        spark(ui.c, r.inset(4.0 * ui.s), &rxs, TEAL);
        spark(ui.c, r.inset(4.0 * ui.s), &txs, ORANGE);
        ui.gap(8.0);
        let r = ui.row(18.0);
        ui.c.text("INTERFACE", r, 11.0 * ui.s, W_SEMIBOLD, TXT2, Align::Left);
        let mut opts: Vec<(u32, String)> = vec![(0, "Todas".into())];
        opts.extend(sh.sys.ifaces.iter().take(5).map(|i| (i.index, i.name.clone())));
        for (idx, name) in opts {
            let r = ui.row(30.0);
            let sel = self.c.iface == idx;
            let (hov, clk) = ui.hit(idn("if", idx as u64), r);
            if hov {
                ui.c.fill_rrect(r, 6.0 * ui.s, FILL);
            }
            if sel {
                ui.c.glyph(0xE73E, Rect::new(r.x, r.y, 24.0 * ui.s, r.h), 11.0 * ui.s, ACCENT);
            }
            ui.c.text(&name, Rect::new(r.x + 26.0 * ui.s, r.y, r.w - 26.0 * ui.s, r.h), 12.5 * ui.s, W_NORMAL, TXT, Align::Left);
            if clk {
                self.c.iface = idx;
                sh.sys.iface = idx;
                sh.save = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Layout do teclado
// ---------------------------------------------------------------------------

pub struct Keyboard {
    cur: isize,
}

fn fg_layout() -> HKL {
    unsafe {
        let fg = GetForegroundWindow();
        let tid = GetWindowThreadProcessId(fg, None);
        GetKeyboardLayout(tid)
    }
}

pub fn layout_names(hkl: HKL) -> (String, String) {
    unsafe {
        let lang = (hkl.0 as usize & 0xFFFF) as u32;
        let mut name = [0u16; 85];
        let n = LCIDToLocaleName(lang, Some(&mut name), 0);
        if n == 0 {
            return ("??".into(), String::new());
        }
        let mut abbr = [0u16; 16];
        let _ = GetLocaleInfoEx(windows::core::PCWSTR(name.as_ptr()), LOCALE_SABBREVLANGNAME, Some(&mut abbr));
        let mut full = [0u16; 128];
        let _ = GetLocaleInfoEx(windows::core::PCWSTR(name.as_ptr()), LOCALE_SLOCALIZEDDISPLAYNAME, Some(&mut full));
        (from_wide(&abbr), from_wide(&full))
    }
}

pub fn all_layouts() -> Vec<HKL> {
    unsafe {
        let n = GetKeyboardLayoutList(None);
        let mut v = vec![HKL::default(); n.max(0) as usize];
        let n = GetKeyboardLayoutList(Some(&mut v));
        v.truncate(n.max(0) as usize);
        v
    }
}

fn switch_to(hkl: HKL) {
    unsafe {
        let fg = GetForegroundWindow();
        let _ = PostMessageW(Some(fg), WM_INPUTLANGCHANGEREQUEST, WPARAM(0), LPARAM(hkl.0 as isize));
    }
}

impl Keyboard {
    pub fn load(_v: &Value) -> Keyboard {
        Keyboard { cur: 0 }
    }
}

impl Widget for Keyboard {
    fn save(&self) -> Value {
        Value::Null
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        Some(1000)
    }
    fn tick(&mut self, _sh: &mut Shared) -> bool {
        let h = fg_layout().0 as isize;
        if h != self.cur {
            self.cur = h;
            return true;
        }
        false
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        t.card();
        let hkl = HKL(self.cur as _);
        let (abbr, full) = layout_names(hkl);
        let inner = t.inner();
        if t.wide() {
            t.c.glyph(0xE765, Rect::new(inner.x, inner.y, inner.h, inner.h), t.fs(0.3), WHITE.a(0.85));
            let r = Rect::new(inner.x + inner.h + t.s * 0.08, inner.y, inner.w - inner.h - t.s * 0.08, inner.h);
            t.fit_display(&abbr, Rect::new(r.x, r.y, r.w, r.h * 0.55), t.fs(0.22), W_SEMIBOLD, WHITE, Align::Left);
            t.fit_text(&full, Rect::new(r.x, r.y + r.h * 0.5, r.w, r.h * 0.42), t.fs(0.12), W_NORMAL, WHITE.a(0.6), Align::Left);
        } else {
            t.c.glyph(0xE765, Rect::new(inner.x, inner.y, inner.w, inner.h * 0.5), t.fs(0.22), WHITE.a(0.6));
            t.fit_display(&abbr, Rect::new(inner.x, inner.y + inner.h * 0.45, inner.w, inner.h * 0.5), t.fs(0.24), W_SEMIBOLD, WHITE, Align::Center);
        }
    }
    fn on_click(&mut self, _fx: f32, _fy: f32, _u: u32, sh: &mut Shared) -> Click {
        let list = all_layouts();
        if list.len() > 1 {
            let cur = fg_layout();
            let i = list.iter().position(|h| *h == cur).unwrap_or(0);
            let next = list[(i + 1) % list.len()];
            switch_to(next);
            self.cur = next.0 as isize;
            sh.redraw = true;
        }
        Click::Handled
    }
    fn menu(&self) -> Vec<(u32, String)> {
        let mut v: Vec<(u32, String)> = all_layouts().iter().enumerate().map(|(i, h)| (100 + i as u32, layout_names(*h).1)).collect();
        v.push((1, "Configurações de idioma…".into()));
        v
    }
    fn on_menu(&mut self, idm: u32, _sh: &mut Shared) {
        if idm == 1 {
            shell_open("ms-settings:keyboard");
        } else if idm >= 100 {
            if let Some(h) = all_layouts().get((idm - 100) as usize) {
                switch_to(*h);
            }
        }
    }
}
