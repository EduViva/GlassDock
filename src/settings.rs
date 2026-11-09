// Janela de Ajustes.
use windows::Win32::Foundation::*;

use crate::app::App;
use crate::config::*;
use crate::gfx::*;
use crate::glass;
use crate::popup::Popup;
use crate::ui::*;
use crate::util::*;
use crate::widgets::{self, CATALOG};

#[derive(Clone, Copy, PartialEq)]
pub enum Page {
    General,
    Appearance,
    Workspaces,
    Widgets,
    Bar,
    About,
    EditWidget(u32),
}

pub struct SettingsWin {
    pub p: Popup,
    pub page: Page,
    pub ws_sel: usize,
    pub search: String,
    pub app_search: String,
    pub content_h: f32,
}

const PAGES: [(Page, &str, u32); 6] = [
    (Page::General, "Geral", 0xE713),
    (Page::Appearance, "Aparência", 0xE790),
    (Page::Workspaces, "Workspaces", 0xE8A9),
    (Page::Widgets, "Widgets", 0xF0E2),
    (Page::Bar, "Barra e bandeja", 0xE7F4),
    (Page::About, "Sobre", 0xE946),
];

fn kind_glyph(kind: &str) -> u32 {
    match kind {
        "clock" => 0xE823,
        "timer" => 0xE916,
        "pomodoro" => 0xE7C1,
        "countdown" => 0xE787,
        "progress" => 0xE9D9,
        "calendar" => 0xE787,
        "tasks" => 0xE73A,
        "note" => 0xE70B,
        "music" => 0xEC4F,
        "battery" => 0xE83F,
        "activity" => 0xE9D9,
        "network" => 0xE968,
        "keyboard" => 0xE765,
        "eyedropper" => 0xEF3C,
        "screenshots" => 0xE722,
        "water" => 0xEB42,
        "pet" => 0xED54,
        "claude" | "codex" | "opencode" => 0xE99A,
        "weather" => 0xE753,
        "collection" => 0xE71D,
        "favorites" => 0xE734,
        "files" => 0xE8B7,
        _ => 0xF0E2,
    }
}

impl SettingsWin {
    pub fn new(dpi: u32, work: RECT) -> SettingsWin {
        let mut p = Popup::new(glass::K_SETTINGS_PLATE, glass::K_SETTINGS, 0, true, true);
        p.scale = scale(dpi);
        p.margin = (24.0 * p.scale) as i32;
        p.radius = 16.0;
        let w = (860.0 * p.scale) as i32;
        let h = (640.0 * p.scale).min((work.bottom - work.top) as f32 * 0.92) as i32;
        let x = (work.left + work.right - w) / 2;
        let y = (work.top + work.bottom - h) / 2;
        p.place(RECT { left: x, top: y, right: x + w, bottom: y + h });
        SettingsWin { p, page: Page::General, ws_sel: 0, search: String::new(), app_search: String::new(), content_h: 0.0 }
    }

    pub fn in_titlebar(&self, x: f32, y: f32) -> bool {
        let m = self.p.margin as f32;
        let sc = self.p.scale;
        let w = (self.p.rect.right - self.p.rect.left) as f32;
        y >= m && y < m + 46.0 * sc && x >= m && x < w - m - 50.0 * sc
    }

    pub fn render(&mut self, app: &mut App) {
        let SettingsWin { p, page, ws_sel, search, app_search, content_h } = self;
        let mut close = false;
        let ch_prev = *content_h;
        let mut ch_new = ch_prev;
        p.paint_ui(false, |ui| {
            let full = ui.area;
            // barra de título
            let tb = Rect::new(full.x, full.y, full.w, 30.0 * ui.s);
            ui.c.glyph(0xE8A9, Rect::new(tb.x, tb.y, 24.0 * ui.s, tb.h), 15.0 * ui.s, ACCENT);
            ui.c.text("Ajustes do GlassDock", Rect::new(tb.x + 30.0 * ui.s, tb.y, tb.w * 0.6, tb.h), 15.0 * ui.s, W_SEMIBOLD, TXT, Align::Left);
            if ui.icon_btn(id("close"), Rect::new(tb.r() - 30.0 * ui.s, tb.y, 30.0 * ui.s, 30.0 * ui.s), 0xE8BB, 11.0, TXT) {
                close = true;
            }
            let body_y = tb.b() + 12.0 * ui.s;
            // barra lateral
            let side = Rect::new(full.x, body_y, 180.0 * ui.s, full.b() - body_y);
            let mut y = side.y;
            for (pg, name, gl) in PAGES {
                let r = Rect::new(side.x, y, side.w - 10.0 * ui.s, 36.0 * ui.s);
                let sel = *page == pg || (pg == Page::Widgets && matches!(*page, Page::EditWidget(_)));
                let (hov, clk) = ui.hit(idn("pg", gl as u64), r);
                if sel {
                    ui.c.fill_rrect(r, 8.0 * ui.s, Color(1.0, 1.0, 1.0, 0.14));
                } else if hov {
                    ui.c.fill_rrect(r, 8.0 * ui.s, FILL);
                }
                ui.c.glyph(gl, Rect::new(r.x + 8.0 * ui.s, r.y, 22.0 * ui.s, r.h), 13.0 * ui.s, if sel { ACCENT } else { TXT2 });
                ui.c.text(name, Rect::new(r.x + 38.0 * ui.s, r.y, r.w - 40.0 * ui.s, r.h), 13.0 * ui.s, if sel { W_SEMIBOLD } else { W_NORMAL }, TXT, Align::Left);
                if clk {
                    *page = pg;
                    ui.st.scroll.insert(id("content"), 0.0);
                }
                y += 40.0 * ui.s;
            }
            // conteúdo rolável
            let content = Rect::new(side.r() + 14.0 * ui.s, body_y, full.r() - side.r() - 14.0 * ui.s, full.b() - body_y);
            let off = ui.begin_scroll(id("content"), content, ch_prev);
            let saved_area = ui.area;
            ui.area = Rect::new(content.x, content.y - off, content.w - 14.0 * ui.s, 100000.0);
            ui.y = content.y - off;
            let start = ui.y;
            match *page {
                Page::General => page_general(app, ui),
                Page::Appearance => page_appearance(app, ui),
                Page::Workspaces => page_workspaces(app, ui, ws_sel, app_search),
                Page::Widgets => page_widgets(app, ui, search, page),
                Page::Bar => page_bar(app, ui),
                Page::About => page_about(app, ui),
                Page::EditWidget(wid) => page_edit_widget(app, ui, wid, page),
            }
            ch_new = ui.y - start + 20.0 * ui.s;
            ui.area = saved_area;
            ui.end_scroll();
        });
        *content_h = ch_new;
        if close {
            app.sh.close_panel = false;
            crate::app::defer(|| {
                crate::app::with_app(|a| a.close_settings());
            });
        }
    }
}

