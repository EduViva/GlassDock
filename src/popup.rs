// Janelas flutuantes de vidro: painéis, prévias de janelas, avisos,
// ajustes e o conta-gotas.
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::DockPos;
use crate::gfx::*;
use crate::glass;
use crate::ui::*;
use crate::util::*;

/// Base comum: placa de vidro + conteúdo com alfa.
pub struct Popup {
    pub hwnd: HWND,
    pub plate: HWND,
    pub surf: Surface,
    /// retângulo da superfície na tela (inclui margem da sombra)
    pub rect: RECT,
    pub margin: i32,
    pub scale: f32,
    pub inp: Input,
    pub st: UiState,
    pub radius: f32,
    pub content_h: f32,
}

impl Popup {
    pub fn new(kind_plate: u32, kind: u32, idx: usize, activatable: bool, appwindow: bool) -> Popup {
        let plate = glass::create_plate(kind_plate, idx, None);
        let hwnd = if appwindow {
            glass::create(
                kind,
                idx,
                WS_EX_LAYERED | WS_EX_APPWINDOW,
                WS_POPUP | WS_SYSMENU | WS_MINIMIZEBOX,
                Some(plate),
                "GlassDock — Ajustes",
            )
        } else {
            glass::create_layered(kind, idx, Some(plate), activatable)
        };
        Popup { hwnd, plate, surf: Surface::new(), rect: RECT::default(), margin: 0, scale: 1.0, inp: Input::default(), st: UiState::default(), radius: 14.0, content_h: 0.0 }
    }

    /// Posiciona: `inner` é o retângulo do vidro na tela.
    pub fn place(&mut self, inner: RECT) {
        let m = self.margin;
        self.rect = RECT { left: inner.left - m, top: inner.top - m, right: inner.right + m, bottom: inner.bottom + m };
        glass::place_plate(self.plate, inner.left, inner.top, inner.right - inner.left, inner.bottom - inner.top, (self.radius * self.scale) as i32, None);
    }

    pub fn inner_rect(&self) -> RECT {
        let m = self.margin;
        RECT { left: self.rect.left + m, top: self.rect.top + m, right: self.rect.right - m, bottom: self.rect.bottom - m }
    }

    pub fn show(&self) {
        glass::show(self.plate, true);
        glass::show(self.hwnd, true);
    }

    pub fn destroy(&mut self) {
        glass::destroy(self.hwnd);
        glass::destroy(self.plate);
        self.hwnd = HWND::default();
    }

    /// Coordenadas do mouse relativas à superfície.
    pub fn mouse(&mut self, lp: LPARAM) {
        let x = (lp.0 & 0xFFFF) as i16 as f32;
        let y = ((lp.0 >> 16) & 0xFFFF) as i16 as f32;
        self.inp.mx = x;
        self.inp.my = y;
        self.inp.inside = true;
    }

    /// Pinta com o fundo de painel e entrega um `Ui` para o conteúdo.
    /// Repete o quadro uma vez se algo mudou (modo imediato).
    pub fn paint_ui<F: FnMut(&mut Ui)>(&mut self, light: bool, mut f: F) -> (bool, bool) {
        let w = self.rect.right - self.rect.left;
        let h = self.rect.bottom - self.rect.top;
        let m = self.margin as f32;
        let sc = self.scale;
        let rad = self.radius * sc;
        let mut any_changed = false;
        let mut close = false;
        for pass in 0..2 {
            let mut changed = false;
            let inp = if pass == 0 { self.inp.clone() } else { Input { mx: self.inp.mx, my: self.inp.my, inside: self.inp.inside, down: self.inp.down, ..Default::default() } };
            let st = &mut self.st;
            let mut content_h = self.content_h;
            self.surf.paint(self.hwnd, self.rect.left, self.rect.top, w, h, |c| {
                let inner = Rect::new(m, m, w as f32 - 2.0 * m, h as f32 - 2.0 * m);
                c.shadow(inner.inset(6.0 * sc), rad, 18.0 * sc, 10.0 * sc, Color(0.0, 0.0, 0.0, 0.35));
                panel_bg(c, inner, rad, light);
                let area = inner.inset(16.0 * sc);
                let mut ui = Ui::new(c, &inp, st, sc, area);
                f(&mut ui);
                changed = ui.changed;
                close |= ui.close;
                content_h = ui.y - area.y;
            });
            self.content_h = content_h;
            any_changed |= changed;
            if !changed {
                break;
            }
        }
        self.inp.clear_events();
        (any_changed, close)
    }
}

