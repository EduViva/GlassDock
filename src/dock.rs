// A dock: montagem dos itens, layout com ampliação, desenho e teste de clique.
use std::collections::{BTreeMap, HashMap};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::HMONITOR;

use crate::config::*;
use crate::gfx::*;
use crate::glass;
use crate::icons;
use crate::status::Net;
use crate::tray;
use crate::util::*;
use crate::widgets::{Shared, Tile, Widget};
use crate::winlist::{self, WinList};

#[derive(Clone, Debug, PartialEq)]
pub enum SlotKind {
    Start,
    WsDots,
    Pinned,
    Running,
    Widget,
    Spacer,
    Folder,
    Sep,
    Downloads,
    Trash,
    Tray,
    Chevron,
    Net,
    Vol,
    Bat,
    Clock,
}

#[derive(Clone, Debug)]
pub struct Slot {
    pub kind: SlotKind,
    /// comprimento no eixo principal, em "ícones"
    pub len: f32,
    pub mag: bool,
    pub small: bool,
    pub item: Option<usize>,
    pub wins: Vec<HWND>,
    pub icon: String,
    pub name: String,
    pub key: String,
    pub rect: Rect,
    pub f: f32,
    pub active: bool,
    pub flashing: bool,
    pub units: u32,
    pub widget: u32,
    pub stack: usize,
    /// 0 = área dos fixados, 1 = apps abertos/Downloads/Lixeira, 2 = bandeja/relógio
    pub zone: u8,
    /// faz parte do grupo de apps (fixados + abertos juntos)
    pub in_block: bool,
    /// ícone de status arrumado em grade (duas linhas)
    pub grid: bool,
}

impl Slot {
    fn new(kind: SlotKind, len: f32) -> Slot {
        Slot {
            kind,
            len,
            mag: true,
            small: false,
            item: None,
            wins: vec![],
            icon: String::new(),
            name: String::new(),
            key: String::new(),
            rect: Rect::default(),
            f: 1.0,
            active: false,
            flashing: false,
            units: 1,
            widget: 0,
            stack: 1,
            zone: 0,
            in_block: false,
            grid: false,
        }
    }
    pub fn is_app(&self) -> bool {
        matches!(self.kind, SlotKind::Pinned | SlotKind::Running)
    }
    pub fn draggable(&self) -> bool {
        matches!(self.kind, SlotKind::Pinned | SlotKind::Widget | SlotKind::Spacer | SlotKind::Folder | SlotKind::Start | SlotKind::Running)
            || (self.kind == SlotKind::Sep && self.item.is_some())
    }
}

pub struct Drag {
    pub slot: usize,
    pub u: f32,
    pub v: f32,
    pub out: bool,
    pub target: usize,
    pub resize: bool,
    pub start_size: f32,
    pub start_v: f32,
}

pub struct Dock {
    pub idx: usize,
    pub monitor: HMONITOR,
    pub mon: RECT,
    pub work: RECT,
    pub dpi: u32,
    pub ws: u32,
    pub hwnd: HWND,
    pub plate: HWND,
    pub trigger: HWND,
    pub pet_hwnd: HWND,
    pub surf: Surface,
    pub pet_surf: Surface,
    pub slots: Vec<Slot>,
    pub win: RECT,
    pub bar: Rect,
    pub mouse: Option<(f32, f32)>,
    pub mag: f32,
    pub mag_target: f32,
    pub hover: Option<usize>,
    pub press: Option<(usize, f32, f32)>,
    pub drag: Option<Drag>,
    pub hide: f32,
    pub hide_target: f32,
    pub fullscreen: bool,
    pub appbar: bool,
    pub tracking: bool,
    pub bounce: HashMap<String, u64>,
    /// apps abertos pelo clique que ainda não mostraram janela
    pub launching: HashMap<String, u64>,
    pub anim_last: u64,
    pub drop_hover: Option<usize>,
    pub hover_since: u64,
    pub s: f32,
    pub pet_rect: RECT,
    pub pet_last: u64,
    pub kb_focus: Option<usize>,
    pub reserved: i32,
    pub plate_rect: RECT,
}

pub struct Geo {
    pub s: f32,
    pub sc: f32,
    pub gap: f32,
    pub sgap: f32,
    pub pad_side: f32,
    pub pad_top: f32,
    pub pad_bottom: f32,
    pub thick: f32,
    pub margin: f32,
    pub head: f32,
    pub h: f32,
    pub main: f32,
    pub pos: DockPos,
    /// início e comprimento da janela ao longo da borda (só o necessário)
    pub u0: f32,
    pub len: f32,
}

impl Dock {
    pub fn new(idx: usize, monitor: HMONITOR, ws: u32) -> Dock {
        let plate = glass::create_plate(glass::K_PLATE, idx, None);
        let hwnd = glass::create_layered(glass::K_DOCK, idx, Some(plate), false);
        let trigger = glass::create_trigger(idx);
        let pet_hwnd = glass::create_layered(glass::K_PET, idx, Some(plate), false);
        unsafe {
            // o pet não recebe cliques
            let ex = windows::Win32::UI::WindowsAndMessaging::GetWindowLongW(pet_hwnd, windows::Win32::UI::WindowsAndMessaging::GWL_EXSTYLE);
            windows::Win32::UI::WindowsAndMessaging::SetWindowLongW(pet_hwnd, windows::Win32::UI::WindowsAndMessaging::GWL_EXSTYLE, ex | 0x20);
        }
        let (mon, work) = glass::monitor_info(monitor);
        Dock {
            idx,
            monitor,
            mon,
            work,
            dpi: glass::monitor_dpi(monitor),
            ws,
            hwnd,
            plate,
            trigger,
            pet_hwnd,
            surf: Surface::new(),
            pet_surf: Surface::new(),
            slots: vec![],
            win: RECT::default(),
            bar: Rect::default(),
            mouse: None,
            mag: 0.0,
            mag_target: 0.0,
            hover: None,
            press: None,
            drag: None,
            hide: 0.0,
            hide_target: 0.0,
            fullscreen: false,
            appbar: false,
            tracking: false,
            bounce: HashMap::new(),
            launching: HashMap::new(),
            anim_last: 0,
            drop_hover: None,
            hover_since: 0,
            s: 46.0,
            pet_rect: RECT::default(),
            pet_last: 0,
            kb_focus: None,
            reserved: 0,
            plate_rect: RECT::default(),
        }
    }

    pub fn destroy(&mut self) {
        crate::dragdrop::unregister(self.hwnd);
        glass::destroy(self.pet_hwnd);
        glass::destroy(self.hwnd);
        glass::destroy(self.trigger);
        glass::destroy(self.plate);
    }

    pub fn update_monitor(&mut self) {
        let (mon, work) = glass::monitor_info(self.monitor);
        self.mon = mon;
        self.work = work;
        self.dpi = glass::monitor_dpi(self.monitor);
    }

    /// Retângulo de referência (a borda onde a dock encosta).
    pub fn edge_rect(&self, cfg: &Config) -> RECT {
        if cfg.general.taskbar == TaskbarMode::Keep {
            // acima da barra de tarefas visível (sem contar a nossa reserva)
            let mut r = self.work;
            match cfg.general.position {
                DockPos::Bottom => r.bottom += self.reserved,
                DockPos::Top => r.top -= self.reserved,
                DockPos::Left => r.left -= self.reserved,
                DockPos::Right => r.right += self.reserved,
            }
            r
        } else {
            self.mon
        }
    }