fn page_title(ui: &mut Ui, t: &str, sub: &str) {
    let r = ui.row(30.0);
    ui.c.text(t, r, 19.0 * ui.s, W_SEMIBOLD, TXT, Align::Left);
    if !sub.is_empty() {
        ui.note(sub);
    }
    ui.gap(6.0);
}

fn section(ui: &mut Ui, t: &str) {
    ui.gap(8.0);
    ui.caption(t);
}

fn page_general(app: &mut App, ui: &mut Ui) {
    page_title(ui, "Geral", "Como a dock convive com a barra de tarefas do Windows.");
    let g = &mut app.cfg.general;
    let mut ch_general = false;
    let mut recreate = false;
    section(ui, "BARRA DE TAREFAS DO WINDOWS");
    let mut mode = match g.taskbar {
        TaskbarMode::Hide => 0,
        TaskbarMode::AutoHide => 1,
        TaskbarMode::Keep => 2,
    };
    let r = ui.row(34.0);
    if ui.segmented(id("tb"), Rect::new(r.x, r.y + 2.0, r.w, r.h - 4.0), &["Ocultar por completo", "Ocultar automaticamente", "Manter visível"], &mut mode) {
        g.taskbar = [TaskbarMode::Hide, TaskbarMode::AutoHide, TaskbarMode::Keep][mode as usize];
        ch_general = true;
    }
    ui.note("Ao sair do GlassDock, a barra volta exatamente como estava, mesmo se ele fechar de repente.");
    ch_general |= ui.row_toggle(id("auto"), "Abrir ao iniciar o Windows", &mut g.start_with_windows);
    section(ui, "DOCK");
    ch_general |= ui.row_toggle(id("ah"), "Ocultar e mostrar automaticamente", &mut g.autohide);
    ch_general |= ui.row_toggle(id("rs"), "Manter as janelas fora da dock (maximizadas param acima)", &mut g.reserve_space);
    ch_general |= ui.row_toggle(id("fs"), "Esconder em vídeos, jogos e apps em tela cheia", &mut g.hide_in_fullscreen);
    if ui.row_toggle(id("am"), "Uma dock em cada monitor", &mut g.all_monitors) {
        recreate = true;
    }
    if g.all_monitors {
        ch_general |= ui.row_toggle(id("afm"), "Mostrar apps abertos de todos os monitores", &mut g.apps_from_all_monitors);
    } else if ui.row_toggle(id("fp"), "A dock segue o ponteiro para outro monitor", &mut g.follow_pointer) {
        recreate = true;
    }
    section(ui, "O QUE APARECE NA DOCK");
    ch_general |= ui.row_toggle(id("sr"), "Mostrar apps abertos", &mut g.show_running);
    ch_general |= ui.row_toggle(id("oc"), "Só apps do workspace atual", &mut g.only_current_ws);
    ch_general |= ui.row_toggle(id("wsel"), "Seletor de workspace (bolinhas)", &mut g.ws_selector);
    ch_general |= ui.row_toggle(id("dl"), "Mostrar Downloads", &mut g.show_downloads);
    ch_general |= ui.row_toggle(id("tr"), "Mostrar Lixeira", &mut g.show_trash);
    section(ui, "PRÉVIAS DE JANELAS");
    ch_general |= ui.row_toggle(id("pv"), "Mostrar prévias ao parar o mouse num app", &mut g.previews);
    let mut delay = g.preview_delay_ms as f32;
    if ui.labeled_slider(id("pvd"), "Atraso", &mut delay, 0.0, 2000.0, "{} ms") {
        g.preview_delay_ms = (delay / 50.0).round() as u32 * 50;
        ch_general = true;
    }
    ch_general |= ui.row_toggle(id("pvm"), "Só quando o app tem 2 ou mais janelas", &mut g.previews_only_multi);
    section(ui, "ATALHOS");
    ch_general |= ui.row_toggle(id("hk"), "Ctrl+Alt+1 a 9 troca de workspace", &mut g.ws_hotkeys);
    ch_general |= ui.row_toggle(id("fk"), "Ctrl+Alt+D leva o foco para a dock (setas, Enter, Esc)", &mut g.focus_hotkey);
    ch_general |= ui.row_toggle(id("wh"), "A roda do mouse sobre a dock troca de workspace", &mut g.wheel_switches_ws);
    if recreate {
        app.recreate_docks();
    } else if ch_general {
        app.apply_general();
    }
}