/// Calcula onde um painel aparece em relação ao item da dock.
pub fn place_near(anchor: RECT, w: i32, h: i32, pos: DockPos, work: RECT, gap: i32) -> RECT {
    let cx = (anchor.left + anchor.right) / 2;
    let cy = (anchor.top + anchor.bottom) / 2;
    let (mut x, mut y) = match pos {
        DockPos::Bottom => (cx - w / 2, anchor.top - gap - h),
        DockPos::Top => (cx - w / 2, anchor.bottom + gap),
        DockPos::Left => (anchor.right + gap, cy - h / 2),
        DockPos::Right => (anchor.left - gap - w, cy - h / 2),
    };
    x = x.clamp(work.left + 8, (work.right - w - 8).max(work.left + 8));
    y = y.clamp(work.top + 8, (work.bottom - h - 8).max(work.top + 8));
    RECT { left: x, top: y, right: x + w, bottom: y + h }
}

// ---------------------------------------------------------------------------
// Prévias de janelas (miniaturas ao vivo do DWM)
// ---------------------------------------------------------------------------

pub struct PItem {
    pub hwnd: HWND,
    pub thumb: isize,
    pub rect: Rect,
    pub title: String,
    pub minimized: bool,
}

pub struct Preview {
    pub p: Popup,
    pub items: Vec<PItem>,
    pub hover: Option<usize>,
    pub dock: usize,
    pub key: String,
    pub icon: String,
}

impl Preview {
    pub fn new(dock: usize, key: String, icon: String, wins: &[HWND], anchor: RECT, pos: DockPos, work: RECT, dpi: u32) -> Preview {
        let mut p = Popup::new(glass::K_PREVIEW_PLATE, glass::K_PREVIEW, 0, false, false);
        let sc = scale(dpi);
        p.scale = sc;
        p.margin = (18.0 * sc) as i32;
        p.radius = 12.0;
        let maxw = 220.0 * sc;
        let maxh = 140.0 * sc;
        let title_h = 26.0 * sc;
        let pad = 10.0 * sc;
        let vertical = matches!(pos, DockPos::Left | DockPos::Right);
        let mut items = vec![];
        let mut sizes = vec![];
        for &h in wins.iter().take(8) {
            let minimized = crate::winlist::is_minimized(h);
            let mut size = (maxw, maxh * 0.7);
            let mut thumb = 0isize;
            if !minimized {
                unsafe {
                    if let Ok(t) = DwmRegisterThumbnail(p.plate, h) {
                        thumb = t;
                        if let Ok(sz) = DwmQueryThumbnailSourceSize(t) {
                            if sz.cx > 0 && sz.cy > 0 {
                                let k = (maxw / sz.cx as f32).min(maxh / sz.cy as f32).min(1.0);
                                size = (sz.cx as f32 * k, sz.cy as f32 * k);
                            }
                        }
                    }
                }
            }
            sizes.push(size);
            items.push(PItem { hwnd: h, thumb, rect: Rect::default(), title: crate::winlist::window_title(h), minimized });
        }
        let m = p.margin as f32;
        let (w, h) = if vertical {
            let w = sizes.iter().map(|s| s.0).fold(0.0f32, f32::max).max(160.0 * sc) + 2.0 * pad;
            let h: f32 = sizes.iter().map(|s| s.1 + title_h + pad).sum::<f32>() + pad;
            (w, h)
        } else {
            let w: f32 = sizes.iter().map(|s| s.0.max(140.0 * sc) + pad).sum::<f32>() + pad;
            let h = sizes.iter().map(|s| s.1).fold(0.0f32, f32::max) + title_h + 2.0 * pad;
            (w, h)
        };
        let mut x = m + pad;
        let mut y = m + pad;
        let row_h = sizes.iter().map(|s| s.1).fold(0.0f32, f32::max);
        for (i, it) in items.iter_mut().enumerate() {
            let (sw, shh) = sizes[i];
            if vertical {
                it.rect = Rect::new(m + (w - sw) / 2.0, y + title_h, sw, shh);
                y += shh + title_h + pad;
            } else {
                let cell_w = sw.max(140.0 * sc);
                it.rect = Rect::new(x + (cell_w - sw) / 2.0, y + title_h + (row_h - shh) / 2.0, sw, shh);
                x += cell_w + pad;
            }
        }
        let inner = place_near(anchor, w as i32, h as i32, pos, work, (6.0 * sc) as i32);
        p.place(inner);
        let pv = Preview { p, items, hover: None, dock, key, icon };
        pv.update_thumbs();
        pv
    }

