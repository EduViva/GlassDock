// Kit de interface em modo imediato, desenhado com Direct2D: botões,
// chaves, segmentados, campos de texto, listas e rolagem. Cada evento do
// mouse/teclado gera um quadro; nada é redesenhado sem evento.
use std::collections::HashMap;
use windows::Win32::Graphics::DirectWrite::*;

use crate::gfx::*;
use crate::util::*;

pub fn id(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn idn(s: &str, n: u64) -> u64 {
    id(s) ^ n.wrapping_mul(0x9E3779B97F4A7C15)
}

#[derive(Default, Clone)]
pub struct Input {
    pub mx: f32,
    pub my: f32,
    pub inside: bool,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
    pub rreleased: bool,
    pub dbl: bool,
    pub wheel: f32,
    pub keys: Vec<(u32, bool, bool)>,
    pub chars: Vec<char>,
}

impl Input {
    pub fn clear_events(&mut self) {
        self.pressed = false;
        self.released = false;
        self.rreleased = false;
        self.dbl = false;
        self.wheel = 0.0;
        self.keys.clear();
        self.chars.clear();
    }
}

#[derive(Default, Clone, Copy)]
pub struct TextEdit {
    pub caret: usize,
    pub anchor: usize,
}

#[derive(Default)]
pub struct UiState {
    pub active: Option<u64>,
    pub focus: Option<u64>,
    pub texts: HashMap<u64, TextEdit>,
    pub scroll: HashMap<u64, f32>,
    pub capture: bool,
    pub drag_value: f32,
}

pub struct TextResult {
    pub changed: bool,
    pub submitted: bool,
}

pub const TXT: Color = Color(1.0, 1.0, 1.0, 0.94);
pub const TXT2: Color = Color(1.0, 1.0, 1.0, 0.62);
pub const TXT3: Color = Color(1.0, 1.0, 1.0, 0.38);
pub const FILL: Color = Color(1.0, 1.0, 1.0, 0.08);
pub const FILL_HOVER: Color = Color(1.0, 1.0, 1.0, 0.14);
pub const LINE: Color = Color(1.0, 1.0, 1.0, 0.10);

#[derive(Clone, Copy, PartialEq)]
pub enum Btn {
    Normal,
    Primary,
    Danger,
    Ghost,
}

pub struct Ui<'a> {
    pub c: &'a Canvas,
    pub inp: &'a Input,
    pub st: &'a mut UiState,
    pub s: f32,
    pub changed: bool,
    pub close: bool,
    pub area: Rect,
    pub y: f32,
    clip_stack: Vec<(f32, Rect)>,
    pub blocked: bool,
}

