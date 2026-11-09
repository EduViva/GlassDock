// Arquivos (Downloads ou outra pasta) e Capturas de tela.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use windows::Win32::UI::Shell::{FOLDERID_Downloads, FOLDERID_Screenshots};

use super::*;
use crate::ui::*;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct Cfg {
    folder: String,
    click: u32,
    count_new: bool,
    show: i64,
    last_seen: i64,
    latest_on_tile: bool,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg { folder: String::new(), click: 0, count_new: true, show: 12, last_seen: 0, latest_on_tile: true }
    }
}

#[derive(Clone)]
pub struct Entry {
    pub path: String,
    pub name: String,
    pub mtime: i64,
    pub dir: bool,
}

pub struct Files {
    c: Cfg,
    shots: bool,
    entries: Vec<Entry>,
    menu_target: Option<usize>,
    drag_from: Option<(usize, f32, f32)>,
    scanned: bool,
}

fn is_image(p: &str) -> bool {
    let l = p.to_lowercase();
    [".png", ".jpg", ".jpeg", ".bmp", ".gif", ".webp"].iter().any(|e| l.ends_with(e))
}

pub fn scan_dir(dir: &str, limit: usize, images_only: bool) -> Vec<Entry> {
    let mut v = vec![];
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name.eq_ignore_ascii_case("desktop.ini") || name.ends_with(".crdownload") || name.ends_with(".part") || name.ends_with(".tmp") {
                continue;
            }
            let Ok(md) = e.metadata() else { continue };
            let path = e.path().to_string_lossy().to_string();
            if images_only && !is_image(&path) {
                continue;
            }
            let mtime = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let ctime = md.created().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
            v.push(Entry { path, name, mtime: mtime.max(ctime), dir: md.is_dir() });
        }
    }
    v.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    v.truncate(limit);
    v
}

impl Files {
    pub fn load(v: &Value, shots: bool) -> Files {
        let mut f = Files { c: jget(v), shots, entries: vec![], menu_target: None, drag_from: None, scanned: false };
        if shots && f.c.click == 0 && v.get("click").is_none() {
            f.c.click = 0;
        }
        f.rescan();
        f
    }
    fn dir(&self) -> String {
        if !self.c.folder.is_empty() {
            return self.c.folder.clone();
        }
        let id = if self.shots { &FOLDERID_Screenshots } else { &FOLDERID_Downloads };
        known_folder(id).map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
    }
    fn rescan(&mut self) {
        self.entries = scan_dir(&self.dir(), 60, self.shots);
        self.scanned = true;
    }
    fn new_count(&self) -> usize {
        self.entries.iter().filter(|e| e.mtime > self.c.last_seen).count()
    }
    fn activate(&self, e: &Entry) {
        match self.c.click {
            1 => show_in_explorer(&e.path),
            2 => copy_image_to_clipboard(&e.path),
            _ => shell_open(&e.path),
        }
    }
}

/// Copia uma imagem (como arquivo) para a área de transferência.
fn copy_image_to_clipboard(path: &str) {
    crate::dragdrop::copy_files_to_clipboard(&[path.to_string()]);
}