    pub fn update_thumbs(&self) {
        let m = self.p.margin as f32;
        for it in &self.items {
            if it.thumb == 0 {
                continue;
            }
            let r = it.rect;
            let props = DWM_THUMBNAIL_PROPERTIES {
                dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_OPACITY | DWM_TNP_SOURCECLIENTAREAONLY,
                rcDestination: RECT { left: (r.x - m) as i32, top: (r.y - m) as i32, right: (r.r() - m) as i32, bottom: (r.b() - m) as i32 },
                rcSource: RECT::default(),
                opacity: 255,
                fVisible: true.into(),
                fSourceClientAreaOnly: false.into(),
            };
            unsafe {
                let _ = DwmUpdateThumbnailProperties(it.thumb, &props);
            }
        }
    }

    pub fn item_at(&self, x: f32, y: f32) -> Option<usize> {
        let sc = self.p.scale;
        self.items.iter().position(|it| Rect::new(it.rect.x - 4.0 * sc, it.rect.y - 26.0 * sc, it.rect.w.max(140.0 * sc) + 8.0 * sc, it.rect.h + 30.0 * sc).contains(x, y))
    }

    pub fn close_rect(&self, i: usize) -> Rect {
        let sc = self.p.scale;
        let it = &self.items[i];
        let r = it.rect;
        Rect::new(r.r() - 22.0 * sc, r.y - 25.0 * sc, 22.0 * sc, 22.0 * sc)
    }

    pub fn render(&mut self, icons: &mut crate::icons::IconCache) {
        let w = self.p.rect.right - self.p.rect.left;
        let h = self.p.rect.bottom - self.p.rect.top;
        let m = self.p.margin as f32;
        let sc = self.p.scale;
        let rad = self.p.radius * sc;
        let hover = self.hover;
        let items: Vec<(Rect, String, bool, bool)> = self.items.iter().map(|i| (i.rect, i.title.clone(), i.minimized, i.thumb != 0)).collect();
        let icon = icons.get(&self.icon, (48.0 * sc) as u32);
        let closes: Vec<Rect> = (0..self.items.len()).map(|i| self.close_rect(i)).collect();
        let (mx, my) = (self.p.inp.mx, self.p.inp.my);
        self.p.surf.paint(self.p.hwnd, self.p.rect.left, self.p.rect.top, w, h, |c| {
            let inner = Rect::new(m, m, w as f32 - 2.0 * m, h as f32 - 2.0 * m);
            c.shadow(inner.inset(6.0 * sc), rad, 16.0 * sc, 8.0 * sc, Color(0.0, 0.0, 0.0, 0.32));
            let holes: Vec<Rect> = items.iter().filter(|x| x.3).map(|x| x.0).collect();
            c.fill_rrect_holes(inner, rad, &holes, Color(0.09, 0.09, 0.11, 0.62));
            c.stroke_rrect(inner.inset(0.5), rad, Color(1.0, 1.0, 1.0, 0.16), 1.0);
            for h in &holes {
                c.fill_rect(*h, Color(0.0, 0.0, 0.0, 1.0 / 255.0));
            }
            for (i, (r, title, minimized, has_thumb)) in items.iter().enumerate() {
                let cell = Rect::new(r.x - 4.0 * sc, r.y - 26.0 * sc, r.w.max(140.0 * sc) + 8.0 * sc, r.h + 30.0 * sc);
                let cell = Rect::new(cell.x.min(r.x - 4.0 * sc), cell.y, cell.w, cell.h);
                if hover == Some(i) {
                    c.stroke_rrect(Rect::new(r.x - 3.0 * sc, r.y - 3.0 * sc, r.w + 6.0 * sc, r.h + 6.0 * sc), 6.0 * sc, Color(1.0, 1.0, 1.0, 0.55), 1.5 * sc);
                }
                let tr = Rect::new(r.x, r.y - 25.0 * sc, r.w.max(120.0 * sc) - 24.0 * sc, 22.0 * sc);
                if let Some(ic) = &icon {
                    c.bitmap(&ic.bmp, Rect::new(tr.x, tr.y + 3.0 * sc, 16.0 * sc, 16.0 * sc), 1.0);
                }
                c.text(title, Rect::new(tr.x + 22.0 * sc, tr.y, tr.w - 22.0 * sc, tr.h), 12.0 * sc, W_MEDIUM, WHITE.a(0.92), Align::Left);
                if !has_thumb {
                    c.fill_rrect(*r, 8.0 * sc, Color(1.0, 1.0, 1.0, 0.06));
                    if let Some(ic) = &icon {
                        c.bitmap(&ic.bmp, r.centered(48.0 * sc, 48.0 * sc), if *minimized { 0.75 } else { 1.0 });
                    }
                    if *minimized {
                        c.text("Minimizada", Rect::new(r.x, r.b() - 22.0 * sc, r.w, 18.0 * sc), 11.0 * sc, W_NORMAL, WHITE.a(0.55), Align::Center);
                    }
                }
                if hover == Some(i) {
                    let cr = closes[i];
                    let over = cr.contains(mx, my);
                    c.fill_circle(cr.cx(), cr.cy(), cr.w / 2.0, if over { RED } else { Color(1.0, 1.0, 1.0, 0.18) });
                    c.glyph(0xE8BB, cr, 8.5 * sc, WHITE);
                }
                let _ = cell;
            }
        });
    }