fn page_appearance(app: &mut App, ui: &mut Ui) {
    page_title(ui, "Aparência", "O vidro e os ícones.");
    let a = &mut app.cfg.appearance;
    let mut ch = false;
    let mut reposition = false;
    let mut pos = match app.cfg.general.position {
        DockPos::Bottom => 0,
        DockPos::Top => 1,
        DockPos::Left => 2,
        DockPos::Right => 3,
    };
    if ui.labeled_segmented(id("pos"), "Posição", &["Embaixo", "Em cima", "Esquerda", "Direita"], &mut pos) {
        app.cfg.general.position = [DockPos::Bottom, DockPos::Top, DockPos::Left, DockPos::Right][pos as usize];
        reposition = true;
    }
    if ui.labeled_slider(id("size"), "Tamanho dos ícones", &mut a.icon_size, 28.0, 96.0, "{} pt") {
        a.icon_size = a.icon_size.round();
        ch = true;
    }
    let mut hv = match a.hover {
        Hover::None => 0,
        Hover::Scale => 1,
        Hover::Magnify => 2,
    };
    if ui.labeled_segmented(id("hover"), "Ao passar o mouse", &["Nada", "Escala", "Ampliação"], &mut hv) {
        a.hover = [Hover::None, Hover::Scale, Hover::Magnify][hv as usize];
        ch = true;
    }
    if a.hover == Hover::Magnify {
        let mut m = a.magnify * 100.0;
        if ui.labeled_slider(id("mag"), "Ampliação", &mut m, 110.0, 200.0, "{}%") {
            a.magnify = (m / 5.0).round() * 5.0 / 100.0;
            ch = true;
        }
    }
    let mut la = match a.launch_anim {
        LaunchAnim::Bounce => 0,
        LaunchAnim::Pulse => 1,
        LaunchAnim::Shake => 2,
    };
    if ui.labeled_segmented(id("la"), "Animação de abertura", &["Pulo", "Pulso", "Tremor"], &mut la) {
        a.launch_anim = [LaunchAnim::Bounce, LaunchAnim::Pulse, LaunchAnim::Shake][la as usize];
        ch = true;
    }
    let mut ind = if a.indicator == Indicator::Dot { 0 } else { 1 };
    if ui.labeled_segmented(id("ind"), "Indicador de aberto", &["Ponto", "Traço"], &mut ind) {
        a.indicator = if ind == 0 { Indicator::Dot } else { Indicator::Dash };
        ch = true;
    }
    ch |= ui.row_toggle(id("indc"), "Indicador com a cor do workspace", &mut a.indicator_ws_color);
    ch |= ui.row_toggle(id("plates"), "Ícones que não são quadrados ganham uma placa arredondada", &mut a.plates);
    ch |= ui.row_toggle(id("labels"), "Mostrar o nome ao passar o mouse", &mut a.labels);
    section(ui, "VIDRO");
    let mut t = match a.tint {
        Tint::Dark => 0,
        Tint::Light => 1,
        Tint::Auto => 2,
    };
    if ui.labeled_segmented(id("tint"), "Tom", &["Escuro", "Claro", "Seguir o Windows"], &mut t) {
        a.tint = [Tint::Dark, Tint::Light, Tint::Auto][t as usize];
        ch = true;
    }
    let mut op = a.opacity * 100.0;
    if ui.labeled_slider(id("op"), "Opacidade da tinta", &mut op, 10.0, 95.0, "{}%") {
        a.opacity = op.round() / 100.0;
        ch = true;
    }
    ui.note("O desfoque é o do próprio Windows: não custa CPU do GlassDock.");
    if reposition {
        app.recreate_docks();
    } else if ch {
        app.sh.icons.clear();
        app.mark_save();
        for i in 0..app.docks.len() {
            app.setup_appbar_pub(i);
        }
        app.rebuild_all();
    }
}

