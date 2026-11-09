// Coleção de apps e Favoritos.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::*;
use crate::config::PinnedApp;
use crate::ui::*;

// ---------------------------------------------------------------------------
// Coleção de apps
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct ColCfg {
    apps: Vec<PinnedApp>,
    layout: u32,
}

pub struct Collection {
    c: ColCfg,
    search: String,
}

impl Collection {
    pub fn load(v: &Value) -> Collection {
        Collection { c: jget(v), search: String::new() }
    }
}

impl Widget for Collection {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn draw(&mut self, t: &Tile, sh: &mut Shared) {
        t.card();
        let inner = t.r.inset(t.s * 0.12);
        if self.c.apps.is_empty() {
            t.c.glyph(0xE71D, inner, t.fs(0.34), WHITE.a(0.7));
            return;
        }
        let cols = if t.wide() { (t.units * 2).min(8) as usize } else { 2 };
        let n = (cols * 2).min(self.c.apps.len());
        let cell = (inner.w / cols as f32).min(inner.h / 2.0);
        let gw = cell * cols as f32;
        let x0 = inner.cx() - gw / 2.0;
        let y0 = inner.cy() - cell;
        for (i, a) in self.c.apps.iter().take(n).enumerate() {
            let r = Rect::new(x0 + cell * (i % cols) as f32, y0 + cell * (i / cols) as f32, cell, cell).inset(cell * 0.08);
            if let Some(ic) = sh.icons.get(&crate::icons::icon_path(a), (cell * 1.5) as u32) {
                t.c.bitmap(&ic.bmp, r, 1.0);
            }
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        let n = self.c.apps.len().max(1);
        if self.c.apps.is_empty() {
            return (280.0, 80.0);
        }
        match self.c.layout {
            1 => ((n as f32 * 84.0 + 32.0).min(760.0), 150.0),
            2 => (280.0, 70.0 + n.min(10) as f32 * 44.0),
            _ => {
                let cols = (n as f32).sqrt().ceil().clamp(3.0, 5.0);
                (cols * 84.0 + 32.0, 70.0 + (n as f32 / cols).ceil() * 96.0)
            }
        }
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let r = ui.row(26.0);
        ui.c.text(if self.c.apps.is_empty() { "Coleção vazia" } else { "Coleção" }, r, 15.0 * ui.s, W_SEMIBOLD, TXT, Align::Left);
        ui.gap(6.0);
        if self.c.apps.is_empty() {
            ui.note("Adicione apps em Editar widget (botão direito).");
            return;
        }
        let apps = self.c.apps.clone();
        match self.c.layout {
            2 => {
                for (i, a) in apps.iter().enumerate() {
                    let r = ui.row(44.0);
                    let (hov, clk) = ui.hit(idn("a", i as u64), r);
                    if hov {
                        ui.c.fill_rrect(r, 8.0 * ui.s, FILL);
                    }
                    if let Some(ic) = sh.icons.get(&crate::icons::icon_path(a), 64) {
                        ui.c.bitmap(&ic.bmp, Rect::new(r.x + 6.0 * ui.s, r.y + 6.0 * ui.s, 32.0 * ui.s, 32.0 * ui.s), 1.0);
                    }
                    ui.c.text(&a.name, Rect::new(r.x + 48.0 * ui.s, r.y, r.w - 52.0 * ui.s, r.h), 12.5 * ui.s, W_NORMAL, TXT, Align::Left);
                    if clk {
                        crate::winlist::launch(a, false);
                        sh.close_panel = true;
                    }
                }
            }
            _ => {
                let cols = if self.c.layout == 1 { apps.len().max(1) } else { ((apps.len() as f32).sqrt().ceil() as usize).clamp(3, 5) };
                let cw = 84.0 * ui.s;
                let chh = 96.0 * ui.s;
                let x0 = ui.area.x + (ui.area.w - cw * cols as f32) / 2.0;
                let y0 = ui.y;
                for (i, a) in apps.iter().enumerate() {
                    let cell = Rect::new(x0 + cw * (i % cols) as f32, y0 + chh * (i / cols) as f32, cw, chh);
                    let (hov, clk) = ui.hit(idn("a", i as u64), cell);
                    if hov {
                        ui.c.fill_rrect(cell.inset(3.0), 10.0 * ui.s, FILL);
                    }
                    let ir = Rect::new(cell.cx() - 26.0 * ui.s, cell.y + 10.0 * ui.s, 52.0 * ui.s, 52.0 * ui.s);
                    if let Some(ic) = sh.icons.get(&crate::icons::icon_path(a), 96) {
                        ui.c.bitmap(&ic.bmp, ir, 1.0);
                    }
                    ui.c.text(&a.name, Rect::new(cell.x + 4.0, ir.b() + 4.0 * ui.s, cell.w - 8.0, 20.0 * ui.s), 11.0 * ui.s, W_NORMAL, TXT, Align::Center);
                    if clk {
                        crate::winlist::launch(a, false);
                        sh.close_panel = true;
                    }
                }
            }
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        if ui.labeled_segmented(id("lay"), "Layout", &["Grade", "Horizontal", "Vertical"], &mut self.c.layout) {
            sh.save = true;
        }
        ui.caption("APPS NA COLEÇÃO");
        let mut action: Option<(usize, i32)> = None;
        for (i, a) in self.c.apps.iter().enumerate() {
            let r = ui.row(34.0);
            if let Some(ic) = sh.icons.get(&crate::icons::icon_path(a), 48) {
                ui.c.bitmap(&ic.bmp, Rect::new(r.x, r.y + 5.0 * ui.s, 24.0 * ui.s, 24.0 * ui.s), 1.0);
            }
            ui.c.text(&a.name, Rect::new(r.x + 32.0 * ui.s, r.y, r.w - 130.0 * ui.s, r.h), 12.5 * ui.s, W_NORMAL, TXT, Align::Left);
            let b = 28.0 * ui.s;
            if ui.icon_btn(idn("up", i as u64), Rect::new(r.r() - 3.0 * b, r.y + 3.0, b, b), 0xE70E, 10.0, TXT2) {
                action = Some((i, -1));
            }
            if ui.icon_btn(idn("dn", i as u64), Rect::new(r.r() - 2.0 * b, r.y + 3.0, b, b), 0xE70D, 10.0, TXT2) {
                action = Some((i, 1));
            }
            if ui.icon_btn(idn("rm", i as u64), Rect::new(r.r() - b, r.y + 3.0, b, b), 0xE711, 10.0, TXT2) {
                action = Some((i, 0));
            }
        }
        if let Some((i, d)) = action {
            if d == 0 {
                self.c.apps.remove(i);
            } else {
                let j = (i as i32 + d).clamp(0, self.c.apps.len() as i32 - 1) as usize;
                self.c.apps.swap(i, j);
            }
            sh.save = true;
        }
        ui.gap(6.0);
        ui.caption("ADICIONAR APPS");
        let r = ui.row(34.0);
        let bw = 110.0 * ui.s;
        ui.text_input(id("s"), Rect::new(r.x, r.y + 2.0, r.w - bw - 8.0, r.h - 4.0), &mut self.search, "Buscar app instalado…", false);
        if ui.button(id("file"), Rect::new(r.r() - bw, r.y + 2.0, bw, r.h - 4.0), "Arquivo…") {
            sh.pick = Some((sh.current_widget, 1, Pick::Exe));
        }
        let q = self.search.to_lowercase();
        if q.len() >= 2 {
            let found: Vec<crate::icons::CatalogApp> = sh.catalog().iter().filter(|a| a.name.to_lowercase().contains(&q)).take(8).cloned().collect();
            for (i, a) in found.iter().enumerate() {
                let r = ui.row(32.0);
                let (hov, clk) = ui.hit(idn("cat", i as u64), r);
                if hov {
                    ui.c.fill_rrect(r, 6.0 * ui.s, FILL);
                }
                let p = a.to_pinned();
                if let Some(ic) = sh.icons.get(&crate::icons::icon_path(&p), 48) {
                    ui.c.bitmap(&ic.bmp, Rect::new(r.x + 4.0, r.y + 5.0 * ui.s, 22.0 * ui.s, 22.0 * ui.s), 1.0);
                }
                ui.c.text(&a.name, Rect::new(r.x + 34.0 * ui.s, r.y, r.w - 60.0 * ui.s, r.h), 12.5 * ui.s, W_NORMAL, TXT, Align::Left);
                ui.c.glyph(0xE710, Rect::new(r.r() - 26.0 * ui.s, r.y, 22.0 * ui.s, r.h), 10.0 * ui.s, ACCENT);
                if clk {
                    self.c.apps.push(p);
                    sh.save = true;
                }
            }
        }
    }
    fn on_picked(&mut self, _p: u32, paths: Vec<String>, sh: &mut Shared) {
        for p in paths {
            self.c.apps.push(crate::icons::pinned_from_path(&p));
        }
        sh.save = true;
    }
    fn on_drop(&mut self, files: &[String], sh: &mut Shared) -> bool {
        for p in files {
            self.c.apps.push(crate::icons::pinned_from_path(p));
        }
        sh.save = true;
        true
    }
}

// ---------------------------------------------------------------------------
// Favoritos
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct Link {
    name: String,
    url: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct FavCfg {
    links: Vec<Link>,
    browser: String,
}

impl Default for FavCfg {
    fn default() -> Self {
        FavCfg {
            links: vec![
                Link { name: "YouTube".into(), url: "https://youtube.com".into() },
                Link { name: "Gmail".into(), url: "https://mail.google.com".into() },
            ],
            browser: String::new(),
        }
    }
}

pub struct Favorites {
    c: FavCfg,
    new_name: String,
    new_url: String,
    browsers: Option<Vec<(String, String)>>,
}

fn domain(url: &str) -> String {
    let u = url.trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.");
    u.split('/').next().unwrap_or(u).to_string()
}

fn link_color(s: &str) -> Color {
    let h = crate::ui::id(&domain(s));
    let cols = [0xFF453A, 0xFF9F0A, 0x30D158, 0x0A84FF, 0xBF5AF2, 0xFF375F, 0x40C8E0, 0x5E5CE6];
    Color::hex(cols[(h % cols.len() as u64) as usize])
}

fn badge(c: &Canvas, r: Rect, l: &Link) {
    c.fill_rrect(r, r.h * 0.26, link_color(&l.url));
    let ch: String = l.name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
    c.text(&ch, r, r.h * 0.55, W_BOLD, WHITE, Align::Center);
}

/// Navegadores instalados (nome, comando).
pub fn browsers() -> Vec<(String, String)> {
    use windows::Win32::System::Registry::*;
    let mut out = vec![];
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        unsafe {
            let base = wide("SOFTWARE\\Clients\\StartMenuInternet");
            let mut hk = HKEY::default();
            if RegOpenKeyExW(root, pcw(&base), None, KEY_READ, &mut hk).is_err() {
                continue;
            }
            let mut i = 0;
            loop {
                let mut name = [0u16; 256];
                let mut len = 256u32;
                if RegEnumKeyExW(hk, i, Some(windows::core::PWSTR(name.as_mut_ptr())), &mut len, None, None, None, None).is_err() {
                    break;
                }
                i += 1;
                let sub = from_wide(&name);
                let cmd_key = wide(&format!("{}\\shell\\open\\command", sub));
                let mut buf = [0u16; 1024];
                let mut sz = (buf.len() * 2) as u32;
                if RegGetValueW(hk, pcw(&cmd_key), None, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr() as _), Some(&mut sz)).is_ok() {
                    let cmd = from_wide(&buf).trim_matches('"').to_string();
                    let mut disp = [0u16; 256];
                    let mut dsz = (disp.len() * 2) as u32;
                    let sw = wide(&sub);
                    let label = if RegGetValueW(hk, pcw(&sw), None, RRF_RT_REG_SZ, None, Some(disp.as_mut_ptr() as _), Some(&mut dsz)).is_ok() { from_wide(&disp) } else { sub.clone() };
                    if !out.iter().any(|(_, c): &(String, String)| c.eq_ignore_ascii_case(&cmd)) {
                        out.push((label, cmd));
                    }
                }
            }
            let _ = RegCloseKey(hk);
        }
    }
    out
}

impl Favorites {
    pub fn load(v: &Value) -> Favorites {
        Favorites { c: jget(v), new_name: String::new(), new_url: String::new(), browsers: None }
    }
    fn open(&self, l: &Link) {
        let mut url = l.url.trim().to_string();
        if !url.contains("://") {
            url = format!("https://{}", url);
        }
        if self.c.browser.is_empty() {
            shell_open(&url);
        } else {
            shell_exec(&self.c.browser, Some(&format!("\"{}\"", url)), None);
        }
    }
}

impl Widget for Favorites {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        if !t.wide() {
            t.card_color(Color::hex(0xFFB340), Color::hex(0xFF8A00));
            t.c.glyph(0xE735, t.r, t.fs(0.42), WHITE);
            return;
        }
        t.card();
        let inner = t.inner();
        let n = self.c.links.len().min((t.units * 2) as usize);
        if n == 0 {
            t.c.text("Sem favoritos", inner, t.fs(0.14), W_MEDIUM, WHITE.a(0.7), Align::Center);
            return;
        }
        let w = inner.w / n as f32;
        for (i, l) in self.c.links.iter().take(n).enumerate() {
            let cell = Rect::new(inner.x + w * i as f32, inner.y, w, inner.h);
            let d = (cell.w * 0.62).min(cell.h * 0.62);
            badge(t.c, Rect::new(cell.cx() - d / 2.0, cell.y, d, d), l);
            t.fit_text(&l.name, Rect::new(cell.x, cell.y + d + t.s * 0.02, cell.w, cell.h - d - t.s * 0.02), t.fs(0.11), W_MEDIUM, WHITE.a(0.85), Align::Center);
        }
    }
    fn on_click(&mut self, fx: f32, _fy: f32, units: u32, _sh: &mut Shared) -> Click {
        if units >= 2 {
            let n = self.c.links.len().min((units * 2) as usize);
            if n > 0 {
                let i = ((fx * n as f32).floor() as usize).min(n - 1);
                let l = self.c.links[i].clone();
                self.open(&l);
                return Click::Handled;
            }
        }
        Click::Panel
    }
    fn menu(&self) -> Vec<(u32, String)> {
        vec![(1, "Abrir a lista".into())]
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (300.0, 70.0 + self.c.links.len().max(1).min(10) as f32 * 42.0)
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        panel_header(ui, "Favoritos", "");
        if self.c.links.is_empty() {
            ui.note("Adicione links em Editar widget (botão direito).");
        }
        for (i, l) in self.c.links.clone().iter().enumerate() {
            let r = ui.row(42.0);
            let (hov, clk) = ui.hit(idn("l", i as u64), r);
            if hov {
                ui.c.fill_rrect(r, 8.0 * ui.s, FILL);
            }
            badge(ui.c, Rect::new(r.x + 6.0 * ui.s, r.y + 6.0 * ui.s, 30.0 * ui.s, 30.0 * ui.s), l);
            ui.c.text(&l.name, Rect::new(r.x + 46.0 * ui.s, r.y + 3.0 * ui.s, r.w - 50.0 * ui.s, r.h * 0.5), 12.5 * ui.s, W_MEDIUM, TXT, Align::Left);
            ui.c.text(&domain(&l.url), Rect::new(r.x + 46.0 * ui.s, r.y + r.h * 0.48, r.w - 50.0 * ui.s, r.h * 0.45), 11.0 * ui.s, W_NORMAL, TXT2, Align::Left);
            if clk {
                self.open(l);
                sh.close_panel = true;
            }
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        ui.caption("LINKS");
        let mut action: Option<(usize, i32)> = None;
        for (i, l) in self.c.links.iter().enumerate() {
            let r = ui.row(34.0);
            badge(ui.c, Rect::new(r.x, r.y + 5.0 * ui.s, 24.0 * ui.s, 24.0 * ui.s), l);
            ui.c.text(&format!("{}  ·  {}", l.name, domain(&l.url)), Rect::new(r.x + 32.0 * ui.s, r.y, r.w - 130.0 * ui.s, r.h), 12.0 * ui.s, W_NORMAL, TXT, Align::Left);
            let b = 28.0 * ui.s;
            if ui.icon_btn(idn("up", i as u64), Rect::new(r.r() - 3.0 * b, r.y + 3.0, b, b), 0xE70E, 10.0, TXT2) {
                action = Some((i, -1));
            }
            if ui.icon_btn(idn("dn", i as u64), Rect::new(r.r() - 2.0 * b, r.y + 3.0, b, b), 0xE70D, 10.0, TXT2) {
                action = Some((i, 1));
            }
            if ui.icon_btn(idn("rm", i as u64), Rect::new(r.r() - b, r.y + 3.0, b, b), 0xE711, 10.0, TXT2) {
                action = Some((i, 0));
            }
        }
        if let Some((i, d)) = action {
            if d == 0 {
                self.c.links.remove(i);
            } else {
                let j = (i as i32 + d).clamp(0, self.c.links.len() as i32 - 1) as usize;
                self.c.links.swap(i, j);
            }
            sh.save = true;
        }
        ui.gap(4.0);
        ui.labeled_input(id("nn"), "Nome", &mut self.new_name, "Ex.: Notícias");
        let res = ui.labeled_input(id("nu"), "Endereço", &mut self.new_url, "https://…");
        let r = ui.row(36.0);
        if (ui.button_k(id("add"), Rect::new(r.r() - 130.0 * ui.s, r.y + 3.0, 130.0 * ui.s, r.h - 6.0), "Adicionar favorito", Btn::Primary) || res.submitted) && !self.new_url.trim().is_empty() {
            let name = if self.new_name.trim().is_empty() { domain(&self.new_url) } else { self.new_name.trim().to_string() };
            self.c.links.push(Link { name, url: self.new_url.trim().to_string() });
            self.new_name.clear();
            self.new_url.clear();
            sh.save = true;
        }
        ui.caption("ABRIR EM");
        if self.browsers.is_none() {
            self.browsers = Some(browsers());
        }
        let mut opts: Vec<(String, String)> = vec![("Navegador padrão".into(), String::new())];
        opts.extend(self.browsers.clone().unwrap_or_default());
        for (i, (label, cmd)) in opts.iter().enumerate() {
            let r = ui.row(30.0);
            let (hov, clk) = ui.hit(idn("br", i as u64), r);
            if hov {
                ui.c.fill_rrect(r, 6.0 * ui.s, FILL);
            }
            if self.c.browser.eq_ignore_ascii_case(cmd) {
                ui.c.glyph(0xE73E, Rect::new(r.x, r.y, 24.0 * ui.s, r.h), 11.0 * ui.s, ACCENT);
            }
            ui.c.text(label, Rect::new(r.x + 26.0 * ui.s, r.y, r.w - 26.0 * ui.s, r.h), 12.5 * ui.s, W_NORMAL, TXT, Align::Left);
            if clk {
                self.c.browser = cmd.clone();
                sh.save = true;
            }
        }
    }
}
