// Conta-gotas: pega a cor de qualquer ponto da tela e copia o código.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::*;
use crate::ui::*;

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Swatch {
    pub hex: String,
    pub pinned: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct Cfg {
    format: u32,
    copy_on_pick: bool,
    hotkey: bool,
    keep: i64,
    colors: Vec<Swatch>,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg { format: 0, copy_on_pick: true, hotkey: false, keep: 12, colors: vec![] }
    }
}

pub struct Eyedropper {
    c: Cfg,
}

const FORMATS: [&str; 6] = ["Hex", "hex minúsculo", "CSS rgb()", "CSS hsl()", "C# Color", "Variável CSS"];

fn parse_hex(h: &str) -> (u8, u8, u8) {
    let v = u32::from_str_radix(h.trim_start_matches('#'), 16).unwrap_or(0);
    (((v >> 16) & 255) as u8, ((v >> 8) & 255) as u8, (v & 255) as u8)
}

pub fn format_color(hex: &str, f: u32) -> String {
    let (r, g, b) = parse_hex(hex);
    match f {
        1 => format!("#{:02x}{:02x}{:02x}", r, g, b),
        2 => format!("rgb({}, {}, {})", r, g, b),
        3 => {
            let (rf, gf, bf) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
            let max = rf.max(gf).max(bf);
            let min = rf.min(gf).min(bf);
            let l = (max + min) / 2.0;
            let d = max - min;
            let (h, s) = if d == 0.0 {
                (0.0, 0.0)
            } else {
                let s = d / (1.0 - (2.0 * l - 1.0).abs());
                let h = if max == rf {
                    60.0 * (((gf - bf) / d).rem_euclid(6.0))
                } else if max == gf {
                    60.0 * ((bf - rf) / d + 2.0)
                } else {
                    60.0 * ((rf - gf) / d + 4.0)
                };
                (h, s)
            };
            format!("hsl({:.0}, {:.0}%, {:.0}%)", h, s * 100.0, l * 100.0)
        }
        4 => format!("Color.FromArgb(255, {}, {}, {})", r, g, b),
        5 => format!("--cor: #{:02X}{:02X}{:02X};", r, g, b),
        _ => format!("#{:02X}{:02X}{:02X}", r, g, b),
    }
}

impl Eyedropper {
    pub fn load(v: &Value) -> Eyedropper {
        Eyedropper { c: jget(v) }
    }
    fn trim(&mut self) {
        let keep = self.c.keep.max(4) as usize;
        let mut unpinned = 0;
        self.c.colors.retain(|s| {
            if s.pinned {
                return true;
            }
            unpinned += 1;
            unpinned <= keep
        });
    }
}