fn page_workspaces(app: &mut App, ui: &mut Ui, sel: &mut usize, app_search: &mut String) {
    page_title(ui, "Workspaces", "Cada workspace tem os seus apps fixados, widgets e janelas. Troque com Ctrl+Alt+número, a roda do mouse ou as bolinhas.");
    let n = app.cfg.workspaces.len();
    if *sel >= n {
        *sel = 0;
    }
    // abas
    let r = ui.row(36.0);
    let mut x = r.x;
    for (k, ws) in app.cfg.workspaces.clone().iter().enumerate() {
        let (tw, _) = ui.c.g.measure(&ws.name, 12.5 * ui.s, W_MEDIUM);
        let w = tw + 40.0 * ui.s;
        let br = Rect::new(x, r.y + 2.0, w, r.h - 4.0);
        let (hov, clk) = ui.hit(idn("wst", k as u64), br);
        ui.c.fill_rrect(br, br.h / 2.0, if *sel == k { Color(1.0, 1.0, 1.0, 0.18) } else if hov { FILL_HOVER } else { FILL });
        ui.c.fill_circle(br.x + 14.0 * ui.s, br.cy(), 5.0 * ui.s, Color::hex(WS_COLORS[ws.color as usize % 6].0));
        ui.c.text(&ws.name, Rect::new(br.x + 24.0 * ui.s, br.y, br.w - 28.0 * ui.s, br.h), 12.5 * ui.s, W_MEDIUM, TXT, Align::Left);
        if clk {
            *sel = k;
        }
        x += w + 6.0 * ui.s;
    }
    let addr = Rect::new(x, r.y + 2.0, 34.0 * ui.s, r.h - 4.0);
    if ui.icon_btn(id("wsadd"), addr, 0xE710, 11.0, TXT) {
        let id_new = app.cfg.workspaces.iter().map(|w| w.id).max().unwrap_or(0) + 1;
        let mut items = vec![DockItem::Start];
        items.extend(crate::icons::default_pins().into_iter().map(DockItem::App));
        app.cfg.workspaces.push(Workspace { id: id_new, name: format!("Workspace {}", n + 1), color: (n % 6) as u32, items });
        *sel = app.cfg.workspaces.len() - 1;
        app.after_ws_change();
        return;
    }
    ui.gap(8.0);
    let wsid = app.cfg.workspaces[*sel].id;
    let mut changed = false;
    {
        let ws = &mut app.cfg.workspaces[*sel];
        let res = ui.labeled_input(idn("wsname", wsid as u64), "Nome", &mut ws.name, "Trabalho, Casa…");
        changed |= res.changed;
        let r = ui.row(34.0);
        ui.c.text("Cor", Rect::new(r.x, r.y, 170.0 * ui.s, r.h), 12.5 * ui.s, W_NORMAL, TXT, Align::Left);
        let cols: Vec<Color> = WS_COLORS.iter().map(|c| Color::hex(c.0)).collect();
        changed |= ui.swatches(idn("wscol", wsid as u64), Rect::new(r.x + 170.0 * ui.s, r.y, r.w - 170.0 * ui.s, r.h), &cols, &mut ws.color);
    }
    let r = ui.row(34.0);
    let bw = 110.0 * ui.s;
    let mut act = 0;
    if ui.button(id("wsdup"), Rect::new(r.x, r.y + 2.0, bw, r.h - 4.0), "Duplicar") {
        act = 1;
    }
    if ui.button(id("wsup"), Rect::new(r.x + bw + 6.0 * ui.s, r.y + 2.0, bw, r.h - 4.0), "‹ Antes") {
        act = 2;
    }
    if ui.button(id("wsdn"), Rect::new(r.x + 2.0 * (bw + 6.0 * ui.s), r.y + 2.0, bw, r.h - 4.0), "Depois ›") {
        act = 3;
    }
    if n > 1 && ui.button_k(id("wsdel"), Rect::new(r.r() - bw, r.y + 2.0, bw, r.h - 4.0), "Apagar", Btn::Danger) {
        act = 4;
    }
    match act {
        1 => {
            let mut c = app.cfg.workspaces[*sel].clone();
            c.id = app.cfg.workspaces.iter().map(|w| w.id).max().unwrap_or(0) + 1;
            c.name = format!("{} (cópia)", c.name);
            app.cfg.workspaces.insert(*sel + 1, c);
            *sel += 1;
            app.after_ws_change();
            return;
        }
        2 if *sel > 0 => {
            app.cfg.workspaces.swap(*sel, *sel - 1);
            *sel -= 1;
            app.after_ws_change();
            return;
        }
        3 if *sel + 1 < n => {
            app.cfg.workspaces.swap(*sel, *sel + 1);
            *sel += 1;
            app.after_ws_change();
            return;
        }
        4 => {
            let removed = app.cfg.workspaces.remove(*sel);
            let fallback = app.cfg.workspaces[0].id;
            for d in app.docks.iter_mut() {
                if d.ws == removed.id {
                    d.ws = fallback;
                }
            }
            for w in app.wins.wins.iter_mut() {
                if w.ws == removed.id {
                    w.ws = fallback;
                }
            }
            *sel = 0;
            app.after_ws_change();
            return;
        }
        _ => {}
    }
    section(ui, "O DOCK DESTE WORKSPACE");
    let items = app.cfg.workspaces[*sel].items.clone();
    let mut action: Option<(usize, i32)> = None;
    for (k, it) in items.iter().enumerate() {
        let r = ui.row(36.0);
        let ir = Rect::new(r.x, r.y + 4.0 * ui.s, 28.0 * ui.s, 28.0 * ui.s);
        let label = match it {
            DockItem::Start => {
                crate::dock::draw_start_icon(ui.c, ir, true);
                "Botão Iniciar".to_string()
            }
            DockItem::App(a) => {
                let ic = app.sh.icons.get(&crate::icons::icon_path(a), 48);
                crate::dock::draw_app_icon(ui.c, ic.as_ref(), ir, app.cfg.appearance.plates, true, &a.name);
                a.name.clone()
            }
            DockItem::Widgets { ids, .. } => {
                ui.c.fill_rrect(ir, 7.0 * ui.s, Color(1.0, 1.0, 1.0, 0.12));
                let kind = ids.first().and_then(|i| app.cfg.widget(*i)).map(|w| w.kind.clone()).unwrap_or_default();
                ui.c.glyph(kind_glyph(&kind), ir, 13.0 * ui.s, TXT);
                let names: Vec<String> = ids.iter().filter_map(|i| app.cfg.widget(*i).map(|w| w.name.clone())).collect();
                format!("Widget: {}", names.join(" + "))
            }
            DockItem::Spacer { large } => {
                ui.c.stroke_rrect(ir.inset(3.0), 6.0 * ui.s, TXT3, 1.0);
                if *large { "Espaçador grande".into() } else { "Espaçador pequeno".into() }
            }
            DockItem::Folder { path, .. } => {
                if let Some(ic) = app.sh.icons.get(path, 48) {
                    ui.c.bitmap(&ic.bmp, ir, 1.0);
                }
                path.clone()
            }
        };
        ui.c.text(&label, Rect::new(r.x + 38.0 * ui.s, r.y, r.w - 140.0 * ui.s, r.h), 12.5 * ui.s, W_NORMAL, TXT, Align::Left);
        let b = 28.0 * ui.s;
        if ui.icon_btn(idn("iu", k as u64), Rect::new(r.r() - 3.0 * b, r.y + 4.0, b, b), 0xE70E, 10.0, TXT2) {
            action = Some((k, -1));
        }
        if ui.icon_btn(idn("id", k as u64), Rect::new(r.r() - 2.0 * b, r.y + 4.0, b, b), 0xE70D, 10.0, TXT2) {
            action = Some((k, 1));
        }
        if ui.icon_btn(idn("ix", k as u64), Rect::new(r.r() - b, r.y + 4.0, b, b), 0xE711, 10.0, TXT2) {
            action = Some((k, 0));
        }
    }
    if let Some((k, d)) = action {
        let items = &mut app.cfg.workspaces[*sel].items;
        if d == 0 {
            items.remove(k);
        } else {
            let j = (k as i32 + d).clamp(0, items.len() as i32 - 1) as usize;
            items.swap(k, j);
        }
        changed = true;
    }
    section(ui, "ADICIONAR");
    let r = ui.row(36.0);
    let bw = 150.0 * ui.s;
    if ui.button(id("addfile"), Rect::new(r.x, r.y + 2.0, bw, r.h - 4.0), "App ou atalho…") {
        app.sh.pick = Some((u32::MAX - 1, 1, widgets::Pick::Exe));
    }
    if ui.button(id("addfolder"), Rect::new(r.x + bw + 6.0 * ui.s, r.y + 2.0, bw, r.h - 4.0), "Pasta…") {
        app.sh.pick = Some((u32::MAX - 1, 2, widgets::Pick::Folder));
    }
    if ui.button(id("addsp"), Rect::new(r.x + 2.0 * (bw + 6.0 * ui.s), r.y + 2.0, bw, r.h - 4.0), "Espaçador") {
        app.cfg.workspaces[*sel].items.push(DockItem::Spacer { large: false });
        changed = true;
    }
    let has_start = app.cfg.workspaces[*sel].items.iter().any(|i| matches!(i, DockItem::Start));
    if !has_start && ui.button(id("addstart"), Rect::new(r.x + 3.0 * (bw + 6.0 * ui.s), r.y + 2.0, bw.min(r.r() - r.x - 3.0 * (bw + 6.0 * ui.s)), r.h - 4.0), "Botão Iniciar") {
        app.cfg.workspaces[*sel].items.insert(0, DockItem::Start);
        changed = true;
    }
    let r = ui.row(36.0);
    ui.text_input(id("appsearch"), Rect::new(r.x, r.y + 2.0, r.w, r.h - 4.0), app_search, "Buscar entre os apps instalados…", false);
    let q = app_search.to_lowercase();
    if q.len() >= 2 {
        let found: Vec<crate::icons::CatalogApp> = app.sh.catalog().iter().filter(|a| a.name.to_lowercase().contains(&q)).take(12).cloned().collect();
        for (k, a) in found.iter().enumerate() {
            let r = ui.row(34.0);
            let (hov, clk) = ui.hit(idn("cat", k as u64), r);
            if hov {
                ui.c.fill_rrect(r, 6.0 * ui.s, FILL);
            }
            let p = a.to_pinned();
            if let Some(ic) = app.sh.icons.get(&crate::icons::icon_path(&p), 48) {
                ui.c.bitmap(&ic.bmp, Rect::new(r.x + 4.0, r.y + 5.0 * ui.s, 24.0 * ui.s, 24.0 * ui.s), 1.0);
            }
            ui.c.text(&a.name, Rect::new(r.x + 36.0 * ui.s, r.y, r.w - 70.0 * ui.s, r.h), 12.5 * ui.s, W_NORMAL, TXT, Align::Left);
            ui.c.glyph(0xE710, Rect::new(r.r() - 30.0 * ui.s, r.y, 24.0 * ui.s, r.h), 10.0 * ui.s, ACCENT);
            if clk {
                let items = &mut app.cfg.workspaces[*sel].items;
                let pos = items.iter().rposition(|x| matches!(x, DockItem::App(_) | DockItem::Start)).map(|p| p + 1).unwrap_or(items.len());
                items.insert(pos, DockItem::App(p));
                changed = true;
            }
        }
    }
    if changed {
        app.after_ws_change();
    }
}

