// Tocando agora: capa, título, artista e controles, de qualquer player.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::*;
use crate::media::Cmd;
use crate::ui::*;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct Cfg {
    art_on_icon: bool,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg { art_on_icon: true }
    }
}

pub struct Music {
    c: Cfg,
}

impl Music {
    pub fn load(v: &Value) -> Music {
        Music { c: jget(v) }
    }
}

fn placeholder_art(c: &Canvas, r: Rect, rad: f32) {
    // capa padrão: degradê com uma nota musical desenhada
    c.fill_rrect_grad(r, rad, 135.0, &[(0.0, Color::hex(0xFF5E7E)), (0.55, Color::hex(0xC03CF0)), (1.0, Color::hex(0x6A3CFF))]);
    let s = r.w.min(r.h);
    let (cx, cy) = (r.cx(), r.cy());
    let w = (s * 0.055).max(1.5);
    let head = s * 0.095;
    let x1 = cx - s * 0.14;
    let x2 = cx + s * 0.16;
    let top1 = cy - s * 0.24;
    let top2 = cy - s * 0.30;
    let y1 = cy + s * 0.16;
    let y2 = cy + s * 0.10;
    let col = Color(1.0, 1.0, 1.0, 0.95);
    c.line(x1 + head * 0.9, y1, x1 + head * 0.9, top1, col, w);
    c.line(x2 + head * 0.9, y2, x2 + head * 0.9, top2, col, w);
    c.fill_poly(&[(x1 + head * 0.9 - w / 2.0, top1), (x2 + head * 0.9 + w / 2.0, top2), (x2 + head * 0.9 + w / 2.0, top2 + s * 0.09), (x1 + head * 0.9 - w / 2.0, top1 + s * 0.09)], col);
    c.fill_ellipse(x1, y1, head * 1.05, head * 0.82, col);
    c.fill_ellipse(x2, y2, head * 1.05, head * 0.82, col);
}

fn eq_bars(c: &Canvas, x: f32, y: f32, h: f32, col: Color) {
    let w = h * 0.18;
    for (i, f) in [0.55, 1.0, 0.75].iter().enumerate() {
        let bh = h * f;
        c.fill_rrect(Rect::new(x + i as f32 * w * 1.6, y + h - bh, w, bh), w / 2.0, col);
    }
}