    pub fn geo(&self, cfg: &Config) -> Geo {
        let sc = scale(self.dpi);
        let ap = &cfg.appearance;
        let pos = cfg.general.position;
        let edge = self.edge_rect(cfg);
        let main = if matches!(pos, DockPos::Bottom | DockPos::Top) { (edge.right - edge.left) as f32 } else { (edge.bottom - edge.top) as f32 };
        let mut s = (ap.icon_size.clamp(28.0, 96.0) * sc).round();
        // encolhe para caber
        let need = |s: f32| -> f32 {
            let mut t = 0.44 * s;
            let mut prev_small = false;
            for (i, sl) in self.slots.iter().enumerate() {
                if i > 0 {
                    t += if sl.small && prev_small { 0.06 * s } else { 0.18 * s };
                }
                t += sl.len * s;
                prev_small = sl.small;
            }
            t
        };
        let avail = main - 24.0 * sc;
        let n = need(s);
        if n > avail && n > 0.0 {
            s = (s * avail / n).floor().max(16.0 * sc);
        }
        let mag = if ap.hover == Hover::Magnify { ap.magnify.clamp(1.0, 2.2) } else if ap.hover == Hover::Scale { 1.18 } else { 1.0 };
        let thick = s * 1.44;
        let margin = (6.0 * sc).round();
        let head = s * (mag - 1.0) + s * 0.75 + 12.0 * sc;
        let base = need(s);
        let len = (base + s * (mag - 1.0) * 3.4 + s * 6.0).min(main).ceil();
        let u0 = ((main - len) / 2.0).floor();
        Geo {
            u0,
            len,
            s,
            sc,
            gap: s * 0.18,
            sgap: s * 0.06,
            pad_side: s * 0.22,
            pad_top: s * 0.17,
            pad_bottom: s * 0.27,
            thick,
            margin,
            head,
            h: (margin + thick + head).ceil(),
            main,
            pos,
        }
    }

    /// Converte (u ao longo da dock, v a partir da borda) em coordenadas da superfície.
    pub fn map(&self, g: &Geo, u: f32, v: f32) -> (f32, f32) {
        let u = u - g.u0;
        match g.pos {
            DockPos::Bottom => (u, g.h - v),
            DockPos::Top => (u, v),
            DockPos::Left => (v, u),
            DockPos::Right => (g.h - v, u),
        }
    }

    pub fn map_rect(&self, g: &Geo, u0: f32, len: f32, v0: f32, thick: f32) -> Rect {
        let u0 = u0 - g.u0;
        match g.pos {
            DockPos::Bottom => Rect::new(u0, g.h - v0 - thick, len, thick),
            DockPos::Top => Rect::new(u0, v0, len, thick),
            DockPos::Left => Rect::new(v0, u0, thick, len),
            DockPos::Right => Rect::new(g.h - v0 - thick, u0, thick, len),
        }
    }

    pub fn unmap(&self, g: &Geo, x: f32, y: f32) -> (f32, f32) {
        let (u, v) = match g.pos {
            DockPos::Bottom => (x, g.h - y),
            DockPos::Top => (x, y),
            DockPos::Left => (y, x),
            DockPos::Right => (y, g.h - x),
        };
        (u + g.u0, v)
    }

    pub fn vertical(&self, cfg: &Config) -> bool {
        matches!(cfg.general.position, DockPos::Left | DockPos::Right)
    }

    /// Retângulo da janela de conteúdo na tela.
    pub fn window_rect(&self, cfg: &Config, g: &Geo) -> RECT {
        let e = self.edge_rect(cfg);
        let h = g.h as i32;
        let slide = (self.hide * (g.margin + g.thick + 4.0)) as i32;
        let a = g.u0 as i32;
        let l = g.len as i32;
        match g.pos {
            DockPos::Bottom => RECT { left: e.left + a, top: e.bottom - h + slide, right: e.left + a + l, bottom: e.bottom + slide },
            DockPos::Top => RECT { left: e.left + a, top: e.top - slide, right: e.left + a + l, bottom: e.top + h - slide },
            DockPos::Left => RECT { left: e.left - slide, top: e.top + a, right: e.left + h - slide, bottom: e.top + a + l },
            DockPos::Right => RECT { left: e.right - h + slide, top: e.top + a, right: e.right + slide, bottom: e.top + a + l },
        }
    }

    /// Faixa reservada na borda (para janelas maximizadas pararem antes).
    pub fn reserve_thickness(&self, g: &Geo) -> i32 {
        (g.margin + g.thick + 2.0 * g.sc).ceil() as i32
    }

    pub fn trigger_rect(&self, cfg: &Config) -> RECT {
        let e = self.edge_rect(cfg);
        let t = 5;
        match cfg.general.position {
            DockPos::Bottom => RECT { left: e.left, top: e.bottom - t, right: e.right, bottom: e.bottom },
            DockPos::Top => RECT { left: e.left, top: e.top, right: e.right, bottom: e.top + t },
            DockPos::Left => RECT { left: e.left, top: e.top, right: e.left + t, bottom: e.bottom },
            DockPos::Right => RECT { left: e.right - t, top: e.top, right: e.right, bottom: e.bottom },
        }
    }
}

// ---------------------------------------------------------------------------
// Montagem dos itens
// ---------------------------------------------------------------------------

pub fn running_for_dock<'a>(d: &Dock, cfg: &Config, wins: &'a WinList, all_docks: usize) -> Vec<&'a winlist::Win> {
    wins.wins
        .iter()
        .filter(|w| {
            if cfg.general.only_current_ws && w.ws != d.ws {
                return false;
            }
            if all_docks > 1 && !cfg.general.apps_from_all_monitors && w.monitor != d.monitor.0 as isize {
                return false;
            }
            !cfg.is_hidden_app(&w.exe, &w.aumid)
        })
        .collect()
}