fn page_widgets(app: &mut App, ui: &mut Ui, search: &mut String, page: &mut Page) {
    page_title(ui, "Widgets", "Clique num cartão: o widget vai direto para o fim da dock do workspace atual.");
    let r = ui.row(36.0);
    ui.text_input(id("wsearch"), Rect::new(r.x, r.y + 2.0, r.w, r.h - 4.0), search, "Buscar widget…", false);
    ui.gap(6.0);
    let q = search.to_lowercase();
    let mut last_group = "";
    let cols = 2;
    let mut col = 0;
    let mut row_y = 0.0;
    let cw = (ui.area.w - 10.0 * ui.s) / cols as f32;
    let chh = 66.0 * ui.s;
    for (k, e) in CATALOG.iter().enumerate() {
        if !q.is_empty() && !e.name.to_lowercase().contains(&q) && !e.desc.to_lowercase().contains(&q) && !e.group.to_lowercase().contains(&q) {
            continue;
        }
        if e.group != last_group {
            if col != 0 {
                ui.y = row_y + chh + 8.0 * ui.s;
                col = 0;
            }
            last_group = e.group;
            ui.gap(6.0);
            ui.caption(&e.group.to_uppercase());
        }
        if col == 0 {
            row_y = ui.y;
        }
        let cr = Rect::new(ui.area.x + col as f32 * (cw + 10.0 * ui.s), row_y, cw, chh);
        let (hov, clk) = ui.hit(idn("cat", k as u64), cr);
        ui.c.fill_rrect(cr, 10.0 * ui.s, if hov { FILL_HOVER } else { FILL });
        let ir = Rect::new(cr.x + 10.0 * ui.s, cr.y + 11.0 * ui.s, 44.0 * ui.s, 44.0 * ui.s);
        ui.c.fill_rrect_vgrad(ir, 10.0 * ui.s, &[(0.0, Color::rgba(60, 60, 70, 1.0)), (1.0, Color::rgba(30, 30, 36, 1.0))]);
        ui.c.glyph(kind_glyph(e.kind), ir, 17.0 * ui.s, WHITE);
        let tr = Rect::new(ir.r() + 10.0 * ui.s, cr.y + 8.0 * ui.s, cr.r() - ir.r() - 18.0 * ui.s, 20.0 * ui.s);
        ui.c.text(e.name, tr, 12.5 * ui.s, W_SEMIBOLD, TXT, Align::Left);
        ui.c.text_wrap(e.desc, Rect::new(tr.x, tr.b(), tr.w, 34.0 * ui.s), 11.0 * ui.s, W_NORMAL, TXT2, Align::Left);
        if clk {
            let wid = app.add_widget(e.kind, e.style, e.units, e.data, e.name);
            let _ = wid;
        }
        col += 1;
        if col == cols {
            col = 0;
            ui.y = row_y + chh + 8.0 * ui.s;
        }
    }
    if col != 0 {
        ui.y = row_y + chh + 8.0 * ui.s;
    }
    section(ui, "EM USO");
    let list: Vec<(u32, String, String)> = app.cfg.widgets.iter().map(|w| (w.id, w.name.clone(), w.kind.clone())).collect();
    if list.is_empty() {
        ui.note("Nenhum widget ainda.");
    }
    let mut del = None;
    for (wid, name, kind) in list {
        let r = ui.row(38.0);
        let ir = Rect::new(r.x, r.y + 5.0 * ui.s, 28.0 * ui.s, 28.0 * ui.s);
        ui.c.fill_rrect(ir, 7.0 * ui.s, Color(1.0, 1.0, 1.0, 0.12));
        ui.c.glyph(kind_glyph(&kind), ir, 12.0 * ui.s, TXT);
        let where_: Vec<String> = app.cfg.workspaces.iter().filter(|ws| ws.items.iter().any(|it| matches!(it, DockItem::Widgets { ids, .. } if ids.contains(&wid)))).map(|ws| ws.name.clone()).collect();
        ui.c.text(&name, Rect::new(r.x + 38.0 * ui.s, r.y + 2.0, r.w * 0.5, r.h * 0.55), 12.5 * ui.s, W_MEDIUM, TXT, Align::Left);
        ui.c.text(&if where_.is_empty() { "fora da dock".to_string() } else { where_.join(", ") }, Rect::new(r.x + 38.0 * ui.s, r.y + r.h * 0.5, r.w * 0.5, r.h * 0.45), 11.0 * ui.s, W_NORMAL, TXT2, Align::Left);
        let bw = 80.0 * ui.s;
        if ui.button(idn("ed", wid as u64), Rect::new(r.r() - 2.0 * bw - 6.0 * ui.s, r.y + 4.0, bw, r.h - 8.0), "Editar") {
            *page = Page::EditWidget(wid);
            ui.st.scroll.insert(id("content"), 0.0);
        }
        if ui.button_k(idn("dl", wid as u64), Rect::new(r.r() - bw, r.y + 4.0, bw, r.h - 8.0), "Apagar", Btn::Danger) {
            del = Some(wid);
        }
    }
    if let Some(w) = del {
        app.delete_widget(w);
    }
}