impl Widget for Music {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn needs_media(&self) -> bool {
        true
    }
    fn draw(&mut self, t: &Tile, sh: &mut Shared) {
        let m = &sh.media;
        let has = m.has_session && !m.title.is_empty();
        if !t.wide() {
            if has && self.c.art_on_icon && sh.media_art.is_some() {
                t.c.fill_rrect(t.r, t.rad(), Color::rgba(22, 22, 26, 1.0));
                t.c.bitmap_cover_rounded(sh.media_art.as_ref().unwrap(), t.r, t.rad(), 1.0);
                if !m.playing {
                    // pausado: um véu discreto, sem ícone
                    t.c.fill_rrect(t.r, t.rad(), Color(0.0, 0.0, 0.0, 0.38));
                }
                t.c.stroke_rrect(t.r.inset(0.5), t.rad(), Color(1.0, 1.0, 1.0, 0.15), 1.0);
            } else {
                placeholder_art(t.c, t.r, t.rad());
                if has && !m.playing {
                    t.c.fill_rrect(t.r, t.rad(), Color(0.0, 0.0, 0.0, 0.25));
                }
            }
            if has && m.playing {
                let d = t.s * 0.26;
                let br = Rect::new(t.r.r() - d - t.s * 0.06, t.r.b() - d - t.s * 0.06, d, d);
                t.c.fill_circle(br.cx(), br.cy(), d / 2.0 + t.s * 0.02, Color(0.0, 0.0, 0.0, 0.55));
                eq_bars(t.c, br.x + d * 0.22, br.y + d * 0.25, d * 0.5, WHITE);
            }
            return;
        }
        t.card();
        let inner = t.inner();
        let d = inner.h;
        let art = Rect::new(inner.x, inner.y, d, d);
        if has && sh.media_art.is_some() {
            t.c.bitmap_cover_rounded(sh.media_art.as_ref().unwrap(), art, t.s * 0.14, 1.0);
        } else {
            placeholder_art(t.c, art, t.s * 0.14);
        }
        let ctrl_w = if t.units >= 3 { t.s * 1.05 } else { 0.0 };
        let tr = Rect::new(art.r() + t.s * 0.14, inner.y, inner.w - d - t.s * 0.14 - ctrl_w, inner.h);
        if has {
            t.fit_text(&m.title, Rect::new(tr.x, tr.y + tr.h * 0.04, tr.w, tr.h * 0.5), t.fs(0.16), W_SEMIBOLD, WHITE, Align::Left);
            t.fit_text(&m.artist, Rect::new(tr.x, tr.y + tr.h * 0.5, tr.w, tr.h * 0.42), t.fs(0.135), W_NORMAL, WHITE.a(0.62), Align::Left);
        } else {
            t.fit_text("Nada tocando", Rect::new(tr.x, tr.y, tr.w, tr.h * 0.55), t.fs(0.15), W_SEMIBOLD, WHITE.a(0.85), Align::Left);
            t.fit_text("Abra um player", Rect::new(tr.x, tr.y + tr.h * 0.5, tr.w, tr.h * 0.42), t.fs(0.13), W_NORMAL, WHITE.a(0.5), Align::Left);
        }
        if ctrl_w > 0.0 {
            let cr = Rect::new(inner.r() - ctrl_w, inner.y, ctrl_w, inner.h);
            let bw = cr.w / 3.0;
            let glyphs = [0xE892, if m.playing { 0xE769 } else { 0xE768 }, 0xE893];
            for (i, g) in glyphs.iter().enumerate() {
                let br = Rect::new(cr.x + bw * i as f32, cr.y, bw, cr.h);
                t.c.glyph(*g, br, t.fs(if i == 1 { 0.24 } else { 0.18 }), WHITE.a(if has { 0.95 } else { 0.4 }));
            }
        }
    }
    fn on_click(&mut self, fx: f32, _fy: f32, units: u32, sh: &mut Shared) -> Click {
        if units >= 3 {
            // a área dos controles ocupa ~1.05 ícone no fim do tile
            let total = units as f32 + (units as f32 - 1.0) * 0.18;
            let ctrl_start = 1.0 - (1.05 + 0.14) / total;
            if fx > ctrl_start {
                let rel = ((fx - ctrl_start) / (1.0 - ctrl_start) * 3.0).floor() as i32;
                sh.media_cmd(match rel {
                    0 => Cmd::Prev,
                    1 => Cmd::PlayPause,
                    _ => Cmd::Next,
                });
                return Click::Handled;
            }
        }
        Click::Panel
    }
    fn double_click(&self) -> bool {
        true
    }
    fn on_double_click(&mut self, sh: &mut Shared) {
        sh.media_cmd(Cmd::PlayPause);
    }
    fn menu(&self) -> Vec<(u32, String)> {
        vec![(2, "Tocar/pausar".into()), (3, "Próxima".into()), (4, "Anterior".into()), (5, "Mostrar o player".into())]
    }
    fn on_menu(&mut self, idm: u32, sh: &mut Shared) {
        match idm {
            2 => sh.media_cmd(Cmd::PlayPause),
            3 => sh.media_cmd(Cmd::Next),
            4 => sh.media_cmd(Cmd::Prev),
            5 => {
                if !sh.media.app_id.is_empty() {
                    sh.focus_app = Some(sh.media.app_id.clone());
                }
            }
            _ => {}
        }
    }
    fn panel_size(&self, sh: &Shared) -> (f32, f32) {
        let has = sh.media.has_session && !sh.media.title.is_empty();
        (320.0, if has { 420.0 } else { 350.0 })
    }
    fn panel_interval(&self, sh: &Shared) -> Option<u32> {
        if sh.media.playing {
            Some(1000)
        } else {
            None
        }
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let m = sh.media.clone();
        let r = ui.row(200.0);
        let art = r.centered(r.h, r.h);
        if let Some(b) = &sh.media_art {
            ui.c.shadow(art, 14.0 * ui.s, 14.0 * ui.s, 6.0 * ui.s, Color(0.0, 0.0, 0.0, 0.35));
            ui.c.bitmap_cover_rounded(b, art, 14.0 * ui.s, 1.0);
        } else {
            placeholder_art(ui.c, art, 14.0 * ui.s);
        }
        ui.gap(10.0);
        let r = ui.row(24.0);
        ui.c.text(if m.title.is_empty() { "Nada tocando" } else { &m.title }, r, 15.0 * ui.s, W_SEMIBOLD, TXT, Align::Center);
        let r = ui.row(20.0);
        ui.c.text(&m.artist, r, 12.5 * ui.s, W_NORMAL, TXT2, Align::Center);
        ui.gap(8.0);
        if m.dur > 0.0 {
            let r = ui.row(14.0);
            let pos = m.position_now();
            let track = Rect::new(r.x, r.y + 4.0, r.w, 5.0 * ui.s);
            bar(ui.c, track, (pos / m.dur) as f32, Color(1.0, 1.0, 1.0, 0.15), WHITE.a(0.85));
            let (_, clicked) = ui.hit(id("seek"), r);
            if clicked {
                let f = ((ui.inp.mx - r.x) / r.w).clamp(0.0, 1.0) as f64;
                sh.media_cmd(Cmd::Seek(f * m.dur));
            }
            let r = ui.row(16.0);
            ui.c.text(&fmt_duration_short(pos as i64), r, 10.5 * ui.s, W_NORMAL, TXT3, Align::Left);
            ui.c.text(&fmt_duration_short(m.dur as i64), r, 10.5 * ui.s, W_NORMAL, TXT3, Align::Right);
        } else {
            ui.gap(30.0);
        }
        let r = ui.row(52.0);
        let bw = 52.0 * ui.s;
        let gap = 18.0 * ui.s;
        let x0 = r.cx() - bw * 1.5 - gap;
        if ui.icon_btn(id("prev"), Rect::new(x0, r.y, bw, bw), 0xE892, 18.0, TXT) {
            sh.media_cmd(Cmd::Prev);
        }
        if ui.icon_btn(id("pp"), Rect::new(x0 + bw + gap, r.y, bw, bw), if m.playing { 0xE769 } else { 0xE768 }, 24.0, TXT) {
            sh.media_cmd(Cmd::PlayPause);
        }
        if ui.icon_btn(id("next"), Rect::new(x0 + 2.0 * (bw + gap), r.y, bw, bw), 0xE893, 18.0, TXT) {
            sh.media_cmd(Cmd::Next);
        }
        if m.has_session && !m.app_id.is_empty() {
            ui.gap(6.0);
            let r = ui.row(30.0);
            if ui.button_k(id("show"), r.centered(150.0 * ui.s, r.h), "Mostrar o player", Btn::Ghost) {
                sh.focus_app = Some(m.app_id.clone());
                sh.close_panel = true;
            }
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        if ui.row_toggle(id("art"), "Capa do álbum no ícone", &mut self.c.art_on_icon) {
            sh.save = true;
        }
        ui.note("Funciona com Spotify, navegadores (YouTube, YouTube Music, Deezer…), o app Música e qualquer player que aparece nos controles de mídia do Windows.");
    }
}