    pub fn destroy(&mut self) {
        for it in &self.items {
            if it.thumb != 0 {
                unsafe {
                    let _ = DwmUnregisterThumbnail(it.thumb);
                }
            }
        }
        self.p.destroy();
    }
}

// ---------------------------------------------------------------------------
// Avisos (timer, pomodoro…)
// ---------------------------------------------------------------------------

pub struct ToastWin {
    pub p: Popup,
    pub title: String,
    pub body: String,
    pub until: u64,
}

impl ToastWin {
    pub fn new(idx: usize, title: &str, body: &str, work: RECT, dpi: u32, stack: usize, sound: bool) -> ToastWin {
        let mut p = Popup::new(glass::K_PANEL_PLATE, glass::K_TOAST, 1000 + idx, false, false);
        let sc = scale(dpi);
        p.scale = sc;
        p.margin = (18.0 * sc) as i32;
        let w = (340.0 * sc) as i32;
        let h = (78.0 * sc) as i32;
        let x = work.right - w - (16.0 * sc) as i32;
        let y = work.bottom - h - (16.0 * sc) as i32 - stack as i32 * (h + (10.0 * sc) as i32);
        p.place(RECT { left: x, top: y, right: x + w, bottom: y + h });
        if sound {
            unsafe {
                let _ = windows::Win32::Media::Audio::PlaySoundW(windows::core::w!("SystemNotification"), None, windows::Win32::Media::Audio::SND_ALIAS | windows::Win32::Media::Audio::SND_ASYNC);
            }
        }
        ToastWin { p, title: title.into(), body: body.into(), until: tick_ms() + 7000 }
    }

    pub fn render(&mut self) {
        let t = self.title.clone();
        let b = self.body.clone();
        self.p.paint_ui(false, |ui| {
            let r = ui.area;
            ui.c.fill_circle(r.x + 20.0 * ui.s, r.cy(), 20.0 * ui.s, ORANGE.a(0.9));
            ui.c.glyph(0xEA8F, Rect::new(r.x, r.cy() - 20.0 * ui.s, 40.0 * ui.s, 40.0 * ui.s), 16.0 * ui.s, WHITE);
            let tr = Rect::new(r.x + 54.0 * ui.s, r.y, r.w - 54.0 * ui.s, r.h);
            ui.c.text(&t, Rect::new(tr.x, tr.y + 2.0, tr.w, tr.h * 0.5), 13.5 * ui.s, W_SEMIBOLD, TXT, Align::Left);
            ui.c.text(&b, Rect::new(tr.x, tr.y + tr.h * 0.5, tr.w, tr.h * 0.45), 12.0 * ui.s, W_NORMAL, TXT2, Align::Left);
        });
    }
}

// ---------------------------------------------------------------------------
// Conta-gotas: captura a tela uma vez e mostra uma lupa que segue o mouse
// ---------------------------------------------------------------------------

pub struct Eyedrop {
    pub overlay: HWND,
    pub lens: HWND,
    pub surf: Surface,
    pub widget: u32,
    pub bits: Vec<u8>,
    pub vx: i32,
    pub vy: i32,
    pub vw: i32,
    pub vh: i32,
}