fn page_edit_widget(app: &mut App, ui: &mut Ui, wid: u32, page: &mut Page) {
    let r = ui.row(30.0);
    if ui.button_k(id("back"), Rect::new(r.x, r.y, 100.0 * ui.s, r.h), "‹ Widgets", Btn::Ghost) {
        *page = Page::Widgets;
        return;
    }
    let Some(idx) = app.cfg.widgets.iter().position(|w| w.id == wid) else {
        ui.note("Este widget não existe mais.");
        return;
    };
    let kind = app.cfg.widgets[idx].kind.clone();
    page_title(ui, &app.cfg.widgets[idx].name.clone(), widgets::CATALOG.iter().find(|e| e.kind == kind).map(|e| e.desc).unwrap_or(""));
    let mut ch = false;
    let mut rebuild = false;
    {
        let wi = &mut app.cfg.widgets[idx];
        ch |= ui.labeled_input(id("wname"), "Nome", &mut wi.name, "Nome").changed;
    }
    let styles = app.widgets.get(&wid).map(|w| w.styles()).unwrap_or(&[]);
    if !styles.is_empty() {
        let mut st = app.cfg.widgets[idx].style.min(styles.len() as u32 - 1);
        if ui.labeled_segmented(id("wstyle"), "Estilo", styles, &mut st) {
            app.cfg.widgets[idx].style = st;
            rebuild = true;
        }
    }
    let maxu = app.widgets.get(&wid).map(|w| w.max_units()).unwrap_or(4).clamp(1, 4);
    if maxu > 1 {
        let opts: Vec<&str> = ["Um ícone", "Dois", "Três", "Quatro ícones"].iter().take(maxu as usize).copied().collect();
        let mut u = app.cfg.widgets[idx].units.clamp(1, maxu) - 1;
        if ui.labeled_segmented(id("wunits"), "Largura do tile", &opts, &mut u) {
            app.cfg.widgets[idx].units = u + 1;
            rebuild = true;
        }
    }
    section(ui, "MOSTRAR NOS WORKSPACES");
    for ws in app.cfg.workspaces.clone() {
        let mut on = ws.items.iter().any(|it| matches!(it, DockItem::Widgets { ids, .. } if ids.contains(&wid)));
        if ui.row_toggle(idn("wws", ws.id as u64), &ws.name, &mut on) {
            app.set_widget_in_ws(wid, ws.id, on);
        }
    }
    section(ui, "OPÇÕES");
    if let Some(w) = app.widgets.get_mut(&wid) {
        app.sh.current_widget = wid;
        w.settings(ui, &mut app.sh);
    }
    ui.gap(14.0);
    let r = ui.row(34.0);
    if ui.button_k(id("wdel"), Rect::new(r.x, r.y, 150.0 * ui.s, r.h), "Apagar widget", Btn::Danger) {
        app.delete_widget(wid);
        *page = Page::Widgets;
        return;
    }
    if app.sh.save {
        rebuild = true;
    }
    if ch || rebuild {
        app.mark_save();
        app.rebuild_all();
        app.schedule_tick();
        app.schedule_pet();
    }
}