pub fn build(d: &mut Dock, cfg: &Config, wins: &WinList, widgets: &BTreeMap<u32, Box<dyn Widget>>, sh: &mut Shared, all_docks: usize) {
    let mut slots = vec![];
    let vertical = d.vertical(cfg);
    let Some(wsi) = cfg.ws_index(d.ws).or(if cfg.workspaces.is_empty() { None } else { Some(0) }) else {
        d.slots = slots;
        return;
    };
    let ws = &cfg.workspaces[wsi];
    let running = running_for_dock(d, cfg, wins, all_docks);
    let mut used: Vec<bool> = vec![false; running.len()];
    let fg = wins.fg;
    if cfg.general.ws_selector && cfg.workspaces.len() > 1 {
        let mut s = Slot::new(SlotKind::WsDots, (cfg.workspaces.len() as f32 * 0.3).max(0.5));
        s.mag = false;
        slots.push(s);
    }
    // Quais janelas pertencem a quais apps fixados: primeiro pelos critérios
    // seguros, depois (só para os que ficaram sem janela) pelos mais soltos.
    let mut assigned: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, it) in ws.items.iter().enumerate() {
        if let DockItem::App(app) = it {
            for (k, w) in running.iter().enumerate() {
                if !used[k] && winlist::app_matches(app, w) {
                    used[k] = true;
                    assigned.entry(i).or_default().push(k);
                }
            }
        }
    }
    for (i, it) in ws.items.iter().enumerate() {
        if let DockItem::App(app) = it {
            if assigned.contains_key(&i) {
                continue;
            }
            for (k, w) in running.iter().enumerate() {
                if !used[k] && winlist::app_matches_loose(app, w) {
                    used[k] = true;
                    assigned.entry(i).or_default().push(k);
                }
            }
        }
    }
    let mut zone1: Vec<Slot> = vec![];
    let mut zone2: Vec<Slot> = vec![];
    // modo livre: fixados, abertos e espaçadores formam um grupo só
    let mixed = cfg.general.mixed_apps;
    let mut block: Vec<Slot> = vec![];
    let mut block_pos: Option<usize> = None;
    let mut spacer_n = 0;
    for (i, it) in ws.items.iter().enumerate() {
        match it {
            DockItem::Start => {
                let mut s = Slot::new(SlotKind::Start, 1.0);
                s.item = Some(i);
                s.name = "Iniciar".into();
                s.key = "start".into();
                slots.push(s);
            }
            DockItem::App(app) => {
                let mut s = Slot::new(SlotKind::Pinned, 1.0);
                s.item = Some(i);
                s.name = app.name.clone();
                s.icon = icons::icon_path(app);
                s.key = format!("pin:{}", app.launch.to_lowercase());
                for &k in assigned.get(&i).map(|v| v.as_slice()).unwrap_or(&[]) {
                    let w = running[k];
                    s.wins.push(w.hwnd);
                    s.active |= w.hwnd == fg;
                    s.flashing |= w.flashing;
                }
                if mixed {
                    if block_pos.is_none() {
                        block_pos = Some(slots.len());
                    }
                    s.in_block = true;
                    block.push(s);
                } else {
                    slots.push(s);
                }
            }
            DockItem::Widgets { ids, cur, zone } => {
                let ids: Vec<u32> = ids.iter().copied().filter(|id| widgets.contains_key(id)).collect();
                if ids.is_empty() {
                    continue;
                }
                let wid = ids[(*cur).min(ids.len() - 1)];
                let units = cfg.widget(wid).map(|w| w.units).unwrap_or(1).clamp(1, 4);
                let units = if vertical { 1 } else { units };
                let len = units as f32 + (units as f32 - 1.0) * 0.18;
                let mut s = Slot::new(SlotKind::Widget, len);
                s.item = Some(i);
                s.units = units;
                s.widget = wid;
                s.stack = ids.len();
                s.name = cfg.widget(wid).map(|w| w.name.clone()).unwrap_or_default();
                s.key = format!("w:{}", wid);
                match zone {
                    1 => zone1.push(s),
                    2 => zone2.push(s),
                    _ => slots.push(s),
                }
            }
            DockItem::Spacer { large } => {
                let mut s = Slot::new(SlotKind::Spacer, if *large { 1.0 } else { 0.5 });
                s.item = Some(i);
                s.mag = false;
                s.key = format!("spacer:{}", spacer_n);
                spacer_n += 1;
                if mixed && block_pos.is_some() {
                    s.in_block = true;
                    block.push(s);
                } else {
                    slots.push(s);
                }
            }
            DockItem::Divider => {
                let mut s = Slot::new(SlotKind::Sep, 0.3);
                s.item = Some(i);
                s.mag = false;
                s.key = format!("divider:{}", spacer_n);
                s.name = String::new();
                spacer_n += 1;
                if mixed && block_pos.is_some() {
                    s.in_block = true;
                    block.push(s);
                } else {
                    slots.push(s);
                }
            }
            DockItem::Folder { path, .. } => {
                let mut s = Slot::new(SlotKind::Folder, 1.0);
                s.item = Some(i);
                s.icon = path.clone();
                s.name = std::path::Path::new(path).file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| path.clone());
                s.key = format!("folder:{}", path.to_lowercase());
                slots.push(s);
            }
        }
    }
    // abertos que não estão fixados
    let mut idx_r = slots.len();
    if cfg.general.show_running {
        let mut groups: Vec<Slot> = vec![];
        for (k, w) in running.iter().enumerate() {
            if used[k] {
                continue;
            }
            if let Some(g) = groups.iter_mut().find(|g| g.key == w.key) {
                g.wins.push(w.hwnd);
                g.active |= w.hwnd == fg;
                g.flashing |= w.flashing;
                continue;
            }
            let mut s = Slot::new(SlotKind::Running, 1.0);
            s.key = w.key.clone();
            s.icon = if !w.aumid.is_empty() { format!("shell:AppsFolder\\{}", w.aumid) } else { w.exe.clone() };
            s.name = icons::app_display_name(&w.exe, &w.aumid);
            s.wins.push(w.hwnd);
            s.active = w.hwnd == fg;
            s.flashing = w.flashing;
            groups.push(s);
        }
        if mixed {
            for mut g in groups {
                g.in_block = true;
                block.push(g);
            }
        } else if !groups.is_empty() {
            let mut sep = Slot::new(SlotKind::Sep, 0.3);
            sep.mag = false;
            slots.push(sep);
            slots.extend(groups);
        }
    }
    if mixed {
        // ordem escolhida pelo usuário; os novos entram no fim (como na barra de tarefas)
        let order = &ws.app_order;
        let mut keyed: Vec<(usize, usize, Slot)> = block
            .into_iter()
            .enumerate()
            .map(|(n, s)| (order.iter().position(|k| *k == s.key).unwrap_or(usize::MAX), n, s))
            .collect();
        keyed.sort_by_key(|x| (x.0, x.1));
        let pos = block_pos.unwrap_or(slots.len()).min(slots.len());
        let tail_items: Vec<Slot> = slots.split_off(pos);
        slots.extend(keyed.into_iter().map(|x| x.2));
        slots.extend(tail_items);
        idx_r = slots.len();
    }
    // widgets colocados depois dos apps abertos
    if !zone1.is_empty() {
        if slots.len() == idx_r {
            let mut sep = Slot::new(SlotKind::Sep, 0.3);
            sep.mag = false;
            slots.push(sep);
        }
        slots.extend(zone1);
    }
    let mut tail: Vec<Slot> = vec![];
    if cfg.general.show_downloads {
        let mut s = Slot::new(SlotKind::Downloads, 1.0);
        s.icon = known_folder(&windows::Win32::UI::Shell::FOLDERID_Downloads).map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        s.name = "Downloads".into();
        s.key = "downloads".into();
        tail.push(s);
    }
    if cfg.general.show_trash {
        let mut s = Slot::new(SlotKind::Trash, 1.0);
        s.name = "Lixeira".into();
        s.key = "trash".into();
        tail.push(s);
    }
    if !tail.is_empty() {
        let mut sep = Slot::new(SlotKind::Sep, 0.3);
        sep.mag = false;
        slots.push(sep);
        slots.extend(tail);
    }
    // ícones de status em linha + relógio
    let mut status: Vec<Slot> = vec![];
    if cfg.general.show_status {
        let pinned = &cfg.tray_pinned;
        let icons_now = tray::visible_icons();
        for ic in &icons_now {
            let exe = ic.key.split('|').next().unwrap_or("");
            if pinned.iter().any(|p| p == &ic.key || *p == format!("{}|*", exe)) {
                let mut s = Slot::new(SlotKind::Tray, if vertical { 0.6 } else { 0.5 });
                s.key = ic.key.clone();
                s.name = ic.tip.clone();
                s.mag = false;
                s.small = true;
                status.push(s);
            }
        }
        // a setinha dos ícones ocultos (pode ser escondida; o menu da barra a abre)
        if cfg.general.show_chevron && !icons_now.is_empty() {
            let mut s = Slot::new(SlotKind::Chevron, 0.4);
            s.mag = false;
            s.small = true;
            s.name = "Mostrar ícones ocultos".into();
            status.insert(0, s);
        }
        for (k, name, on) in [(SlotKind::Net, "Rede", cfg.general.show_net), (SlotKind::Vol, "Volume", cfg.general.show_vol)] {
            if !on {
                continue;
            }
            let mut s = Slot::new(k, 0.5);
            s.mag = false;
            s.small = true;
            s.name = name.into();
            status.push(s);
        }
        if sh.status.battery.present && cfg.general.show_bat {
            let mut s = Slot::new(SlotKind::Bat, 0.5);
            s.mag = false;
            s.small = true;
            s.name = "Bateria".into();
            status.push(s);
        }
        // grade: duas linhas na altura de um ícone (ocupa metade do espaço)
        if cfg.general.status_layout >= 1 && !vertical {
            for s in status.iter_mut() {
                s.grid = true;
                s.len = 0.46;
            }
        }
    }
    if cfg.general.show_clock {
        let len = if vertical { 1.0 } else { clock_len(cfg) };
        let mut s = Slot::new(SlotKind::Clock, len);
        s.mag = false;
        s.small = true;
        s.name = "Relógio".into();
        status.push(s);
    }
    let idx_s = slots.len();
    if !status.is_empty() {
        let mut sep = Slot::new(SlotKind::Sep, 0.3);
        sep.mag = false;
        slots.push(sep);
        slots.extend(status);
    }
    // widgets colocados depois da bandeja e do relógio
    if !zone2.is_empty() {
        let mut sep = Slot::new(SlotKind::Sep, 0.3);
        sep.mag = false;
        slots.push(sep);
        slots.extend(zone2);
    }
    for (k, sl) in slots.iter_mut().enumerate() {
        sl.zone = if k >= idx_s { 2 } else if k >= idx_r { 1 } else { 0 };
    }
    d.slots = slots;
}