impl Widget for Files {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn watch_dirs(&self) -> Vec<String> {
        vec![self.dir()]
    }
    fn poll(&mut self, _sh: &mut Shared) -> bool {
        let before: Vec<i64> = self.entries.iter().take(8).map(|e| e.mtime).collect();
        let n = self.entries.len();
        self.rescan();
        let after: Vec<i64> = self.entries.iter().take(8).map(|e| e.mtime).collect();
        before != after || n != self.entries.len()
    }
    fn draw(&mut self, t: &Tile, sh: &mut Shared) {
        if !self.scanned {
            self.rescan();
        }
        let latest = self.entries.first().cloned();
        if self.shots && self.c.latest_on_tile {
            if let Some(e) = &latest {
                if let Some(ic) = sh.icons.thumb(&e.path, (t.s * 2.0) as u32) {
                    t.c.fill_rrect(t.r, t.rad(), Color::rgba(22, 22, 26, 0.9));
                    t.c.bitmap_cover_rounded(&ic.bmp, t.r, t.rad(), 1.0);
                    t.c.stroke_rrect(t.r.inset(0.5), t.rad(), Color(1.0, 1.0, 1.0, 0.18), 1.0);
                    if t.wide() {
                        let lr = Rect::new(t.r.x, t.r.b() - t.s * 0.3, t.r.w, t.s * 0.3);
                        t.c.fill_rect(lr, Color(0.0, 0.0, 0.0, 0.45));
                        t.fit_text(&e.name, lr.inset_xy(t.s * 0.1, 0.0), t.fs(0.12), W_MEDIUM, WHITE, Align::Left);
                    }
                    return;
                }
            }
        }
        t.card();
        let inner = t.inner();
        let icon_path = self.dir();
        let d = if t.wide() { inner.h } else { inner.h.min(inner.w) };
        let ir = if t.wide() { Rect::new(inner.x, inner.y, d, d) } else { inner.centered(d, d) };
        if self.shots {
            t.c.glyph(0xE722, ir, d * 0.55, WHITE.a(0.9));
        } else if let Some(ic) = sh.icons.get(&icon_path, (t.s * 1.2) as u32) {
            t.c.bitmap(&ic.bmp, ir.inset(d * 0.04), 1.0);
        } else {
            t.c.glyph(0xE896, ir, d * 0.5, WHITE);
        }
        if t.wide() {
            let r = Rect::new(inner.x + d + t.s * 0.1, inner.y, inner.w - d - t.s * 0.1, inner.h);
            let title = std::path::Path::new(&icon_path).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            t.fit_text(&title, Rect::new(r.x, r.y, r.w, r.h * 0.5), t.fs(0.15), W_SEMIBOLD, WHITE, Align::Left);
            let sub = latest.map(|e| e.name).unwrap_or_else(|| "Vazia".into());
            t.fit_text(&sub, Rect::new(r.x, r.y + r.h * 0.5, r.w, r.h * 0.42), t.fs(0.125), W_NORMAL, WHITE.a(0.6), Align::Left);
        }
        let n = self.new_count();
        if self.c.count_new && n > 0 {
            let txt = if n > 99 { "99+".to_string() } else { n.to_string() };
            let bh = t.s * 0.3;
            let (tw, _) = t.c.g.measure(&txt, bh * 0.62, W_BOLD);
            let bw = (tw + bh * 0.5).max(bh);
            let br = Rect::new(t.r.r() - bw + t.s * 0.04, t.r.y - t.s * 0.04, bw, bh);
            t.c.fill_rrect(br, bh / 2.0, RED);
            t.c.text(&txt, br, bh * 0.62, W_BOLD, WHITE, Align::Center);
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        if self.entries.is_empty() {
            return (380.0, 80.0);
        }
        let n = self.entries.len().min(self.c.show as usize).max(1);
        let rows = ((n + 3) / 4).min(4) as f32;
        (380.0, 92.0 + rows * 104.0)
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let dir = self.dir();
        let name = std::path::Path::new(&dir).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let r = ui.row(28.0);
        ui.c.text(&name, Rect::new(r.x, r.y, r.w - 150.0 * ui.s, r.h), 15.0 * ui.s, W_SEMIBOLD, TXT, Align::Left);
        if ui.button_k(id("open"), Rect::new(r.r() - 140.0 * ui.s, r.y, 140.0 * ui.s, r.h), "Abrir no Explorador", Btn::Ghost) {
            shell_open(&dir);
            sh.close_panel = true;
        }
        ui.gap(6.0);
        if self.c.last_seen < unix_now() - 1 && self.new_count() > 0 {
            self.c.last_seen = unix_now();
            sh.save = true;
            sh.redraw = true;
        }
        let items: Vec<Entry> = self.entries.iter().take(self.c.show as usize).cloned().collect();
        if items.is_empty() {
            ui.note("Nada por aqui ainda.");
            return;
        }
        let area = Rect::new(ui.area.x, ui.y, ui.area.w, ui.area.b() - ui.y);
        let cw = area.w / 4.0;
        let ch = 104.0 * ui.s;
        let content_h = ((items.len() + 3) / 4) as f32 * ch;
        let off = ui.begin_scroll(id("sc"), area, content_h);
        for (i, e) in items.iter().enumerate() {
            let cell = Rect::new(area.x + cw * (i % 4) as f32, area.y + ch * (i / 4) as f32 - off, cw, ch);
            if cell.b() < area.y || cell.y > area.b() {
                continue;
            }
            let (hov, clk) = ui.hit(idn("f", i as u64), cell);
            if hov {
                ui.c.fill_rrect(cell.inset(2.0), 8.0 * ui.s, FILL);
            }
            let ir = Rect::new(cell.cx() - 30.0 * ui.s, cell.y + 8.0 * ui.s, 60.0 * ui.s, 60.0 * ui.s);
            let img = if is_image(&e.path) { sh.icons.thumb(&e.path, 128) } else { sh.icons.get(&e.path, 64) };
            if let Some(ic) = img {
                if is_image(&e.path) {
                    ui.c.bitmap_cover_rounded(&ic.bmp, ir, 6.0 * ui.s, 1.0);
                } else {
                    ui.c.bitmap(&ic.bmp, ir, 1.0);
                }
            }
            if e.mtime > self.c.last_seen - 1 && self.c.count_new && !self.shots && e.mtime > unix_now() - 86400 {
                ui.c.fill_circle(ir.r(), ir.y + 4.0 * ui.s, 4.0 * ui.s, ACCENT);
            }
            ui.c.text_ex(&e.name, Rect::new(cell.x + 4.0, ir.b() + 4.0 * ui.s, cell.w - 8.0, 30.0 * ui.s), 10.5 * ui.s, W_NORMAL, TXT, Align::Center, VAlign::Top, 0, true);
            if clk {
                self.activate(e);
                sh.close_panel = true;
            }
            if hov && ui.inp.pressed {
                self.drag_from = Some((i, ui.inp.mx, ui.inp.my));
            }
            if ui.right_clicked(cell) {
                self.menu_target = Some(i);
                sh.popup = Some(vec![(1, "Abrir".into()), (2, "Mostrar no Explorador".into()), (4, "Copiar".into()), (0, String::new()), (3, "Mover para a Lixeira".into())]);
            }
        }
        ui.end_scroll();
        // arrastar para fora do painel
        if let Some((i, x0, y0)) = self.drag_from {
            if !ui.inp.down {
                self.drag_from = None;
            } else if (ui.inp.mx - x0).abs() + (ui.inp.my - y0).abs() > 8.0 * ui.s {
                if let Some(e) = items.get(i) {
                    sh.drag_out = Some(vec![e.path.clone()]);
                }
                self.drag_from = None;
                ui.st.active = None;
            }
        }
    }
    fn on_menu(&mut self, idm: u32, sh: &mut Shared) {
        let Some(i) = self.menu_target.take() else { return };
        let Some(e) = self.entries.get(i).cloned() else { return };
        match idm {
            1 => shell_open(&e.path),
            2 => show_in_explorer(&e.path),
            3 => {
                crate::dragdrop::recycle(&[e.path.clone()]);
                self.rescan();
                sh.redraw = true;
            }
            4 => crate::dragdrop::copy_files_to_clipboard(&[e.path.clone()]),
            _ => {}
        }
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let r = ui.row(34.0);
        let lw = 170.0 * ui.s;
        ui.c.text("Pasta", Rect::new(r.x, r.y, lw, r.h), 12.5 * ui.s, W_NORMAL, TXT, Align::Left);
        ui.c.text(&self.dir(), Rect::new(r.x + lw * 0.45, r.y, r.w - lw * 0.45 - 170.0 * ui.s, r.h), 11.5 * ui.s, W_NORMAL, TXT2, Align::Left);
        if ui.button(id("pick"), Rect::new(r.r() - 160.0 * ui.s, r.y + 3.0, 76.0 * ui.s, r.h - 6.0), "Escolher…") {
            sh.pick = Some((sh.current_widget, 1, Pick::Folder));
        }
        if ui.button(id("def"), Rect::new(r.r() - 80.0 * ui.s, r.y + 3.0, 80.0 * ui.s, r.h - 6.0), "Padrão") {
            self.c.folder.clear();
            self.rescan();
            sh.save = true;
        }
        let opts: &[&str] = if self.shots { &["Abre", "Mostra no Explorador", "Copia"] } else { &["Abre", "Mostra no Explorador"] };
        if ui.labeled_segmented(id("click"), "O clique", opts, &mut self.c.click) {
            sh.save = true;
        }
        if self.shots {
            if ui.row_toggle(id("lt"), "A última captura no tile", &mut self.c.latest_on_tile) {
                sh.save = true;
            }
        } else if ui.row_toggle(id("cn"), "Contar os novos no tile", &mut self.c.count_new) {
            sh.save = true;
        }
        if ui.stepper(id("show"), "Quantos mostrar", &mut self.c.show, 6, 60, 2, "") {
            sh.save = true;
        }
    }
    fn on_picked(&mut self, _p: u32, paths: Vec<String>, sh: &mut Shared) {
        if let Some(p) = paths.into_iter().next() {
            self.c.folder = p;
            self.rescan();
            sh.save = true;
            sh.rewatch = true;
        }
    }
}