fn page_bar(app: &mut App, ui: &mut Ui) {
    page_title(ui, "Barra e bandeja", "Rede, volume, bateria, os ícones dos apps e o relógio, em linha no fim da dock.");
    let g = &mut app.cfg.general;
    let mut ch = false;
    ch |= ui.row_toggle(id("st"), "Ícones de status e da bandeja", &mut g.show_status);
    ch |= ui.row_toggle(id("ck"), "Relógio", &mut g.show_clock);
    if g.show_clock {
        ch |= ui.row_toggle(id("dt"), "Mostrar a data", &mut g.show_date);
        ch |= ui.labeled_segmented(id("al"), "Alinhamento do relógio", &["Esquerda", "Centro", "Direita"], &mut g.clock_align);
    }
    if ch {
        app.apply_general();
    }
    section(ui, "ÍCONES DA BANDEJA NA DOCK");
    ui.note("Os fixados aparecem em linha na dock; os outros ficam na setinha. Clique com o botão direito num ícone para o menu do próprio app.");
    let icons_now = crate::tray::visible_icons();
    if icons_now.is_empty() {
        ui.note("Nenhum app com ícone na bandeja agora.");
    }
    let mut toggle: Option<(String, bool)> = None;
    for (k, ic) in icons_now.iter().enumerate() {
        let exe = ic.key.split('|').next().unwrap_or("").to_string();
        let mut on = app.cfg.tray_pinned.iter().any(|p| *p == ic.key || *p == format!("{}|*", exe));
        let r = ui.row(34.0);
        let key = format!("tray:{}:{}", ic.key, ic.hicon.0 as usize);
        let bmp = app.sh.icons.get_hicon_cached(&key).or_else(|| app.sh.icons.put_hicon(&key, ic.hicon));
        if let Some(b) = bmp {
            ui.c.bitmap(&b.bmp, Rect::new(r.x + 4.0, r.y + 8.0 * ui.s, 18.0 * ui.s, 18.0 * ui.s), 1.0);
        }
        let label = if ic.tip.is_empty() { exe.clone() } else { ic.tip.lines().next().unwrap_or("").to_string() };
        let tr = Rect::new(r.x + 32.0 * ui.s, r.y, r.w - 32.0 * ui.s, r.h);
        if ui.toggle(idn("tp", k as u64), tr, &label, &mut on) {
            toggle = Some((ic.key.clone(), on));
        }
    }
    if let Some((k, on)) = toggle {
        app.set_tray_pin(&k, on);
    }
}