fn clock_texts(cfg: &Config) -> (String, String) {
    let n = local_now();
    let st = n.to_systemtime();
    let t = format_time_sys(&st);
    let dt = if cfg.general.show_date { format_date_sys(&st) } else { String::new() };
    (t, dt)
}

fn clock_len(cfg: &Config) -> f32 {
    let (t, d) = clock_texts(cfg);
    let g = gfx();
    let (w1, _) = g.measure(&t, 13.0, W_SEMIBOLD);
    let (w2, _) = g.measure(&d, 11.0, W_NORMAL);
    (w1.max(w2) + 14.0) / cfg.appearance.icon_size.max(20.0)
}

pub fn format_time_sys(st: &SYSTEMTIME) -> String {
    use windows::Win32::Globalization::*;
    unsafe {
        let mut buf = [0u16; 64];
        let n = GetTimeFormatEx(windows::core::PCWSTR::null(), TIME_NOSECONDS, Some(st), windows::core::PCWSTR::null(), Some(&mut buf));
        String::from_utf16_lossy(&buf[..(n.max(1) - 1) as usize])
    }
}

pub fn format_date_sys(st: &SYSTEMTIME) -> String {
    use windows::Win32::Globalization::*;
    unsafe {
        let mut buf = [0u16; 64];
        let n = GetDateFormatEx(windows::core::PCWSTR::null(), DATE_SHORTDATE, Some(st), windows::core::PCWSTR::null(), Some(&mut buf), windows::core::PCWSTR::null());
        String::from_utf16_lossy(&buf[..(n.max(1) - 1) as usize])
    }
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// Posições no eixo principal: início, comprimento, linha da grade
/// (-1 = fora da grade, 0 = de cima, 1 = de baixo) e o total.
fn place(slots: &[Slot], lens: &[f32], g: &Geo) -> (Vec<f32>, Vec<f32>, Vec<i8>, f32) {
    let n = slots.len();
    let mut starts = vec![0.0f32; n];
    let mut ls = lens.to_vec();
    let mut rows = vec![-1i8; n];
    let mut u = 0.0f32;
    let mut open: Option<usize> = None;
    for i in 0..n {
        if slots[i].grid {
            if let Some(j) = open.take() {
                let w = ls[j].max(lens[i]);
                ls[j] = w;
                ls[i] = w;
                starts[i] = starts[j];
                rows[i] = 1;
                u = starts[j] + w;
                continue;
            }
            if i > 0 {
                u += if slots[i - 1].small { g.sgap } else { g.gap };
            }
            starts[i] = u;
            rows[i] = 0;
            open = Some(i);
            u += lens[i];
            continue;
        }
        open = None;
        if i > 0 {
            u += if slots[i].small && slots[i - 1].small { g.sgap } else { g.gap };
        }
        starts[i] = u;
        u += lens[i];
    }
    (starts, ls, rows, u)
}

pub fn layout(d: &mut Dock, cfg: &Config, g: &Geo) {
    let s = g.s;
    let n = d.slots.len();
    if n == 0 {
        d.bar = Rect::default();
        return;
    }
    // posições base
    let lens0: Vec<f32> = d.slots.iter().map(|x| x.len * s).collect();
    let (base_u, base_len, _, inner0) = place(&d.slots, &lens0, g);
    let total0 = inner0 + 2.0 * g.pad_side;
    let start0 = (g.main - total0) / 2.0 + g.pad_side;
    // fatores de ampliação
    let mouse_u = d.mouse.map(|(x, y)| d.unmap(g, x, y).0);
    let hov = cfg.appearance.hover;
    let maxm = if hov == Hover::Magnify { cfg.appearance.magnify.clamp(1.0, 2.2) } else { 1.18 };
    let mut f = vec![1.0f32; n];
    if let Some(mu) = mouse_u {
        if d.mag > 0.001 && d.drag.is_none() {
            for i in 0..n {
                if !d.slots[i].mag {
                    continue;
                }
                let a = start0 + base_u[i];
                let b = a + base_len[i];
                match hov {
                    Hover::Magnify => {
                        let dist = if mu < a { a - mu } else if mu > b { mu - b } else { 0.0 } + s * 0.5;
                        let r = s * 2.6;
                        let w = if dist < r { ((std::f32::consts::PI * dist / r).cos() + 1.0) / 2.0 } else { 0.0 };
                        // ícones largos crescem menos para não empurrar demais
                        let k = if d.slots[i].len > 1.2 { 0.45 } else { 1.0 };
                        f[i] = 1.0 + (maxm - 1.0) * d.mag * w * k;
                    }
                    Hover::Scale => {
                        if mu >= a - g.gap / 2.0 && mu < b + g.gap / 2.0 {
                            f[i] = 1.0 + (maxm - 1.0) * d.mag;
                        }
                    }
                    Hover::None => {}
                }
            }
        }
    }
    let lens: Vec<f32> = (0..n).map(|i| d.slots[i].len * s * if hov == Hover::Magnify { f[i] } else { 1.0 }).collect();
    let (starts, ls, rows, inner) = place(&d.slots, &lens, g);
    let total = inner + 2.0 * g.pad_side;
    let start = (g.main - total) / 2.0;
    for i in 0..n {
        let u = start + g.pad_side + starts[i];
        let len_main = ls[i];
        let (cross, v0) = if rows[i] >= 0 {
            // grade de ícones de status: duas linhas na altura de um ícone
            (s * 0.5, g.margin + g.pad_bottom + if rows[i] == 0 { s * 0.5 } else { 0.0 })
        } else {
            (s * f[i], g.margin + g.pad_bottom)
        };
        let (draw_u, draw_len) = if hov == Hover::Scale && rows[i] < 0 {
            let grow = d.slots[i].len * s * (f[i] - 1.0);
            (u - grow / 2.0, len_main + grow)
        } else {
            (u, len_main)
        };
        let mut r = d.map_rect(g, draw_u, draw_len, v0, cross);
        if let Some(t0) = d.bounce.get(&d.slots[i].key) {
            let mut t = (tick_ms().saturating_sub(*t0)) as f32 / 1000.0;
            if d.launching.contains_key(&d.slots[i].key) {
                // pula em laço até a janela aparecer
                t %= 1.65;
            }
            let off = bounce_offset(cfg.appearance.launch_anim, t, s);
            let (dx, dy) = match g.pos {
                DockPos::Bottom => (0.0, -off.1),
                DockPos::Top => (0.0, off.1),
                DockPos::Left => (off.1, 0.0),
                DockPos::Right => (-off.1, 0.0),
            };
            r = r.offset(dx + off.0, dy);
        }
        d.slots[i].rect = r;
        d.slots[i].f = f[i];
    }
    d.bar = d.map_rect(g, start, total, g.margin, g.thick);
}

/// (deslocamento lateral, altura) da animação de abertura.
pub fn bounce_offset(kind: LaunchAnim, t: f32, s: f32) -> (f32, f32) {
    let per = 0.55;
    if t > per * 3.0 {
        return (0.0, 0.0);
    }
    let ph = (t % per) / per;
    match kind {
        LaunchAnim::Bounce => (0.0, s * 0.5 * (std::f32::consts::PI * ph).sin() * (1.0 - t / (per * 3.6))),
        LaunchAnim::Pulse => (0.0, s * 0.08 * (std::f32::consts::PI * 2.0 * ph).sin().abs()),
        LaunchAnim::Shake => (s * 0.08 * (std::f32::consts::PI * 4.0 * ph).sin(), 0.0),
    }
}

pub fn animating(d: &Dock) -> bool {
    (d.mag - d.mag_target).abs() > 0.001 || (d.hide - d.hide_target).abs() > 0.001 || d.bounce.values().any(|t| tick_ms().saturating_sub(*t) < 1700)
}

pub fn slot_at(d: &Dock, x: f32, y: f32, g: &Geo) -> Option<usize> {
    let (u, _) = d.unmap(g, x, y);
    let (_, bv) = d.unmap(g, d.bar.cx(), d.bar.cy());
    let _ = bv;
    // primeiro pelo retângulo, depois pela faixa do eixo principal dentro da barra
    for (i, s) in d.slots.iter().enumerate() {
        if s.rect.contains(x, y) {
            return Some(i);
        }
    }
    if d.bar.contains(x, y) {
        let mut best = None;
        let mut bd = f32::MAX;
        for (i, s) in d.slots.iter().enumerate() {
            let (a, _) = d.unmap(g, s.rect.x, s.rect.y);
            let (b, _) = d.unmap(g, s.rect.r(), s.rect.b());
            let (lo, hi) = (a.min(b), a.max(b));
            let dist = if u < lo { lo - u } else if u > hi { u - hi } else { 0.0 };
            if dist < bd {
                bd = dist;
                best = Some(i);
            }
        }
        if bd <= g.gap {
            return best;
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Desenho
// ---------------------------------------------------------------------------

pub struct Theme {
    pub fill: Color,
    pub border: Color,
    pub text: Color,
    pub glyph: Color,
    pub dark: bool,
}

pub fn theme(cfg: &Config) -> Theme {
    let dark = match cfg.appearance.tint {
        Tint::Dark => true,
        Tint::Light => false,
        Tint::Auto => glass::system_dark(),
    };
    let op = cfg.appearance.opacity.clamp(0.1, 0.95);
    if dark {
        Theme { fill: Color::hex(0x1C1C22).a(op), border: Color(1.0, 1.0, 1.0, 0.14), text: WHITE, glyph: WHITE.a(0.92), dark }
    } else {
        Theme { fill: Color::hex(0xF4F4F8).a(op * 0.95), border: Color(1.0, 1.0, 1.0, 0.55), text: Color::rgba(20, 20, 26, 1.0), glyph: Color::rgba(20, 20, 26, 0.9), dark }
    }
}

pub fn draw_start_icon(c: &Canvas, r: Rect, plate: bool) {
    let rad = r.w * 0.225;
    if plate {
        c.fill_rrect_vgrad(r, rad, &[(0.0, Color::hex(0x3AA0FF)), (1.0, Color::hex(0x0A5FD8))]);
        c.fill_rrect_vgrad(r, rad, &[(0.0, Color(1.0, 1.0, 1.0, 0.25)), (0.5, Color(1.0, 1.0, 1.0, 0.0))]);
        c.stroke_rrect(r.inset(0.5), rad, Color(1.0, 1.0, 1.0, 0.25), 1.0);
    }
    let q = r.w * 0.17;
    let gap = r.w * 0.025;
    let cx = r.cx();
    let cy = r.cy();
    for (dx, dy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let x = if dx < 0.0 { cx - gap - q } else { cx + gap };
        let y = if dy < 0.0 { cy - gap - q } else { cy + gap };
        c.fill_rrect(Rect::new(x, y, q, q), q * 0.12, if plate { WHITE } else { Color::hex(0x2E9BFF) });
    }
}

#[derive(Clone, Copy)]
pub struct Plate {
    pub on: bool,
    pub all: bool,
    pub color: Color,
}

pub fn plate_cfg(cfg: &Config) -> Plate {
    let hex = cfg.appearance.plate_color.trim_start_matches('#');
    let color = u32::from_str_radix(hex, 16).map(Color::hex).unwrap_or(WHITE);
    Plate { on: cfg.appearance.plates, all: cfg.appearance.plate_all, color }
}

pub fn draw_plate(c: &Canvas, r: Rect, col: Color) {
    let rad = r.w * 0.225;
    c.fill_rrect_vgrad(r, rad, &[(0.0, col.mix(WHITE, 0.08)), (1.0, col.mix(BLACK, 0.07))]);
    let edge = if col.luminance() > 0.5 { Color(0.0, 0.0, 0.0, 0.10) } else { Color(1.0, 1.0, 1.0, 0.16) };
    c.stroke_rrect(r.inset(0.5), rad, edge, 1.0);
}

pub fn draw_app_icon(c: &Canvas, ic: Option<&icons::Icon>, r: Rect, plate: Plate, _dark: bool, fallback: &str) {
    match ic {
        Some(ic) => {
            if plate.on && (plate.all || !ic.full) {
                draw_plate(c, r, plate.color);
                c.bitmap(&ic.bmp, r.inset(r.w * 0.15), 1.0);
            } else {
                c.bitmap(&ic.bmp, r, 1.0);
            }
        }
        None => {
            c.fill_rrect_vgrad(r, r.w * 0.225, &[(0.0, Color::hex(0x8E8E93)), (1.0, Color::hex(0x636366))]);
            let ch: String = fallback.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
            c.text(&ch, r, r.h * 0.5, W_SEMIBOLD, WHITE, Align::Center);
        }
    }
}

/// Ícone do app de um item, carregando do cache (ou da janela, se preciso).
pub fn slot_icon(sh: &mut Shared, sl: &Slot, px: f32) -> Option<icons::Icon> {
    if sl.icon.is_empty() {
        return None;
    }
    if let Some(i) = sh.icons.get(&sl.icon, px as u32) {
        return Some(i);
    }
    let key = format!("hwnd:{}", sl.key);
    if let Some(i) = sh.icons.get_hicon_cached(&key) {
        return Some(i);
    }
    let h = *sl.wins.first()?;
    let hi = icons::window_hicon(h)?;
    sh.icons.put_hicon(&key, hi)
}

pub struct DrawCtx<'a> {
    pub cfg: &'a Config,
    pub widgets: &'a mut BTreeMap<u32, Box<dyn Widget>>,
    pub sh: &'a mut Shared,
    pub trash_full: bool,
}

pub fn render(d: &mut Dock, ctx: &mut DrawCtx) {
    render_dirty(d, ctx, &[]);
}

/// Redesenha; se `only_widgets` não estiver vazio e nada mais mudou, só os
/// tiles desses widgets são refeitos.
pub fn render_dirty(d: &mut Dock, ctx: &mut DrawCtx, only_widgets: &[u32]) {
    let cfg = ctx.cfg;
    let g = d.geo(cfg);
    d.s = g.s;
    layout(d, cfg, &g);
    let wr = d.window_rect(cfg, &g);
    let same_window = wr == d.win;
    d.win = wr;
    let mut dirty: Option<Rect> = None;
    if !only_widgets.is_empty() && same_window && d.mouse.is_none() && d.drag.is_none() {
        for sl in &d.slots {
            if sl.kind == SlotKind::Widget && only_widgets.contains(&sl.widget) {
                let r = sl.rect.inset(-g.s * 0.22);
                dirty = Some(match dirty {
                    None => r,
                    Some(u) => {
                        let x0 = u.x.min(r.x);
                        let y0 = u.y.min(r.y);
                        Rect::new(x0, y0, u.r().max(r.r()) - x0, u.b().max(r.b()) - y0)
                    }
                });
            }
        }
        if dirty.is_none() {
            return;
        }
    }
    let th = theme(cfg);
    let plate = plate_cfg(cfg);
    let (w, h) = (wr.right - wr.left, wr.bottom - wr.top);
    let ws_color = cfg.ws_index(d.ws).map(|i| Color::hex(WS_COLORS[cfg.workspaces[i].color as usize % 6].0)).unwrap_or(ACCENT);
    let hover = d.hover;
    let launching: Vec<String> = d.launching.keys().cloned().collect();
    let dragging = d.drag.as_ref().map(|x| (x.slot, x.u, x.v, x.out, x.resize));
    let drop_hover = d.drop_hover;
    let kb = d.kb_focus;
    let slots = d.slots.clone();
    let bar = d.bar;
    let mouse = d.mouse;
    let vertical = d.vertical(cfg);
    let mut tray_rects: Vec<(String, Rect)> = vec![];
    let cur_ws = d.ws;
    let mut label: Option<(Rect, String)> = None;
    let pos = g.pos;
    let hwnd = d.hwnd;
    let ctx_sh = &mut *ctx.sh;
    let widgets = &mut *ctx.widgets;
    let trash_full = ctx.trash_full;
    d.surf.paint_dirty(hwnd, wr.left, wr.top, w, h, dirty, |c| {
        if bar.w <= 0.0 {
            return;
        }
        let rad = (g.s * 0.4 + 9.0 * g.sc).min(bar.h.min(bar.w) / 2.0);
        // Faixa quase invisível que segura o mouse só onde os ícones
        // ampliados passam da barra (fora disso, os cliques vão para a tela).
        if mouse.is_some() && cfg.appearance.hover == Hover::Magnify {
            let mut over = 0.0f32;
            for sl in slots.iter().filter(|s| s.mag) {
                let o = match pos {
                    DockPos::Bottom => bar.y - sl.rect.y,
                    DockPos::Top => sl.rect.b() - bar.b(),
                    DockPos::Left => sl.rect.r() - bar.r(),
                    DockPos::Right => bar.x - sl.rect.x,
                };
                over = over.max(o);
            }
            if over > 1.0 {
                let hold = match pos {
                    DockPos::Bottom => Rect::new(bar.x, bar.y - over, bar.w, over + 1.0),
                    DockPos::Top => Rect::new(bar.x, bar.b() - 1.0, bar.w, over + 1.0),
                    DockPos::Left => Rect::new(bar.r() - 1.0, bar.y, over + 1.0, bar.h),
                    DockPos::Right => Rect::new(bar.x - over, bar.y, over + 1.0, bar.h),
                };
                c.fill_rect(hold, Color(0.0, 0.0, 0.0, 1.0 / 255.0));
            }
        }
        // sombra só do lado da borda da tela, para não bloquear cliques em volta
        if dirty.is_none() {
            let clip = match pos {
                DockPos::Bottom => Rect::new(bar.x, bar.cy(), bar.w, w as f32),
                DockPos::Top => Rect::new(bar.x, 0.0, bar.w, bar.cy()),
                DockPos::Left => Rect::new(0.0, bar.y, bar.cx(), bar.h),
                DockPos::Right => Rect::new(bar.cx(), bar.y, w as f32, bar.h),
            };
            c.push_clip(clip);
            c.shadow(bar.inset(4.0 * g.sc), rad, 10.0 * g.sc, 4.0 * g.sc, Color(0.0, 0.0, 0.0, 0.32));
            c.pop_axis_clip();
        }
        c.fill_rrect(bar, rad, th.fill);
        c.fill_rrect_vgrad(bar, rad, &[(0.0, Color(1.0, 1.0, 1.0, if th.dark { 0.10 } else { 0.30 })), (0.5, Color(1.0, 1.0, 1.0, 0.0))]);
        c.stroke_rrect(bar.inset(0.5), rad, th.border, 1.0);
        let hl = Rect::new(bar.x + rad * 0.6, bar.y + 1.0, bar.w - rad * 1.2, 1.0);
        if !vertical {
            c.fill_rect(hl, Color(1.0, 1.0, 1.0, 0.10));
        }
        let first = slots.iter().position(|s| !matches!(s.kind, SlotKind::Sep | SlotKind::Spacer));
        let last = slots.iter().rposition(|s| !matches!(s.kind, SlotKind::Sep | SlotKind::Spacer));
        let end_k = (rad - g.pad_side.min(g.pad_top)).max(0.0);
        for (i, sl) in slots.iter().enumerate() {
            c.shape.set(None);
            if Some(i) == first || Some(i) == last {
                let r0 = sl.rect;
                let k = if Some(i) == first {
                    if vertical { [end_k, end_k, 0.0, 0.0] } else { [end_k, 0.0, 0.0, end_k] }
                } else if vertical {
                    [0.0, 0.0, end_k, end_k]
                } else {
                    [0.0, end_k, end_k, 0.0]
                };
                let k = if first == last { [end_k; 4] } else { k };
                c.shape.set(Some((r0, k)));
            }
            if let Some((ds, ..)) = dragging {
                if ds == i && !dragging.unwrap().4 {
                    continue;
                }
            }
            let r = sl.rect;
            if let Some(dr) = dirty {
                let m = g.s * 0.25;
                if r.r() + m < dr.x || r.x - m > dr.r() || r.b() + m < dr.y || r.y - m > dr.b() {
                    continue;
                }
            }
            let is_hover = hover == Some(i) || kb == Some(i);
            match sl.kind {
                SlotKind::Sep => {
                    let (a, b) = if vertical {
                        ((r.x + r.w * 0.2, r.cy()), (r.r() - r.w * 0.2, r.cy()))
                    } else {
                        ((r.cx(), r.y + r.h * 0.12), (r.cx(), r.b() - r.h * 0.12))
                    };
                    let col = if th.dark { Color(1.0, 1.0, 1.0, 0.22) } else { Color(0.0, 0.0, 0.0, 0.2) };
                    c.line(a.0, a.1, b.0, b.1, col, 1.0);
                }
                SlotKind::Spacer => {}
                SlotKind::WsDots => {
                    let n = cfg.workspaces.len().max(1);
                    for (k, ws) in cfg.workspaces.iter().enumerate() {
                        let col = Color::hex(WS_COLORS[ws.color as usize % 6].0);
                        let t = (k as f32 + 0.5) / n as f32;
                        let (x, y) = if vertical { (r.cx(), r.y + r.h * t) } else { (r.x + r.w * t, r.cy()) };
                        let active = ws.id == cur_ws;
                        let rr = g.s * if active { 0.1 } else { 0.07 };
                        c.fill_circle(x, y, rr, if active { col } else { col.a(0.45) });
                        if active {
                            c.stroke_circle(x, y, rr + 2.0 * g.sc, col.a(0.5), 1.0);
                        }
                    }
                    if is_hover {
                        label = Some((r, cfg.ws_index(cur_ws).map(|i| cfg.workspaces[i].name.clone()).unwrap_or_default()));
                    }
                }
                SlotKind::Start => {
                    draw_start_icon(c, r, true);
                }
                SlotKind::Pinned | SlotKind::Running => {
                    let ic = slot_icon(ctx_sh, sl, g.s * cfg.appearance.magnify.max(1.0));
                    c.shadow(r.inset(r.w * 0.08), r.w * 0.2, 5.0 * g.sc, 4.0 * g.sc, Color(0.04, 0.05, 0.12, 0.22));
                    draw_app_icon(c, ic.as_ref(), r, plate, th.dark, &sl.name);
                    let ind_col = if cfg.appearance.indicator_ws_color { ws_color } else { th.glyph };
                    if !sl.wins.is_empty() {
                        draw_indicator(c, cfg, &g, d_pos(pos), r, sl, ind_col, &bar);
                    } else if launching.contains(&sl.key) {
                        // abrindo: o indicador pulsa até a janela aparecer
                        let ph = (tick_ms() % 1100) as f32 / 1100.0;
                        let a = 0.3 + 0.7 * (ph * std::f32::consts::PI).sin().abs();
                        let mut tmp = sl.clone();
                        tmp.active = false;
                        tmp.wins = vec![HWND::default()];
                        draw_indicator(c, cfg, &g, d_pos(pos), r, &tmp, ind_col.a(a), &bar);
                    }
                }
                SlotKind::Widget => {
                    if let Some(wd) = widgets.get_mut(&sl.widget) {
                        let style = cfg.widget(sl.widget).map(|w| w.style).unwrap_or(0);
                        let name = cfg.widget(sl.widget).map(|w| w.name.clone()).unwrap_or_default();
                        let s_now = if vertical { r.w } else { r.h };
                        let tile = Tile { c, r, s: s_now, units: sl.units, style, hover: is_hover, name: &name };
                        ctx_sh.current_widget = sl.widget;
                        c.shadow(r.inset(r.h * 0.06), r.h * 0.2, 5.0 * g.sc, 4.0 * g.sc, Color(0.04, 0.05, 0.12, 0.2));
                        wd.draw(&tile, ctx_sh);
                        if sl.stack > 1 {
                            // pontinhos da pilha
                            let cur = match cfg.workspaces.get(cfg.ws_index(cur_ws).unwrap_or(0)).and_then(|w| sl.item.and_then(|i| w.items.get(i))) {
                                Some(DockItem::Widgets { cur, .. }) => *cur,
                                _ => 0,
                            };
                            // embaixo da pilha, na linha dos indicadores de app aberto
                            let off = g.pad_bottom * 0.45 + g.margin;
                            let (cx, cy) = match pos {
                                DockPos::Bottom => (r.cx(), bar.b() - off + g.margin * 0.4),
                                DockPos::Top => (r.cx(), bar.y + off - g.margin * 0.4),
                                DockPos::Left => (bar.x + off - g.margin * 0.4, r.cy()),
                                DockPos::Right => (bar.r() - off + g.margin * 0.4, r.cy()),
                            };
                            let dot = (g.s * 0.035).max(1.6 * g.sc);
                            let step = dot * 3.2;
                            let n = sl.stack as f32;
                            for k in 0..sl.stack {
                                let o = (k as f32 - (n - 1.0) / 2.0) * step;
                                let (x, y) = if vertical { (cx, cy + o) } else { (cx + o, cy) };
                                c.fill_circle(x, y, if k == cur { dot * 1.15 } else { dot }, if k == cur { th.glyph } else { th.glyph.a(0.35) });
                            }
                        }
                    }
                }
                SlotKind::Folder | SlotKind::Downloads => {
                    if let Some(ic) = ctx_sh.icons.get(&sl.icon, (g.s * cfg.appearance.magnify.max(1.0)) as u32) {
                        if plate.on && plate.all {
                            draw_plate(c, r, plate.color);
                            c.bitmap(&ic.bmp, r.inset(r.w * 0.15), 1.0);
                        } else {
                            c.bitmap(&ic.bmp, r, 1.0);
                        }
                    }
                    if sl.kind == SlotKind::Downloads {
                        let b = r.w * 0.36;
                        let br = Rect::new(r.r() - b, r.b() - b, b, b);
                        c.fill_circle(br.cx(), br.cy(), b / 2.0, ACCENT);
                        c.glyph(0xE896, br, b * 0.55, WHITE);
                    }
                }
                SlotKind::Trash => {
                    let key = if trash_full { "trash:full" } else { "trash:empty" };
                    match ctx_sh.icons.get_keyed(key, "shell:RecycleBinFolder", (g.s * cfg.appearance.magnify.max(1.0)) as u32) {
                        Some(ic) => c.bitmap(&ic.bmp, r, 1.0),
                        None => c.glyph(0xE74D, r, r.h * 0.6, th.glyph),
                    }
                }
                SlotKind::Tray => {
                    let ics = tray::visible_icons();
                    if let Some(ti) = ics.iter().find(|t| t.key == sl.key) {
                        let key = format!("tray:{}:{}", ti.key, ti.hicon.0 as usize);
                        let bmp = ctx_sh.icons.get_hicon_cached(&key).or_else(|| ctx_sh.icons.put_hicon(&key, ti.hicon));
                        let d = (g.s * 0.42).min(r.h * 0.82).round();
                        let ir = r.centered(d, d);
                        if is_hover {
                            c.fill_rrect(r.centered(r.w, (g.s * 0.62).min(r.h)), 6.0 * g.sc, th.glyph.a(0.12));
                        }
                        if let Some(b) = bmp {
                            c.bitmap(&b.bmp, ir, 1.0);
                        }
                        tray_rects.push((ti.key.clone(), r));
                        if is_hover && !ti.tip.is_empty() {
                            label = Some((r, ti.tip.lines().next().unwrap_or("").to_string()));
                        }
                    }
                }
                SlotKind::Chevron => {
                    if is_hover {
                        c.fill_rrect(r.centered(r.w, (g.s * 0.62).min(r.h)), 6.0 * g.sc, th.glyph.a(0.12));
                    }
                    let gl = match pos {
                        DockPos::Bottom => 0xE70E,
                        DockPos::Top => 0xE70D,
                        DockPos::Left => 0xE76C,
                        DockPos::Right => 0xE76B,
                    };
                    c.glyph(gl, r, (g.s * 0.26).min(r.h * 0.6), th.glyph);
                }
                SlotKind::Net | SlotKind::Vol | SlotKind::Bat => {
                    if is_hover {
                        c.fill_rrect(r.centered(r.w, (g.s * 0.62).min(r.h)), 6.0 * g.sc, th.glyph.a(0.12));
                    }
                    let st = &ctx_sh.status;
                    let (gl, tip) = match sl.kind {
                        SlotKind::Net => (
                            st.net_glyph(),
                            match st.net {
                                Net::Wifi(q) => format!("{} · {}%", if st.ssid.is_empty() { "Wi-Fi".into() } else { st.ssid.clone() }, q),
                                Net::Ethernet => "Rede cabeada".into(),
                                Net::None => "Sem internet".into(),
                            },
                        ),
                        SlotKind::Vol => (st.vol_glyph(), if st.muted { "Sem som".into() } else { format!("Volume {:.0}%", st.vol * 100.0) }),
                        _ => (st.battery_glyph(), format!("Bateria {}%{}", st.battery.pct, if st.battery.charging { " · carregando" } else { "" })),
                    };
                    c.glyph(gl, r, (g.s * 0.34).min(r.h * 0.66), th.glyph);
                    if is_hover {
                        label = Some((r, tip));
                    }
                }
                SlotKind::Clock => {
                    if is_hover {
                        c.fill_rrect(r.centered(r.w, g.s * 0.8), 6.0 * g.sc, th.glyph.a(0.12));
                    }
                    let (t, dt) = clock_texts(cfg);
                    let al = match cfg.general.clock_align {
                        0 => Align::Left,
                        1 => Align::Center,
                        _ => Align::Right,
                    };
                    let inner = r.inset_xy(4.0 * g.sc, 0.0);
                    if vertical {
                        c.text(&t, Rect::new(r.x, r.cy() - g.s * 0.3, r.w, g.s * 0.3), g.s * 0.24, W_SEMIBOLD, th.text, Align::Center);
                        let n = local_now();
                        c.text(&format!("{:02}/{:02}", n.day, n.month), Rect::new(r.x, r.cy(), r.w, g.s * 0.28), g.s * 0.2, W_NORMAL, th.text.a(0.75), Align::Center);
                    } else if dt.is_empty() {
                        c.text(&t, inner, 13.5 * g.sc, W_SEMIBOLD, th.text, al);
                    } else {
                        let lh = 17.0 * g.sc;
                        c.text(&t, Rect::new(inner.x, r.cy() - lh, inner.w, lh), 13.0 * g.sc, W_SEMIBOLD, th.text, al);
                        c.text(&dt, Rect::new(inner.x, r.cy(), inner.w, lh), 11.0 * g.sc, W_NORMAL, th.text.a(0.75), al);
                    }
                }
            }
            c.shape.set(None);
            if drop_hover == Some(i) {
                c.stroke_rrect(r.inset(-3.0 * g.sc), r.h.min(r.w) * 0.25, ACCENT, 2.0 * g.sc);
            }
            if kb == Some(i) {
                c.stroke_rrect(r.inset(-3.0 * g.sc), r.h.min(r.w) * 0.25, WHITE.a(0.9), 2.0 * g.sc);
            }
            if is_hover && cfg.appearance.labels && label.is_none() && dragging.is_none() && !sl.name.is_empty() && !matches!(sl.kind, SlotKind::Spacer | SlotKind::Sep | SlotKind::Widget) {
                label = Some((r, sl.name.clone()));
            }
        }
        // item arrastado
        if let Some((ds, u, v, out, resize)) = dragging {
            if !resize {
                if let Some(sl) = slots.get(ds) {
                    let len = sl.len * g.s;
                    let (x, y) = d_map(&g, u, v);
                    let rr = if vertical { Rect::new(x - g.s / 2.0, y - len / 2.0, g.s, len) } else { Rect::new(x - len / 2.0, y - g.s / 2.0, len, g.s) };
                    let a = if out { 0.45 } else { 0.92 };
                    match sl.kind {
                        SlotKind::Widget => {
                            if let Some(wd) = widgets.get_mut(&sl.widget) {
                                let style = cfg.widget(sl.widget).map(|w| w.style).unwrap_or(0);
                                let tile = Tile { c, r: rr, s: g.s, units: sl.units, style, hover: false, name: "" };
                                wd.draw(&tile, ctx_sh);
                            }
                        }
                        SlotKind::Start => draw_start_icon(c, rr, true),
                        SlotKind::Spacer => c.stroke_rrect(rr.inset(2.0), 8.0 * g.sc, th.glyph.a(0.5), 1.5),
                        _ => {
                            let ic = slot_icon(ctx_sh, sl, g.s * cfg.appearance.magnify.max(1.0));
                            let ic2 = if sl.kind == SlotKind::Folder { ctx_sh.icons.get(&sl.icon, (g.s * cfg.appearance.magnify.max(1.0)) as u32) } else { ic };
                            draw_app_icon(c, ic2.as_ref(), rr, plate, th.dark, &sl.name);
                        }
                    }
                    if out {
                        c.fill_rrect(rr, rr.h.min(rr.w) * 0.22, Color(0.0, 0.0, 0.0, 1.0 - a));
                        let b = g.s * 0.34;
                        let br = Rect::new(rr.r() - b * 0.7, rr.y - b * 0.3, b, b);
                        c.fill_circle(br.cx(), br.cy(), b / 2.0, RED);
                        c.glyph(0xE738, br, b * 0.5, WHITE);
                    }
                }
            }
        }
        // nome em cima do item
        if let Some((r, text)) = label {
            let fs = 12.5 * g.sc;
            let (tw, _) = gfx().measure(&text, fs, W_MEDIUM);
            let pw = (tw + 22.0 * g.sc).min(420.0 * g.sc);
            let ph = 26.0 * g.sc;
            let gap = 10.0 * g.sc;
            let pr = match pos {
                DockPos::Bottom => Rect::new(r.cx() - pw / 2.0, r.y - gap - ph, pw, ph),
                DockPos::Top => Rect::new(r.cx() - pw / 2.0, r.b() + gap, pw, ph),
                DockPos::Left => Rect::new(r.r() + gap, r.cy() - ph / 2.0, pw, ph),
                DockPos::Right => Rect::new(r.x - gap - pw, r.cy() - ph / 2.0, pw, ph),
            };
            let pr = Rect::new(pr.x.max(4.0).min(w as f32 - pw - 4.0), pr.y.max(2.0), pw, ph);
            c.fill_rrect(pr, ph / 2.0, Color::rgba(28, 28, 32, 0.9));
            c.stroke_rrect(pr.inset(0.5), ph / 2.0, Color(1.0, 1.0, 1.0, 0.14), 1.0);
            c.text(&text, pr.inset_xy(10.0 * g.sc, 0.0), fs, W_MEDIUM, WHITE, Align::Center);
        }
    });
    // posições reais dos ícones da bandeja (para os menus dos apps abrirem no lugar)
    for (k, r) in tray_rects {
        tray::set_rect(&k, RECT { left: wr.left + r.x as i32, top: wr.top + r.y as i32, right: wr.left + r.r() as i32, bottom: wr.top + r.b() as i32 });
    }
}

fn d_pos(p: DockPos) -> DockPos {
    p
}

fn d_map(g: &Geo, u: f32, v: f32) -> (f32, f32) {
    let u = u - g.u0;
    match g.pos {
        DockPos::Bottom => (u, g.h - v),
        DockPos::Top => (u, v),
        DockPos::Left => (v, u),
        DockPos::Right => (g.h - v, u),
    }
}

fn draw_indicator(c: &Canvas, cfg: &Config, g: &Geo, pos: DockPos, r: Rect, sl: &Slot, col: Color, bar: &Rect) {
    let col = if sl.flashing { ORANGE } else { col };
    let d = (g.s * 0.07).max(3.0 * g.sc);
    let off = g.pad_bottom * 0.45 + g.margin;
    let (cx, cy) = match pos {
        DockPos::Bottom => (r.cx(), bar.b() - off + g.margin * 0.4),
        DockPos::Top => (r.cx(), bar.y + off - g.margin * 0.4),
        DockPos::Left => (bar.x + off - g.margin * 0.4, r.cy()),
        DockPos::Right => (bar.r() - off + g.margin * 0.4, r.cy()),
    };
    match cfg.appearance.indicator {
        Indicator::Dot => {
            c.fill_circle(cx, cy, d / 2.0 + if sl.active { 0.5 } else { 0.0 }, col.a(if sl.active { 1.0 } else { 0.75 }));
            if sl.wins.len() > 1 {
                let k = d * 1.4;
                let (x2, y2) = if matches!(pos, DockPos::Left | DockPos::Right) { (cx, cy + k) } else { (cx + k, cy) };
                c.fill_circle(x2, y2, d / 2.5, col.a(0.6));
            }
        }
        Indicator::Dash => {
            let len = if sl.active { g.s * 0.36 } else { g.s * 0.2 };
            if matches!(pos, DockPos::Left | DockPos::Right) {
                c.line(cx, cy - len / 2.0, cx, cy + len / 2.0, col.a(if sl.active { 1.0 } else { 0.75 }), d * 0.8);
            } else {
                c.line(cx - len / 2.0, cy, cx + len / 2.0, cy, col.a(if sl.active { 1.0 } else { 0.75 }), d * 0.8);
            }
        }
    }
}

pub use windows::Win32::Foundation::SYSTEMTIME;