impl Eyedrop {
    pub fn start(widget: u32) -> Option<Eyedrop> {
        unsafe {
            let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: vw,
                    biHeight: -vh,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut ptr = std::ptr::null_mut();
            let bmp = CreateDIBSection(Some(mem), &bi, DIB_RGB_COLORS, &mut ptr, None, 0).ok()?;
            let old = SelectObject(mem, bmp.into());
            let _ = BitBlt(mem, 0, 0, vw, vh, Some(screen), vx, vy, SRCCOPY | CAPTUREBLT);
            let bits = std::slice::from_raw_parts(ptr as *const u8, (vw * vh * 4) as usize).to_vec();
            SelectObject(mem, old);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            let overlay = glass::create(glass::K_EYEDROP, 0, WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST, WS_POPUP, None, "GlassDockEyedrop");
            let _ = SetLayeredWindowAttributes(overlay, COLORREF(0), 1, LWA_ALPHA);
            let _ = SetWindowPos(overlay, Some(HWND_TOPMOST), vx, vy, vw, vh, SWP_SHOWWINDOW);
            let lens = glass::create_layered(glass::K_MAGNIFIER, 0, Some(overlay), false);
            unsafe {
                let ex = GetWindowLongW(lens, GWL_EXSTYLE);
                SetWindowLongW(lens, GWL_EXSTYLE, ex | 0x20);
            }
            let _ = SetForegroundWindow(overlay);
            let _ = SetCursor(LoadCursorW(None, IDC_CROSS).ok());
            Some(Eyedrop { overlay, lens, surf: Surface::new(), widget, bits, vx, vy, vw, vh })
        }
    }

    pub fn pixel(&self, sx: i32, sy: i32) -> (u8, u8, u8) {
        let x = (sx - self.vx).clamp(0, self.vw - 1);
        let y = (sy - self.vy).clamp(0, self.vh - 1);
        let i = ((y * self.vw + x) * 4) as usize;
        (self.bits[i + 2], self.bits[i + 1], self.bits[i])
    }

    pub fn update(&mut self, sx: i32, sy: i32) {
        let dpi = unsafe { glass::monitor_dpi(MonitorFromPoint(POINT { x: sx, y: sy }, MONITOR_DEFAULTTONEAREST)) };
        let sc = scale(dpi);
        let size = (150.0 * sc) as i32;
        let label = (30.0 * sc) as i32;
        let n = 11i32;
        let cell = size as f32 / n as f32;
        let mut px = vec![];
        for j in 0..n {
            for i in 0..n {
                px.push(self.pixel(sx + i - n / 2, sy + j - n / 2));
            }
        }
        let (r, g, b) = self.pixel(sx, sy);
        let hex = format!("#{:02X}{:02X}{:02X}", r, g, b);
        let x = sx + (24.0 * sc) as i32;
        let y = sy + (24.0 * sc) as i32;
        let total_h = size + label + 6;
        self.surf.paint(self.lens, x, y, size + 4, total_h + 4, |c| {
            let circle = Rect::new(2.0, 2.0, size as f32, size as f32);
            c.push_rrect_clip(circle, size as f32 / 2.0);
            for j in 0..n {
                for i in 0..n {
                    let (pr, pg, pb) = px[(j * n + i) as usize];
                    c.fill_rect(Rect::new(2.0 + i as f32 * cell, 2.0 + j as f32 * cell, cell + 0.5, cell + 0.5), Color::rgba(pr, pg, pb, 1.0));
                }
            }
            c.pop_clip();
            let mid = Rect::new(2.0 + (n / 2) as f32 * cell, 2.0 + (n / 2) as f32 * cell, cell, cell);
            c.stroke_rrect(mid, 2.0, WHITE, 2.0);
            c.stroke_rrect(mid.inset(-1.5), 2.0, BLACK.a(0.6), 1.0);
            c.stroke_circle(circle.cx(), circle.cy(), size as f32 / 2.0 - 1.0, WHITE.a(0.9), 2.5);
            let lr = Rect::new(circle.cx() - 55.0 * sc, size as f32 + 8.0, 110.0 * sc, label as f32 - 4.0);
            c.fill_rrect(lr, lr.h / 2.0, Color::rgba(28, 28, 32, 0.92));
            c.fill_circle(lr.x + lr.h / 2.0 + 2.0, lr.cy(), lr.h * 0.3, Color::rgba(r, g, b, 1.0));
            c.text(&hex, lr.inset_xy(10.0 * sc, 0.0).offset(8.0 * sc, 0.0), 12.5 * sc, W_SEMIBOLD, WHITE, Align::Center);
        });
        glass::show(self.lens, true);
    }

    pub fn destroy(&mut self) {
        glass::destroy(self.lens);
        glass::destroy(self.overlay);
    }
}