fn page_about(app: &mut App, ui: &mut Ui) {
    page_title(ui, "GlassDock", "Versão 1.0 · uma dock de vidro com widgets para o Windows 10 e 11.");
    ui.note("Feito para ser leve: app nativo (Win32 + Direct2D), sem navegador embutido. Parado, não redesenha nada; só acorda com eventos do Windows (janelas, volume, rede, bateria) e com os relógios dos widgets que estão na tela.");
    ui.note("O desfoque é feito pelo próprio Windows (DWM). Os ícones e widgets são desenhados por software só quando mudam.");
    ui.gap(8.0);
    let r = ui.row(36.0);
    if ui.button(id("data"), Rect::new(r.x, r.y + 2.0, 200.0 * ui.s, r.h - 4.0), "Abrir pasta de dados") {
        shell_open(&data_dir().to_string_lossy());
    }
    if ui.button_k(id("quit"), Rect::new(r.x + 210.0 * ui.s, r.y + 2.0, 200.0 * ui.s, r.h - 4.0), "Sair do GlassDock", Btn::Danger) {
        app.quit();
    }
    section(ui, "ATALHOS");
    for (k, v) in [
        ("Ctrl+Alt+1 … 9", "Troca para o workspace 1 a 9"),
        ("Ctrl+Alt+D", "Foco na dock: setas andam, Enter abre, Esc sai"),
        ("Roda do mouse na dock", "Troca de workspace (em pilhas, troca de widget)"),
        ("Clique do meio num app", "Abre uma janela nova"),
        ("Arrastar o separador", "Aumenta ou diminui os ícones"),
        ("Arrastar para fora", "Tira o item da dock"),
    ] {
        let r = ui.row(24.0);
        ui.c.text(k, Rect::new(r.x, r.y, 200.0 * ui.s, r.h), 12.0 * ui.s, W_SEMIBOLD, TXT, Align::Left);
        ui.c.text(v, Rect::new(r.x + 210.0 * ui.s, r.y, r.w - 210.0 * ui.s, r.h), 12.0 * ui.s, W_NORMAL, TXT2, Align::Left);
    }
}

/// Resultado dos seletores de arquivo abertos pelos Ajustes.
pub fn on_picked(app: &mut App, purpose: u32, paths: Vec<String>) {
    let sel = app.settings.as_ref().map(|s| s.ws_sel).unwrap_or(0);
    let Some(ws) = app.cfg.workspaces.get_mut(sel) else { return };
    for p in paths {
        if purpose == 2 {
            ws.items.push(DockItem::Folder { path: p, list: false, sort: 2 });
        } else {
            let pos = ws.items.iter().rposition(|x| matches!(x, DockItem::App(_) | DockItem::Start)).map(|p| p + 1).unwrap_or(ws.items.len());
            ws.items.insert(pos, DockItem::App(crate::icons::pinned_from_path(&p)));
        }
    }
    app.after_ws_change();
    app.render_settings();
}

impl App {
    pub fn after_ws_change(&mut self) {
        self.sync_ws_list();
        self.mark_save();
        self.rebuild_all();
    }

    pub fn setup_appbar_pub(&mut self, i: usize) {
        let _ = i;
    }
}