impl<'a> Ui<'a> {
    pub fn new(c: &'a Canvas, inp: &'a Input, st: &'a mut UiState, s: f32, area: Rect) -> Ui<'a> {
        Ui { c, inp, st, s, changed: false, close: false, area, y: area.y, clip_stack: vec![], blocked: false }
    }

    pub fn sp(&self, v: f32) -> f32 {
        v * self.s
    }

    /// Reserva uma linha de altura `h` (pontos) no fluxo vertical.
    pub fn row(&mut self, h: f32) -> Rect {
        let r = Rect::new(self.area.x, self.y, self.area.w, h * self.s);
        self.y += h * self.s;
        r
    }

    pub fn gap(&mut self, h: f32) {
        self.y += h * self.s;
    }

    pub fn mouse_in(&self, r: Rect) -> bool {
        if self.blocked || !self.inp.inside {
            return false;
        }
        if let Some((_, clip)) = self.clip_stack.last() {
            if !clip.contains(self.inp.mx, self.inp.my) {
                return false;
            }
        }
        r.contains(self.inp.mx, self.inp.my)
    }

    /// Área clicável genérica: devolve (hover, clique).
    pub fn hit(&mut self, idv: u64, r: Rect) -> (bool, bool) {
        let hov = self.mouse_in(r);
        if hov && self.inp.pressed {
            self.st.active = Some(idv);
            if self.st.focus.is_some() && self.st.focus != Some(idv) {
                self.st.focus = None;
            }
        }
        let clicked = self.inp.released && self.st.active == Some(idv) && hov;
        if clicked {
            self.changed = true;
        }
        (hov, clicked)
    }

    pub fn right_clicked(&self, r: Rect) -> bool {
        self.inp.rreleased && self.mouse_in(r)
    }

    pub fn label(&self, r: Rect, t: &str, size: f32, weight: u32, col: Color, align: Align) {
        self.c.text(t, r, size * self.s, weight, col, align);
    }

    pub fn title(&mut self, t: &str) {
        let r = self.row(30.0);
        self.c.text(t, r, 15.0 * self.s, W_SEMIBOLD, TXT, Align::Left);
    }

    pub fn caption(&mut self, t: &str) {
        let r = self.row(22.0);
        self.c.text(t, r, 11.5 * self.s, W_SEMIBOLD, TXT2, Align::Left);
    }

    pub fn note(&mut self, t: &str) {
        let (_, h) = self.c.g.measure_wrapped(t, 11.5 * self.s, W_NORMAL, self.area.w);
        let r = Rect::new(self.area.x, self.y, self.area.w, h + 2.0);
        self.y += h + self.sp(6.0);
        self.c.text_wrap(t, r, 11.5 * self.s, W_NORMAL, TXT2, Align::Left);
    }

    pub fn button_k(&mut self, idv: u64, r: Rect, t: &str, kind: Btn) -> bool {
        let (hov, clicked) = self.hit(idv, r);
        let pressed = self.st.active == Some(idv) && self.inp.down && hov;
        let rad = (r.h / 2.0).min(self.sp(9.0));
        let (fill, txt) = match kind {
            Btn::Primary => (if hov { ACCENT.mix(WHITE, 0.12) } else { ACCENT }, WHITE),
            Btn::Danger => (if hov { RED.a(0.32) } else { RED.a(0.22) }, Color::hex(0xFF7A70)),
            Btn::Ghost => (if hov { FILL } else { TRANSPARENT }, TXT),
            Btn::Normal => (if hov { FILL_HOVER } else { FILL }, TXT),
        };
        self.c.fill_rrect(r, rad, if pressed { fill.a(0.75) } else { fill });
        if kind == Btn::Normal {
            self.c.stroke_rrect(r.inset(0.5), rad, LINE, 1.0);
        }
        self.c.text(t, r, 12.5 * self.s, W_MEDIUM, txt, Align::Center);
        clicked
    }

    pub fn button(&mut self, idv: u64, r: Rect, t: &str) -> bool {
        self.button_k(idv, r, t, Btn::Normal)
    }

    pub fn icon_btn(&mut self, idv: u64, r: Rect, glyph: u32, size: f32, col: Color) -> bool {
        let (hov, clicked) = self.hit(idv, r);
        if hov {
            self.c.fill_rrect(r, r.h.min(r.w) / 2.0, FILL_HOVER);
        }
        self.c.glyph(glyph, r, size * self.s, col);
        clicked
    }

    pub fn toggle(&mut self, idv: u64, r: Rect, label: &str, v: &mut bool) -> bool {
        let (_, clicked) = self.hit(idv, r);
        if clicked {
            *v = !*v;
        }
        let sw = self.sp(36.0);
        let sh = self.sp(20.0);
        let sr = Rect::new(r.r() - sw, r.cy() - sh / 2.0, sw, sh);
        self.c.text(label, Rect::new(r.x, r.y, r.w - sw - self.sp(8.0), r.h), 12.5 * self.s, W_NORMAL, TXT, Align::Left);
        self.c.fill_rrect(sr, sh / 2.0, if *v { ACCENT } else { Color(1.0, 1.0, 1.0, 0.18) });
        let k = sh - self.sp(4.0);
        let kx = if *v { sr.r() - k - self.sp(2.0) } else { sr.x + self.sp(2.0) };
        self.c.fill_circle(kx + k / 2.0, sr.cy(), k / 2.0, WHITE);
        clicked
    }

    pub fn checkbox(&mut self, idv: u64, r: Rect, v: &mut bool, col: Color) -> bool {
        let (hov, clicked) = self.hit(idv, r);
        if clicked {
            *v = !*v;
        }
        let d = r.h.min(r.w);
        let cr = Rect::new(r.x, r.cy() - d / 2.0, d, d);
        if *v {
            self.c.fill_circle(cr.cx(), cr.cy(), d / 2.0, col);
            self.c.glyph(0xE73E, cr, d * 0.6, WHITE);
        } else {
            self.c.stroke_circle(cr.cx(), cr.cy(), d / 2.0 - 1.0, if hov { TXT } else { TXT2 }, 1.5);
        }
        clicked
    }

    pub fn segmented(&mut self, idv: u64, r: Rect, opts: &[&str], v: &mut u32) -> bool {
        let rad = self.sp(8.0);
        self.c.fill_rrect(r, rad, FILL);
        let n = opts.len().max(1) as f32;
        let w = r.w / n;
        let mut ch = false;
        for (i, o) in opts.iter().enumerate() {
            let cr = Rect::new(r.x + w * i as f32, r.y, w, r.h);
            let (hov, clicked) = self.hit(idv ^ (i as u64 + 1), cr);
            if clicked && *v != i as u32 {
                *v = i as u32;
                ch = true;
            }
            if *v == i as u32 {
                self.c.fill_rrect(cr.inset(self.sp(2.0)), rad - self.sp(2.0), Color(1.0, 1.0, 1.0, 0.22));
            } else if hov {
                self.c.fill_rrect(cr.inset(self.sp(2.0)), rad - self.sp(2.0), Color(1.0, 1.0, 1.0, 0.07));
            }
            self.c.text(o, cr, 12.0 * self.s, if *v == i as u32 { W_SEMIBOLD } else { W_NORMAL }, TXT, Align::Center);
        }
        ch
    }

    pub fn labeled_segmented(&mut self, idv: u64, label: &str, opts: &[&str], v: &mut u32) -> bool {
        let r = self.row(34.0);
        let lw = (r.w * 0.38).min(self.sp(170.0));
        self.c.text(label, Rect::new(r.x, r.y, lw, r.h), 12.5 * self.s, W_NORMAL, TXT, Align::Left);
        let sr = Rect::new(r.x + lw, r.y + self.sp(4.0), r.w - lw, r.h - self.sp(8.0));
        self.segmented(idv, sr, opts, v)
    }

    pub fn row_toggle(&mut self, idv: u64, label: &str, v: &mut bool) -> bool {
        let r = self.row(34.0);
        self.toggle(idv, r, label, v)
    }

    pub fn slider(&mut self, idv: u64, r: Rect, v: &mut f32, min: f32, max: f32) -> bool {
        let (hov, _) = self.hit(idv, r);
        let track = Rect::new(r.x, r.cy() - self.sp(2.0), r.w, self.sp(4.0));
        let t = ((*v - min) / (max - min)).clamp(0.0, 1.0);
        let mut ch = false;
        if self.st.active == Some(idv) && self.inp.down {
            let nt = ((self.inp.mx - r.x) / r.w).clamp(0.0, 1.0);
            let nv = min + nt * (max - min);
            if (nv - *v).abs() > f32::EPSILON {
                *v = nv;
                ch = true;
                self.changed = true;
            }
            self.st.capture = true;
        }
        if hov && self.inp.wheel != 0.0 {
            *v = (*v + self.inp.wheel.signum() * (max - min) / 20.0).clamp(min, max);
            ch = true;
            self.changed = true;
        }
        self.c.fill_rrect(track, self.sp(2.0), Color(1.0, 1.0, 1.0, 0.18));
        self.c.fill_rrect(Rect::new(track.x, track.y, track.w * t, track.h), self.sp(2.0), ACCENT);
        let kx = r.x + r.w * ((*v - min) / (max - min)).clamp(0.0, 1.0);
        self.c.fill_circle(kx, r.cy(), self.sp(8.0), WHITE);
        self.c.stroke_circle(kx, r.cy(), self.sp(8.0), Color(0.0, 0.0, 0.0, 0.15), 1.0);
        ch
    }

    pub fn labeled_slider(&mut self, idv: u64, label: &str, v: &mut f32, min: f32, max: f32, fmt: &str) -> bool {
        let r = self.row(34.0);
        let lw = (r.w * 0.38).min(self.sp(170.0));
        self.c.text(label, Rect::new(r.x, r.y, lw, r.h), 12.5 * self.s, W_NORMAL, TXT, Align::Left);
        let vw = self.sp(56.0);
        let sr = Rect::new(r.x + lw + self.sp(8.0), r.y, r.w - lw - vw - self.sp(16.0), r.h);
        let ch = self.slider(idv, sr, v, min, max);
        let txt = fmt.replace("{}", &format!("{:.0}", *v));
        self.c.text(&txt, Rect::new(r.r() - vw, r.y, vw, r.h), 12.0 * self.s, W_NORMAL, TXT2, Align::Right);
        ch
    }

    pub fn stepper(&mut self, idv: u64, label: &str, v: &mut i64, min: i64, max: i64, step: i64, suffix: &str) -> bool {
        let r = self.row(34.0);
        self.c.text(label, Rect::new(r.x, r.y, r.w * 0.6, r.h), 12.5 * self.s, W_NORMAL, TXT, Align::Left);
        let bw = self.sp(28.0);
        let vw = self.sp(78.0);
        let plus = Rect::new(r.r() - bw, r.cy() - bw / 2.0, bw, bw);
        let val = Rect::new(plus.x - vw, r.y, vw, r.h);
        let minus = Rect::new(val.x - bw, r.cy() - bw / 2.0, bw, bw);
        let mut ch = false;
        if self.icon_btn(idv ^ 1, minus, 0xE738, 11.0, TXT) && *v > min {
            *v = (*v - step).max(min);
            ch = true;
        }
        if self.icon_btn(idv ^ 2, plus, 0xE710, 11.0, TXT) && *v < max {
            *v = (*v + step).min(max);
            ch = true;
        }
        let (hov, _) = self.hit(idv ^ 3, val);
        if hov && self.inp.wheel != 0.0 {
            *v = (*v + step * self.inp.wheel.signum() as i64).clamp(min, max);
            ch = true;
            self.changed = true;
        }
        self.c.text(&format!("{}{}", v, suffix), val, 12.5 * self.s, W_SEMIBOLD, TXT, Align::Center);
        ch
    }

    pub fn swatches(&mut self, idv: u64, r: Rect, cols: &[Color], v: &mut u32) -> bool {
        let d = r.h.min(self.sp(24.0));
        let mut ch = false;
        for (i, c) in cols.iter().enumerate() {
            let cr = Rect::new(r.x + i as f32 * (d + self.sp(8.0)), r.cy() - d / 2.0, d, d);
            let (_, clicked) = self.hit(idv ^ (i as u64 + 7), cr);
            if clicked {
                *v = i as u32;
                ch = true;
            }
            self.c.fill_circle(cr.cx(), cr.cy(), d / 2.0, *c);
            if *v == i as u32 {
                self.c.stroke_circle(cr.cx(), cr.cy(), d / 2.0 + self.sp(2.5), WHITE, 2.0);
            }
        }
        ch
    }

    // ------------------------------------------------------------------
    // Campo de texto
    // ------------------------------------------------------------------

    pub fn text_input(&mut self, idv: u64, r: Rect, val: &mut String, placeholder: &str, multiline: bool) -> TextResult {
        let mut res = TextResult { changed: false, submitted: false };
        let (hov, _) = self.hit(idv, r);
        let size = 12.5 * self.s;
        let pad = self.sp(9.0);
        let inner = Rect::new(r.x + pad, r.y + if multiline { pad * 0.7 } else { 0.0 }, r.w - 2.0 * pad, r.h - if multiline { pad * 1.4 } else { 0.0 });
        let layout_w = if multiline { inner.w } else { 100000.0 };
        if hov && self.inp.pressed {
            self.st.focus = Some(idv);
            self.changed = true;
            let scroll = *self.st.scroll.get(&idv).unwrap_or(&0.0);
            if let Some(l) = self.c.g.layout(val, size, W_NORMAL, layout_w, multiline) {
                let (lx, ly) = if multiline { (self.inp.mx - inner.x, self.inp.my - inner.y + scroll) } else { (self.inp.mx - inner.x + scroll, size * 0.5) };
                let pos = hit_index(&l, lx, ly, val);
                self.st.texts.insert(idv, TextEdit { caret: pos, anchor: pos });
            }
        }
        let focused = self.st.focus == Some(idv);
        let mut te = *self.st.texts.get(&idv).unwrap_or(&TextEdit { caret: val.chars().count(), anchor: val.chars().count() });
        let n = val.chars().count();
        te.caret = te.caret.min(n);
        te.anchor = te.anchor.min(n);
        if focused {
            for &(vk, ctrl, shift) in &self.inp.keys {
                let n = val.chars().count();
                let sel = te.caret != te.anchor;
                let (a, b) = (te.caret.min(te.anchor), te.caret.max(te.anchor));
                match vk {
                    0x25 => {
                        if te.caret > 0 {
                            te.caret -= 1;
                        }
                        if !shift {
                            te.anchor = te.caret;
                        }
                    }
                    0x27 => {
                        if te.caret < n {
                            te.caret += 1;
                        }
                        if !shift {
                            te.anchor = te.caret;
                        }
                    }
                    0x24 => {
                        te.caret = if multiline { line_start(val, te.caret) } else { 0 };
                        if !shift {
                            te.anchor = te.caret;
                        }
                    }
                    0x23 => {
                        te.caret = if multiline { line_end(val, te.caret) } else { n };
                        if !shift {
                            te.anchor = te.caret;
                        }
                    }
                    0x08 => {
                        if sel {
                            remove_range(val, a, b);
                            te.caret = a;
                        } else if te.caret > 0 {
                            remove_range(val, te.caret - 1, te.caret);
                            te.caret -= 1;
                        }
                        te.anchor = te.caret;
                        res.changed = true;
                    }
                    0x2E => {
                        if sel {
                            remove_range(val, a, b);
                            te.caret = a;
                        } else if te.caret < n {
                            remove_range(val, te.caret, te.caret + 1);
                        }
                        te.anchor = te.caret;
                        res.changed = true;
                    }
                    0x0D => {
                        if multiline && !ctrl {
                            if sel {
                                remove_range(val, a, b);
                                te.caret = a;
                            }
                            insert_at(val, te.caret, "\n");
                            te.caret += 1;
                            te.anchor = te.caret;
                            res.changed = true;
                        } else {
                            res.submitted = true;
                        }
                    }
                    0x1B => {
                        self.st.focus = None;
                    }
                    0x41 if ctrl => {
                        te.anchor = 0;
                        te.caret = n;
                    }
                    0x43 | 0x58 if ctrl => {
                        if sel {
                            let s: String = val.chars().skip(a).take(b - a).collect();
                            set_clipboard_text(&s);
                            if vk == 0x58 {
                                remove_range(val, a, b);
                                te.caret = a;
                                te.anchor = a;
                                res.changed = true;
                            }
                        }
                    }
                    0x56 if ctrl => {
                        if let Some(mut s) = get_clipboard_text() {
                            if !multiline {
                                s = s.replace(['\r', '\n'], " ");
                            } else {
                                s = s.replace("\r\n", "\n");
                            }
                            if sel {
                                remove_range(val, a, b);
                                te.caret = a;
                            }
                            insert_at(val, te.caret, &s);
                            te.caret += s.chars().count();
                            te.anchor = te.caret;
                            res.changed = true;
                        }
                    }
                    _ => {}
                }
            }
            for &ch in &self.inp.chars {
                if ch < ' ' {
                    continue;
                }
                let (a, b) = (te.caret.min(te.anchor), te.caret.max(te.anchor));
                if a != b {
                    remove_range(val, a, b);
                    te.caret = a;
                }
                insert_at(val, te.caret, &ch.to_string());
                te.caret += 1;
                te.anchor = te.caret;
                res.changed = true;
            }
            self.st.texts.insert(idv, te);
        }
        if res.changed || res.submitted {
            self.changed = true;
        }
        // desenho
        let rad = self.sp(8.0);
        self.c.fill_rrect(r, rad, if focused { Color(1.0, 1.0, 1.0, 0.12) } else { FILL });
        self.c.stroke_rrect(r.inset(0.5), rad, if focused { ACCENT.a(0.9) } else { LINE }, if focused { 1.5 } else { 1.0 });
        self.c.push_clip(inner);
        if val.is_empty() {
            let pr = if multiline { Rect::new(inner.x, inner.y, inner.w, size * 1.4) } else { inner };
            self.c.text(placeholder, pr, size, W_NORMAL, TXT3, Align::Left);
            if focused {
                let x = inner.x;
                let (y0, y1) = if multiline { (inner.y, inner.y + size * 1.35) } else { (inner.cy() - size * 0.7, inner.cy() + size * 0.7) };
                self.c.line(x, y0, x, y1, ACCENT, 1.5);
            }
        } else if let Some(l) = self.c.g.layout(val, size, W_NORMAL, layout_w, multiline) {
            let mut m = DWRITE_TEXT_METRICS::default();
            unsafe {
                let _ = l.GetMetrics(&mut m);
            }
            let caret_pt = caret_xy(&l, val, te.caret);
            let mut scroll = *self.st.scroll.get(&idv).unwrap_or(&0.0);
            if multiline {
                let lh = size * 1.35;
                if focused {
                    if caret_pt.1 - scroll < 0.0 {
                        scroll = caret_pt.1;
                    }
                    if caret_pt.1 + lh - scroll > inner.h {
                        scroll = caret_pt.1 + lh - inner.h;
                    }
                }
                if hov && self.inp.wheel != 0.0 {
                    scroll -= self.inp.wheel * lh * 3.0;
                }
                scroll = scroll.clamp(0.0, (m.height - inner.h).max(0.0));
            } else if focused {
                if caret_pt.0 - scroll > inner.w - 4.0 {
                    scroll = caret_pt.0 - inner.w + 4.0;
                }
                if caret_pt.0 - scroll < 0.0 {
                    scroll = caret_pt.0;
                }
                scroll = scroll.max(0.0);
            } else {
                scroll = 0.0;
            }
            self.st.scroll.insert(idv, scroll);
            let (ox, oy) = if multiline { (inner.x, inner.y - scroll) } else { (inner.x - scroll, inner.cy() - m.height / 2.0) };
            if focused && te.caret != te.anchor {
                let (a, b) = (te.caret.min(te.anchor), te.caret.max(te.anchor));
                for (x0, y0, x1, y1) in sel_rects(&l, val, a, b) {
                    self.c.fill_rect(Rect::new(ox + x0, oy + y0, x1 - x0, y1 - y0), ACCENT.a(0.45));
                }
            }
            self.c.layout_text(&l, ox, oy, TXT);
            if focused {
                let x = ox + caret_pt.0;
                let y = oy + caret_pt.1;
                self.c.line(x, y, x, y + caret_pt.2, ACCENT, 1.5);
            }
        }
        self.c.pop_axis_clip();
        res
    }

    pub fn labeled_input(&mut self, idv: u64, label: &str, val: &mut String, placeholder: &str) -> TextResult {
        let r = self.row(36.0);
        let lw = (r.w * 0.38).min(self.sp(170.0));
        self.c.text(label, Rect::new(r.x, r.y, lw, r.h), 12.5 * self.s, W_NORMAL, TXT, Align::Left);
        let ir = Rect::new(r.x + lw, r.y + self.sp(3.0), r.w - lw, r.h - self.sp(6.0));
        self.text_input(idv, ir, val, placeholder, false)
    }

    // ------------------------------------------------------------------
    // Rolagem
    // ------------------------------------------------------------------

    /// Começa uma área rolável; devolve o deslocamento já aplicado.
    pub fn begin_scroll(&mut self, idv: u64, r: Rect, content_h: f32) -> f32 {
        let mut off = *self.st.scroll.get(&idv).unwrap_or(&0.0);
        let max = (content_h - r.h).max(0.0);
        if self.mouse_in(r) && self.inp.wheel != 0.0 && self.st.focus.map(|f| self.st.texts.contains_key(&f)).unwrap_or(false) == false {
            off -= self.inp.wheel * self.sp(48.0);
            self.changed = true;
        } else if self.mouse_in(r) && self.inp.wheel != 0.0 {
            off -= self.inp.wheel * self.sp(48.0);
            self.changed = true;
        }
        off = off.clamp(0.0, max);
        self.st.scroll.insert(idv, off);
        self.c.push_clip(r);
        self.clip_stack.push((off, r));
        if max > 0.0 {
            let bar_h = (r.h * r.h / content_h).max(self.sp(24.0));
            let by = r.y + (r.h - bar_h) * (off / max);
            self.c.fill_rrect(Rect::new(r.r() - self.sp(4.0), by, self.sp(3.0), bar_h), self.sp(1.5), Color(1.0, 1.0, 1.0, 0.25));
        }
        off
    }

    pub fn end_scroll(&mut self) {
        if self.clip_stack.pop().is_some() {
            self.c.pop_axis_clip();
        }
    }

    pub fn separator(&mut self) {
        let r = self.row(13.0);
        self.c.fill_rect(Rect::new(r.x, r.cy(), r.w, 1.0), LINE);
    }
}

// ---------------------------------------------------------------------------
// Auxiliares de texto (índices em caracteres <-> UTF-16 do DirectWrite)
// ---------------------------------------------------------------------------

fn char_to_u16(s: &str, ci: usize) -> u32 {
    s.chars().take(ci).map(|c| c.len_utf16() as u32).sum()
}

fn u16_to_char(s: &str, ui: u32) -> usize {
    let mut acc = 0u32;
    for (i, c) in s.chars().enumerate() {
        if acc >= ui {
            return i;
        }
        acc += c.len_utf16() as u32;
    }
    s.chars().count()
}

fn hit_index(l: &IDWriteTextLayout, x: f32, y: f32, s: &str) -> usize {
    unsafe {
        let mut trailing = windows::core::BOOL(0);
        let mut inside = windows::core::BOOL(0);
        let mut m = DWRITE_HIT_TEST_METRICS::default();
        if l.HitTestPoint(x, y, &mut trailing, &mut inside, &mut m).is_ok() {
            let pos = m.textPosition + if trailing.as_bool() { m.length } else { 0 };
            return u16_to_char(s, pos);
        }
    }
    s.chars().count()
}

/// (x, y, altura da linha) do cursor.
fn caret_xy(l: &IDWriteTextLayout, s: &str, ci: usize) -> (f32, f32, f32) {
    unsafe {
        let (mut x, mut y) = (0.0f32, 0.0f32);
        let mut m = DWRITE_HIT_TEST_METRICS::default();
        if l.HitTestTextPosition(char_to_u16(s, ci), false, &mut x, &mut y, &mut m).is_ok() {
            return (x, y, m.height);
        }
    }
    (0.0, 0.0, 16.0)
}

fn sel_rects(l: &IDWriteTextLayout, s: &str, a: usize, b: usize) -> Vec<(f32, f32, f32, f32)> {
    let mut out = vec![];
    unsafe {
        let start = char_to_u16(s, a);
        let len = char_to_u16(s, b) - start;
        let mut count = 0u32;
        let _ = l.HitTestTextRange(start, len, 0.0, 0.0, None, &mut count);
        if count == 0 {
            return out;
        }
        let mut v = vec![DWRITE_HIT_TEST_METRICS::default(); count as usize];
        if l.HitTestTextRange(start, len, 0.0, 0.0, Some(&mut v), &mut count).is_ok() {
            for m in v.iter().take(count as usize) {
                out.push((m.left, m.top, m.left + m.width, m.top + m.height));
            }
        }
    }
    out
}

fn remove_range(s: &mut String, a: usize, b: usize) {
    let v: String = s.chars().enumerate().filter(|(i, _)| *i < a || *i >= b).map(|(_, c)| c).collect();
    *s = v;
}

fn insert_at(s: &mut String, at: usize, t: &str) {
    let byte = s.char_indices().nth(at).map(|(b, _)| b).unwrap_or(s.len());
    s.insert_str(byte, t);
}

fn line_start(s: &str, ci: usize) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let mut i = ci.min(chars.len());
    while i > 0 && chars[i - 1] != '\n' {
        i -= 1;
    }
    i
}

fn line_end(s: &str, ci: usize) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let mut i = ci.min(chars.len());
    while i < chars.len() && chars[i] != '\n' {
        i += 1;
    }
    i
}

/// Fundo padrão dos painéis de vidro.
pub fn panel_bg(c: &Canvas, r: Rect, rad: f32, light: bool) {
    let solid = crate::glass::transparent_mode();
    if light {
        c.fill_rrect(r, rad, Color(0.97, 0.97, 0.99, if solid { 0.94 } else { 0.70 }));
    } else {
        c.fill_rrect(r, rad, Color(0.09, 0.09, 0.11, if solid { 0.93 } else { 0.62 }));
    }
    c.fill_rrect_vgrad(r, rad, &[(0.0, Color(1.0, 1.0, 1.0, 0.07)), (0.35, Color(1.0, 1.0, 1.0, 0.0))]);
    c.stroke_rrect(r.inset(0.5), rad, Color(1.0, 1.0, 1.0, 0.16), 1.0);
}