impl Widget for Eyedropper {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn draw(&mut self, t: &Tile, _sh: &mut Shared) {
        t.card();
        let inner = t.inner();
        let cols: Vec<&Swatch> = self.c.colors.iter().take(if t.wide() { 6 } else { 4 }).collect();
        if cols.is_empty() {
            t.c.glyph(0xEF3C, inner, t.fs(0.36), WHITE.a(0.9));
            return;
        }
        if t.wide() {
            let d = inner.h;
            t.c.glyph(0xEF3C, Rect::new(inner.x, inner.y, d * 0.8, d), t.fs(0.3), WHITE.a(0.9));
            let area = Rect::new(inner.x + d * 0.8, inner.y, inner.w - d * 0.8, inner.h);
            let w = area.w / cols.len() as f32;
            for (i, s) in cols.iter().enumerate() {
                let (r, g, b) = parse_hex(&s.hex);
                let rr = Rect::new(area.x + w * i as f32, area.y, w, area.h).inset(t.s * 0.05);
                t.c.fill_rrect(rr, t.s * 0.08, Color::rgba(r, g, b, 1.0));
                t.c.stroke_rrect(rr, t.s * 0.08, Color(1.0, 1.0, 1.0, 0.25), 1.0);
            }
        } else {
            let (r, g, b) = parse_hex(&cols[0].hex);
            let big = Rect::new(inner.x, inner.y, inner.w, inner.h * 0.62);
            t.c.fill_rrect(big, t.s * 0.1, Color::rgba(r, g, b, 1.0));
            t.c.stroke_rrect(big, t.s * 0.1, Color(1.0, 1.0, 1.0, 0.25), 1.0);
            let rest = &cols[1..];
            let w = inner.w / 3.0;
            for (i, s) in rest.iter().enumerate() {
                let (r, g, b) = parse_hex(&s.hex);
                t.c.fill_circle(inner.x + w * i as f32 + w / 2.0, inner.y + inner.h * 0.84, t.s * 0.07, Color::rgba(r, g, b, 1.0));
            }
        }
    }
    fn on_click(&mut self, _fx: f32, _fy: f32, _u: u32, sh: &mut Shared) -> Click {
        sh.eyedrop = Some(sh.current_widget);
        Click::Handled
    }
    fn menu(&self) -> Vec<(u32, String)> {
        vec![(1, "Pegar uma cor".into()), (2, "Ver as cores".into())]
    }
    fn on_menu(&mut self, idm: u32, sh: &mut Shared) {
        if idm == 1 {
            sh.eyedrop = Some(sh.current_widget);
        }
    }
    fn hotkey(&self) -> Option<(u32, u32)> {
        if self.c.hotkey {
            // Ctrl+Alt+P
            Some((0x0002 | 0x0001, 0x50))
        } else {
            None
        }
    }
    fn on_hotkey(&mut self, sh: &mut Shared) {
        sh.eyedrop = Some(sh.current_widget);
    }
    fn on_picked(&mut self, purpose: u32, paths: Vec<String>, sh: &mut Shared) {
        if purpose != 100 {
            return;
        }
        if let Some(hex) = paths.into_iter().next() {
            self.c.colors.retain(|s| s.hex != hex || s.pinned);
            if !self.c.colors.iter().any(|s| s.hex == hex) {
                self.c.colors.insert(0, Swatch { hex: hex.clone(), pinned: false });
            }
            self.trim();
            if self.c.copy_on_pick {
                set_clipboard_text(&format_color(&hex, self.c.format));
            }
            sh.save = true;
            sh.redraw = true;
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (320.0, 120.0 + self.c.colors.len().clamp(1, 10) as f32 * 38.0)
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let r = ui.row(30.0);
        ui.c.text("Conta-gotas", Rect::new(r.x, r.y, r.w * 0.5, r.h), 15.0 * ui.s, W_SEMIBOLD, TXT, Align::Left);
        if ui.button_k(id("pick"), Rect::new(r.r() - 130.0 * ui.s, r.y, 130.0 * ui.s, r.h), "Pegar uma cor", Btn::Primary) {
            sh.eyedrop = Some(sh.current_widget);
            sh.close_panel = true;
        }
        ui.gap(8.0);
        let mut action: Option<(usize, u32)> = None;
        for (i, s) in self.c.colors.iter().enumerate() {
            let r = ui.row(38.0);
            let (r8, g8, b8) = parse_hex(&s.hex);
            let sw = Rect::new(r.x, r.y + 5.0 * ui.s, 28.0 * ui.s, 28.0 * ui.s);
            ui.c.fill_rrect(sw, 7.0 * ui.s, Color::rgba(r8, g8, b8, 1.0));
            ui.c.stroke_rrect(sw, 7.0 * ui.s, Color(1.0, 1.0, 1.0, 0.25), 1.0);
            ui.c.text(&format_color(&s.hex, self.c.format), Rect::new(r.x + 38.0 * ui.s, r.y, r.w - 120.0 * ui.s, r.h), 12.5 * ui.s, W_MEDIUM, TXT, Align::Left);
            let b = 28.0 * ui.s;
            if ui.icon_btn(idn("cp", i as u64), Rect::new(r.r() - 3.0 * b, r.y + 5.0, b, b), 0xE8C8, 11.0, TXT2) {
                action = Some((i, 0));
            }
            if ui.icon_btn(idn("pin", i as u64), Rect::new(r.r() - 2.0 * b, r.y + 5.0, b, b), if s.pinned { 0xE841 } else { 0xE718 }, 11.0, if s.pinned { ACCENT } else { TXT2 }) {
                action = Some((i, 1));
            }
            if ui.icon_btn(idn("rm", i as u64), Rect::new(r.r() - b, r.y + 5.0, b, b), 0xE711, 10.0, TXT2) {
                action = Some((i, 2));
            }
        }
        if let Some((i, a)) = action {
            match a {
                0 => set_clipboard_text(&format_color(&self.c.colors[i].hex, self.c.format)),
                1 => self.c.colors[i].pinned = !self.c.colors[i].pinned,
                _ => {
                    self.c.colors.remove(i);
                }
            }
            sh.save = true;
            sh.redraw = true;
        }
        if self.c.colors.is_empty() {
            ui.note("Clique no widget para pegar uma cor de qualquer lugar da tela. Esc cancela.");
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        ui.caption("COPIAR COMO");
        for (i, f) in FORMATS.iter().enumerate() {
            let r = ui.row(28.0);
            let (hov, clk) = ui.hit(idn("fmt", i as u64), r);
            if hov {
                ui.c.fill_rrect(r, 6.0 * ui.s, FILL);
            }
            if self.c.format == i as u32 {
                ui.c.glyph(0xE73E, Rect::new(r.x, r.y, 24.0 * ui.s, r.h), 11.0 * ui.s, ACCENT);
            }
            ui.c.text(f, Rect::new(r.x + 26.0 * ui.s, r.y, r.w * 0.4, r.h), 12.5 * ui.s, W_NORMAL, TXT, Align::Left);
            ui.c.text(&format_color("#3A8DDE", i as u32), Rect::new(r.x, r.y, r.w - 8.0, r.h), 11.5 * ui.s, W_NORMAL, TXT2, Align::Right);
            if clk {
                self.c.format = i as u32;
                sh.save = true;
            }
        }
        let mut ch = ui.row_toggle(id("cop"), "Copiar ao pegar", &mut self.c.copy_on_pick);
        ch |= ui.row_toggle(id("hk"), "Atalho global Ctrl+Alt+P", &mut self.c.hotkey);
        ch |= ui.stepper(id("keep"), "Quantas guardar", &mut self.c.keep, 4, 60, 1, "");
        if ch {
            self.trim();
            sh.save = true;
            sh.rewatch = true;
        }
    }
}
