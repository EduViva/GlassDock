// O app: estado global, laço de mensagens e todas as ações.
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Threading::*;
use windows::Win32::Storage::FileSystem::{FindCloseChangeNotification, FindNextChangeNotification};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::*;
use crate::dock::{self, Dock, SlotKind};
use crate::gfx::*;
use crate::glass::{self, *};
use crate::icons;
use crate::popup::*;
use crate::status::Status;
use crate::sysmon::SysMon;
use crate::taskbar::{self, Taskbar};
use crate::tray;
use crate::ui::*;
use crate::util::*;
use crate::widgets::{self, Shared, Widget};
use crate::winlist::{self, WinList};

pub const WM_APP_POLL: u32 = WM_APP + 1;
pub const WM_APP_TRAY: u32 = WM_APP + 2;
pub const WM_APP_WINS: u32 = WM_APP + 3;
pub const WM_APP_STATUS: u32 = WM_APP + 4;
pub const WM_APP_MEDIA: u32 = WM_APP + 5;
pub const WM_APP_FG: u32 = WM_APP + 6;
pub const WM_APP_TASKBAR: u32 = WM_APP + 7;
pub const WM_APP_APPBAR: u32 = WM_APP + 8;
pub const WM_APP_SHOW_SETTINGS: u32 = WM_APP + 9;
pub const WM_APP_LAUNCHFAIL: u32 = WM_APP + 10;

const T_ANIM: usize = 1;
const T_TICK: usize = 2;
const T_SAVE: usize = 3;
const T_WINS: usize = 4;
const T_HIDE: usize = 5;
const T_PREVIEW: usize = 6;
const T_SLOW: usize = 7;
const T_PET: usize = 8;
const T_PANEL: usize = 9;
const T_PREVIEW_HIDE: usize = 10;
const T_SHOW: usize = 11;
const T_TOAST: usize = 12;
const T_FOLLOW: usize = 13;
const T_DUMP: usize = 20;
const T_CLICK: usize = 21;
const T_EDGE: usize = 22;

static MAIN: AtomicIsize = AtomicIsize::new(0);

pub fn post(msg: u32, w: usize, l: isize) {
    let h = MAIN.load(Ordering::Relaxed);
    if h != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(h as _)), msg, WPARAM(w), LPARAM(l));
        }
    }
}

thread_local! {
    pub static DUMP: RefCell<Option<String>> = const { RefCell::new(None) };
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    static DEFERRED: RefCell<Vec<Box<dyn FnOnce()>>> = RefCell::new(vec![]);
    static WATCH: RefCell<Vec<HANDLE>> = const { RefCell::new(vec![]) };
}

pub fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| {
        let mut b = a.try_borrow_mut().ok()?;
        let app = b.as_mut()?;
        Some(f(app))
    })
}

pub fn defer(f: impl FnOnce() + 'static) {
    DEFERRED.with(|d| d.borrow_mut().push(Box::new(f)));
}

fn run_deferred() {
    loop {
        let next = DEFERRED.with(|d| {
            let mut v = d.borrow_mut();
            if v.is_empty() {
                None
            } else {
                Some(v.remove(0))
            }
        });
        match next {
            Some(f) => f(),
            None => break,
        }
    }
}

// ---------------------------------------------------------------------------
// Tipos auxiliares
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub enum PanelKind {
    Widget(u32),
    Folder(String, bool),
    Tray,
    Calendar,
}

pub struct Panel {
    pub p: Popup,
    pub kind: PanelKind,
    pub dock: usize,
    pub anchor: RECT,
    pub key: String,
    pub modal: bool,
    pub opened: u64,
    pub scroll_target: Option<usize>,
    pub menu_target: Option<String>,
    pub drag_from: Option<(String, f32, f32)>,
}

#[derive(Clone)]
pub enum Act {
    Activate(isize),
    NewWindow(PinnedApp),
    Pin(PinnedApp, String),
    Unpin(usize),
    MoveTo(Vec<isize>, u32),
    Close(Vec<isize>),
    Quit(String),
    Kill(Vec<u32>),
    Explorer(String),
    WidgetMenu(u32, u32),
    EditWidget(u32),
    StackWith(usize, u32),
    Unstack(usize),
    UnstackAll(usize),
    RemoveItem(usize),
    AddSpacer(bool),
    AddDivider,
    ToggleAutohide,
    SetHover(Hover),
    ToggleRunning,
    SwitchWs(u32),
    Widgets,
    Settings,
    NewWs,
    Exit,
    EmptyTrash,
    Open(String),
    FolderList(usize, bool),
    FolderSort(usize, u32),
    Keys(Vec<u16>),
    TrayPin(String, bool),
    FileMenu(u32),
    HideApp(String),
    TrayPanel,
}

pub enum MItem {
    Item(String, Act, bool),
    Disabled(String),
    Sub(String, Vec<MItem>),
    Sep,
}

fn mi(t: &str, a: Act) -> MItem {
    MItem::Item(t.into(), a, false)
}

fn mic(t: &str, a: Act, checked: bool) -> MItem {
    MItem::Item(t.into(), a, checked)
}

// ---------------------------------------------------------------------------
// Estado do app
// ---------------------------------------------------------------------------

pub struct App {
    pub cfg: Config,
    pub sh: Shared,
    pub widgets: BTreeMap<u32, Box<dyn Widget>>,
    pub docks: Vec<Dock>,
    pub wins: WinList,
    pub taskbar: Taskbar,
    pub panel: Option<Panel>,
    pub preview: Option<Preview>,
    pub settings: Option<crate::settings::SettingsWin>,
    pub toasts: Vec<ToastWin>,
    pub eyedrop: Option<Eyedrop>,
    pub main: HWND,
    pub shellhook_msg: u32,
    pub taskbar_created_msg: u32,
    hooks: Vec<HWINEVENTHOOK>,
    due: HashMap<u32, u64>,
    pub trash_full: bool,
    pending_launch: Option<PendingLaunch>,
    preview_pending: Option<(usize, usize)>,
    watch_ids: Vec<u32>,
    hotkeys: Vec<i32>,
    pub clock_cal: Option<Box<dyn Widget>>,
    pub menu_open: bool,
    last_minute: u32,
    follow: Vec<(HWND, HMONITOR)>,
    follow_pending: Option<HMONITOR>,
    toast_serial: usize,
    pet_tick: u64,
    explorer_pid: u32,
    pub quitting: bool,
    last_panel_close: (String, u64),
    sys_last: u64,
    media_started: bool,
    last_rebroadcast: u64,
    watchdog_started: bool,
    pending_click: Option<(usize, usize, f32, f32, u32)>,
}

/// Um app fixado que acabou de ser aberto pelo clique.
#[derive(Clone)]
pub struct PendingLaunch {
    /// chave usada para reconhecer janelas novas (AUMID ou nome do exe)
    pub k: String,
    pub t: u64,
    pub ws: u32,
    /// `launch` do app fixado (para guardar o que for aprendido)
    pub launch: String,
    pub pinned: bool,
}

pub fn run() {
    register_class(glass::CLASS, Some(wndproc));
    let main = glass::create(K_MAIN, 0, WS_EX_TOOLWINDOW, WS_POPUP, None, "GlassDockMain");
    MAIN.store(main.0 as isize, Ordering::Relaxed);
    taskbar::recover_on_start();
    let app = App::new(main);
    APP.with(|a| *a.borrow_mut() = Some(app));
    with_app(|a| a.start());
    run_deferred();
    unsafe {
        let mut msg = MSG::default();
        'outer: loop {
            let handles: Vec<HANDLE> = WATCH.with(|w| w.borrow().clone());
            let r = MsgWaitForMultipleObjectsEx(Some(&handles), INFINITE, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            let idx = r.0 as usize;
            if idx < handles.len() {
                let _ = FindNextChangeNotification(handles[idx]);
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_app(|a| a.on_watch(idx));
                    run_deferred();
                }));
            }
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    break 'outer;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
    with_app(|a| a.shutdown());
}

unsafe extern "system" fn wndproc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let t = GetWindowLongPtrW(h, GWLP_USERDATA);
    let kind = tag_kind(t);
    // respostas que não podem esperar pelo estado do app
    match msg {
        WM_MOUSEACTIVATE => {
            if matches!(kind, K_DOCK | K_PLATE | K_TRIGGER | K_PREVIEW | K_PREVIEW_PLATE | K_TOAST | K_PET | K_PANEL_PLATE | K_SETTINGS_PLATE) {
                return LRESULT(MA_NOACTIVATE as isize);
            }
        }
        WM_NCHITTEST if kind == K_PET || kind == K_MAGNIFIER => return LRESULT(HTTRANSPARENT as isize),
        WM_SETCURSOR if kind == K_EYEDROP => {
            SetCursor(LoadCursorW(None, IDC_CROSS).ok());
            return LRESULT(1);
        }
        WM_ERASEBKGND if !matches!(kind, K_PLATE | K_PANEL_PLATE | K_PREVIEW_PLATE | K_SETTINGS_PLATE) => return LRESULT(1),
        WM_PAINT if matches!(kind, K_PLATE | K_PANEL_PLATE | K_PREVIEW_PLATE | K_SETTINGS_PLATE) => {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(h, &mut ps);
            FillRect(dc, &ps.rcPaint, HBRUSH(GetStockObject(BLACK_BRUSH).0));
            let _ = EndPaint(h, &ps);
            return LRESULT(0);
        }
        _ => {}
    }
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| with_app(|a| a.on_message(h, kind, tag_idx(t), msg, wp, lp)).flatten())).unwrap_or(None);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run_deferred));
    match r {
        Some(v) => v,
        None => DefWindowProcW(h, msg, wp, lp),
    }
}

unsafe extern "system" fn win_event(_h: HWINEVENTHOOK, event: u32, hwnd: HWND, id_obj: i32, _id_child: i32, _t: u32, _ms: u32) {
    if id_obj != 0 {
        return;
    }
    match event {
        EVENT_SYSTEM_FOREGROUND => post(WM_APP_FG, hwnd.0 as usize, 0),
        EVENT_OBJECT_SHOW => {
            let mut cls = [0u16; 32];
            let n = GetClassNameW(hwnd, &mut cls);
            let c = String::from_utf16_lossy(&cls[..n as usize]);
            if c == "Shell_TrayWnd" || c == "Shell_SecondaryTrayWnd" {
                post(WM_APP_TASKBAR, 0, 0);
            }
        }
        _ => post(WM_APP_WINS, 0, 0),
    }
}

fn lp_xy(lp: LPARAM) -> (f32, f32) {
    ((lp.0 & 0xFFFF) as i16 as f32, ((lp.0 >> 16) & 0xFFFF) as i16 as f32)
}

impl App {
    fn new(main: HWND) -> App {
        let cfg = Config::load();
        let sh = Shared {
            sys: SysMon::new(),
            status: Status::new(),
            media: Default::default(),
            media_tx: None,
            media_art: None,
            media_art_serial: 0,
            icons: Default::default(),
            toasts: vec![],
            ws_list: vec![],
            now: local_now(),
            save: false,
            redraw: false,
            close_panel: false,
            pick: None,
            eyedrop: None,
            focus_app: None,
            catalog: None,
            panel_scale: 1.0,
            open_settings_for: None,
            main_hwnd: main,
            current_widget: 0,
            popup: None,
            drag_out: None,
            rewatch: false,
            ics: Default::default(),
        };
        let mut cfg = cfg;
        let mut sh = sh;
        if cfg.calendars.is_empty() {
            // versões antigas guardavam as agendas em cada widget
            for w in cfg.widgets.iter().filter(|w| w.kind == "calendar") {
                for u in widgets::calendar::Calendar::legacy_urls(&w.data) {
                    if !cfg.calendars.contains(&u) {
                        cfg.calendars.push(u);
                    }
                }
            }
        }
        sh.ics.urls = cfg.calendars.clone();
        // apps fixados de versões antigas: lê os argumentos do atalho (uma vez)
        let mut resolved_any = false;
        for ws in cfg.workspaces.iter_mut() {
            for it in ws.items.iter_mut() {
                if let DockItem::App(a) = it {
                    if a.resolved {
                        continue;
                    }
                    a.resolved = true;
                    resolved_any = true;
                    if a.launch.to_lowercase().ends_with(".lnk") && std::path::Path::new(&a.launch).exists() {
                        if let Some((target, args, aumid)) = icons::resolve_lnk(&a.launch) {
                            a.lnk_args = args;
                            if a.exe.is_empty() {
                                a.exe = target;
                            }
                            if a.aumid.is_empty() {
                                a.aumid = aumid;
                            }
                        }
                    }
                }
            }
        }
        if resolved_any {
            cfg.save();
        }
        let mut widgets: BTreeMap<u32, Box<dyn Widget>> = BTreeMap::new();
        for wi in &cfg.widgets {
            widgets.insert(wi.id, widgets::make(&wi.kind, &wi.data));
        }
        App {
            cfg,
            sh,
            widgets,
            docks: vec![],
            wins: WinList::new(),
            taskbar: Taskbar::new(),
            panel: None,
            preview: None,
            settings: None,
            toasts: vec![],
            eyedrop: None,
            main,
            shellhook_msg: 0,
            taskbar_created_msg: 0,
            hooks: vec![],
            due: HashMap::new(),
            trash_full: false,
            pending_launch: None,
            preview_pending: None,
            watch_ids: vec![],
            hotkeys: vec![],
            clock_cal: None,
            menu_open: false,
            last_minute: 99,
            follow: vec![],
            follow_pending: None,
            toast_serial: 0,
            pet_tick: 0,
            explorer_pid: 0,
            quitting: false,
            last_panel_close: (String::new(), 0),
            sys_last: 0,
            media_started: false,
            last_rebroadcast: 0,
            watchdog_started: false,
            pending_click: None,
        }
    }

    fn start(&mut self) {
        glass::set_glass_mode(self.cfg.appearance.glass_mode);
        glass::enable_dark_menus(true);
        self.sh.status.init(self.main);
        self.sync_ws_list();
        unsafe {
            self.shellhook_msg = RegisterWindowMessageW(w!("SHELLHOOK"));
            self.taskbar_created_msg = RegisterWindowMessageW(w!("TaskbarCreated"));
            let _ = RegisterShellHookWindow(self.main);
            let tray_h = taskbar::explorer_tray();
            GetWindowThreadProcessId(tray_h, Some(&mut self.explorer_pid));
            let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
            self.hooks.push(SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, None, Some(win_event), 0, 0, flags));
            self.hooks.push(SetWinEventHook(EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED, None, Some(win_event), 0, 0, flags));
            if self.explorer_pid != 0 {
                self.hooks.push(SetWinEventHook(EVENT_OBJECT_SHOW, EVENT_OBJECT_SHOW, None, Some(win_event), self.explorer_pid, 0, flags));
            }
        }
        self.apply_taskbar();
        if self.cfg.general.start_with_windows {
            taskbar::set_autostart(true);
        }
        self.wins.fg = unsafe { GetForegroundWindow() };
        self.create_docks();
        self.refresh_windows();
        self.register_hotkeys();
        self.rewatch();
        self.ensure_services();
        self.sh.ics.maybe_fetch();
        unsafe {
            SetTimer(Some(self.main), T_SLOW, 5000, None);
            if self.cfg.general.autohide {
                SetTimer(Some(self.main), T_HIDE, 1500, None);
            }
        }
        self.schedule_tick();
        self.schedule_pet();
        if DUMP.with(|d| d.borrow().is_some()) {
            unsafe {
                SetTimer(Some(self.main), T_DUMP, 5000, None);
            }
        }
    }

    /// Modo de teste: salva em PNG o que cada parte desenha e sai.
    fn dump(&mut self) {
        let Some(dir) = DUMP.with(|d| d.borrow().clone()) else { return };
        let _ = std::fs::create_dir_all(&dir);
        for i in 0..self.docks.len() {
            self.render(i);
            self.docks[i].surf.save_png(&format!("{}\\dock{}.png", dir, i));
            // teste do redesenho parcial: só os widgets
            let ids: Vec<u32> = self.docks[i].slots.iter().filter(|s| s.kind == SlotKind::Widget).map(|s| s.widget).collect();
            self.render_widgets(i, &ids[ids.len().saturating_sub(1)..]);
            self.docks[i].surf.save_png(&format!("{}\\dock{}_partial.png", dir, i));
        }
        // folha com todos os tiles do catálogo
        let s = 64.0f32;
        let cols = 4;
        let cell_w = s * 4.0 + 3.0 * s * 0.18 + 24.0;
        let cell_h = s + 40.0;
        let rows = (widgets::CATALOG.len() + cols - 1) / cols;
        let mut sheet = Surface::new();
        let mut temp: Vec<(Box<dyn Widget>, u32, u32, &'static str)> = widgets::CATALOG
            .iter()
            .map(|e| {
                let v: serde_json::Value = if e.data.is_empty() { serde_json::Value::Null } else { serde_json::from_str(e.data).unwrap_or(serde_json::Value::Null) };
                (widgets::make(e.kind, &v), e.units, e.style, e.name)
            })
            .collect();
        self.sh.sys.sample();
        std::thread::sleep(std::time::Duration::from_millis(300));
        self.sh.sys.sample();
        let sh = &mut self.sh;
        sheet.paint(self.main, 0, 0, (cell_w * cols as f32) as i32, (cell_h * rows as f32) as i32, |c| {
            c.fill_rect(Rect::new(0.0, 0.0, cell_w * cols as f32, cell_h * rows as f32), Color::hex(0x2A3A4A));
            for (k, (w, units, style, name)) in temp.iter_mut().enumerate() {
                let x = (k % cols) as f32 * cell_w + 12.0;
                let y = (k / cols) as f32 * cell_h + 8.0;
                let len = *units as f32 * s + (*units as f32 - 1.0) * s * 0.18;
                let r = Rect::new(x, y, len, s);
                let tile = widgets::Tile { c, r, s, units: *units, style: *style, hover: false, name };
                w.draw(&tile, sh);
                c.text(name, Rect::new(x, y + s + 4.0, cell_w - 20.0, 22.0), 12.0, W_NORMAL, WHITE.a(0.8), Align::Left);
            }
        });
        sheet.save_png(&format!("{}\\tiles.png", dir));
        // painéis de cada widget
        let anchor = RECT { left: 700, top: 820, right: 760, bottom: 880 };
        for (k, e) in widgets::CATALOG.iter().enumerate() {
            let v: serde_json::Value = if e.data.is_empty() { serde_json::Value::Null } else { serde_json::from_str(e.data).unwrap_or(serde_json::Value::Null) };
            let id = 50_000 + k as u32;
            self.widgets.insert(id, widgets::make(e.kind, &v));
            self.open_panel(PanelKind::Widget(id), 0, anchor, format!("dump{}", k));
            if let Some(p) = &self.panel {
                p.p.surf.save_png(&format!("{}\\panel_{:02}_{}.png", dir, k, e.kind));
            }
            self.close_panel();
            self.widgets.remove(&id);
        }
        self.open_panel(PanelKind::Calendar, 0, anchor, "dumpcal".into());
        if let Some(p) = &self.panel {
            p.p.surf.save_png(&format!("{}\\panel_clock_calendar.png", dir));
        }
        self.close_panel();
        let pages = [
            (crate::settings::Page::General, "general"),
            (crate::settings::Page::Appearance, "appearance"),
            (crate::settings::Page::Workspaces, "workspaces"),
            (crate::settings::Page::Widgets, "widgets"),
            (crate::settings::Page::Bar, "bar"),
            (crate::settings::Page::About, "about"),
        ];
        for (pg, name) in pages {
            self.open_settings(Some(pg));
            self.render_settings();
            if let Some(s) = &self.settings {
                s.p.surf.save_png(&format!("{}\\settings_{}.png", dir, name));
            }
        }
        if let Some(wid) = self.cfg.widgets.first().map(|w| w.id) {
            self.open_settings(Some(crate::settings::Page::EditWidget(wid)));
            self.render_settings();
            if let Some(s) = &self.settings {
                s.p.surf.save_png(&format!("{}\\settings_editwidget.png", dir));
            }
        }
        self.close_settings();
        self.quit();
    }

    fn apply_taskbar(&mut self) {
        let mode = self.cfg.general.taskbar;
        self.taskbar.apply(mode);
        if mode != TaskbarMode::Keep {
            if !self.watchdog_started {
                self.watchdog_started = true;
                taskbar::spawn_watchdog();
            }
            let has_tray = tray::with(|t| !t.own.0.is_null());
            if !has_tray && self.cfg.general.show_status {
                tray::start(taskbar::explorer_tray());
                if !self.cfg.tray_initialized {
                    // começa com os ícones que o próprio Windows deixa à mostra
                    self.cfg.tray_initialized = true;
                    let promoted = tray::windows_promoted_exes();
                    self.cfg.tray_pinned = promoted.iter().map(|e| format!("{}|*", e)).collect();
                    self.mark_save();
                }
            }
        } else {
            tray::stop();
        }
    }

    fn create_docks(&mut self) {
        let plates: Vec<HWND> = self.docks.iter().map(|d| d.plate).collect();
        for p in plates {
            self.remove_appbar_of(p);
        }
        for d in self.docks.iter_mut() {
            d.destroy();
        }
        self.docks.clear();
        for (h, _) in self.follow.drain(..) {
            glass::destroy(h);
        }
        let mons = glass::monitors();
        let first_ws = self.cfg.workspaces.first().map(|w| w.id).unwrap_or(1);
        let targets: Vec<HMONITOR> = if self.cfg.general.all_monitors {
            mons.clone()
        } else {
            let cur = unsafe { MonitorFromPoint(glass::cursor_pos(), MONITOR_DEFAULTTOPRIMARY) };
            let _ = cur;
            vec![glass::primary_monitor()]
        };
        for (i, m) in targets.iter().enumerate() {
            let d = Dock::new(i, *m, first_ws);
            crate::dragdrop::register(d.hwnd, i);
            self.docks.push(d);
        }
        if !self.cfg.general.all_monitors && self.cfg.general.follow_pointer {
            for m in mons {
                if m != targets[0] {
                    let h = glass::create_trigger(900 + self.follow.len());
                    self.follow.push((h, m));
                }
            }
        }
        for i in 0..self.docks.len() {
            self.setup_appbar(i);
        }
        self.place_triggers();
        self.rebuild_all();
        for d in &self.docks {
            if d.hide < 0.5 {
                glass::show(d.plate, true);
                glass::show(d.hwnd, true);
            }
        }
    }

    fn place_triggers(&mut self) {
        let cfg = &self.cfg;
        for d in &self.docks {
            let r = d.trigger_rect(cfg);
            unsafe {
                let _ = SetWindowPos(d.trigger, Some(HWND_TOPMOST), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOACTIVATE);
            }
            glass::show(d.trigger, d.hide > 0.5 || d.fullscreen);
        }
        for (h, m) in &self.follow {
            let (mon, _) = glass::monitor_info(*m);
            let r = match cfg.general.position {
                DockPos::Bottom => RECT { left: mon.left, top: mon.bottom - 2, right: mon.right, bottom: mon.bottom },
                DockPos::Top => RECT { left: mon.left, top: mon.top, right: mon.right, bottom: mon.top + 2 },
                DockPos::Left => RECT { left: mon.left, top: mon.top, right: mon.left + 2, bottom: mon.bottom },
                DockPos::Right => RECT { left: mon.right - 2, top: mon.top, right: mon.right, bottom: mon.bottom },
            };
            unsafe {
                let _ = SetWindowPos(*h, Some(HWND_TOPMOST), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }
        }
    }

    // -----------------------------------------------------------------------
    // AppBar: reserva o espaço da dock e avisa sobre tela cheia
    // -----------------------------------------------------------------------

    fn setup_appbar(&mut self, i: usize) {
        let reserve = !self.cfg.general.autohide && self.cfg.general.reserve_space && self.cfg.general.taskbar != TaskbarMode::Keep;
        let d = &mut self.docks[i];
        unsafe {
            let mut abd = APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, hWnd: d.plate, uCallbackMessage: WM_APP_APPBAR, ..Default::default() };
            if !d.appbar {
                SHAppBarMessage(ABM_NEW, &mut abd);
                d.appbar = true;
            }
            let g = d.geo(&self.cfg);
            let thick = if reserve { d.reserve_thickness(&g) } else { 0 };
            d.reserved = thick;
            let m = d.mon;
            abd.uEdge = match self.cfg.general.position {
                DockPos::Bottom => ABE_BOTTOM,
                DockPos::Top => ABE_TOP,
                DockPos::Left => ABE_LEFT,
                DockPos::Right => ABE_RIGHT,
            };
            abd.rc = match self.cfg.general.position {
                DockPos::Bottom => RECT { left: m.left, top: m.bottom - thick, right: m.right, bottom: m.bottom },
                DockPos::Top => RECT { left: m.left, top: m.top, right: m.right, bottom: m.top + thick },
                DockPos::Left => RECT { left: m.left, top: m.top, right: m.left + thick, bottom: m.bottom },
                DockPos::Right => RECT { left: m.right - thick, top: m.top, right: m.right, bottom: m.bottom },
            };
            if thick > 0 {
                SHAppBarMessage(ABM_QUERYPOS, &mut abd);
                match self.cfg.general.position {
                    DockPos::Bottom => abd.rc.top = abd.rc.bottom - thick,
                    DockPos::Top => abd.rc.bottom = abd.rc.top + thick,
                    DockPos::Left => abd.rc.right = abd.rc.left + thick,
                    DockPos::Right => abd.rc.left = abd.rc.right - thick,
                }
                SHAppBarMessage(ABM_SETPOS, &mut abd);
            } else {
                abd.rc = RECT { left: m.left, top: m.bottom, right: m.right, bottom: m.bottom };
                SHAppBarMessage(ABM_SETPOS, &mut abd);
            }
        }
        let (mon, work) = glass::monitor_info(self.docks[i].monitor);
        self.docks[i].mon = mon;
        self.docks[i].work = work;
    }

    fn remove_appbar_of(&self, plate: HWND) {
        unsafe {
            let mut abd = APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, hWnd: plate, ..Default::default() };
            SHAppBarMessage(ABM_REMOVE, &mut abd);
        }
    }

    // -----------------------------------------------------------------------
    // Montagem e desenho
    // -----------------------------------------------------------------------

    pub fn sync_ws_list(&mut self) {
        self.sh.ws_list = self.cfg.workspaces.iter().map(|w| (w.id, w.name.clone(), w.color)).collect();
    }

    pub fn rebuild_all(&mut self) {
        let n = self.docks.len();
        for i in 0..n {
            dock::build(&mut self.docks[i], &self.cfg, &self.wins, &self.widgets, &mut self.sh, n);
        }
        self.render_all();
        self.schedule_pet();
    }

    pub fn rebuild(&mut self, i: usize) {
        let n = self.docks.len();
        if i < n {
            dock::build(&mut self.docks[i], &self.cfg, &self.wins, &self.widgets, &mut self.sh, n);
            self.render(i);
        }
    }

    pub fn render_all(&mut self) {
        for i in 0..self.docks.len() {
            self.render(i);
        }
    }

    pub fn render(&mut self, i: usize) {
        self.render_widgets(i, &[]);
    }

    pub fn render_widgets(&mut self, i: usize, only: &[u32]) {
        if i >= self.docks.len() {
            return;
        }
        self.sh.now = local_now();
        let mut ctx = dock::DrawCtx { cfg: &self.cfg, widgets: &mut self.widgets, sh: &mut self.sh, trash_full: self.trash_full };
        let d = &mut self.docks[i];
        dock::render_dirty(d, &mut ctx, only);
        // a placa de vidro acompanha a barra
        let bar = d.bar;
        let wr = d.win;
        if bar.w > 0.0 {
            let g = d.geo(&self.cfg);
            let rad = (g.s * 0.4 + 9.0 * g.sc).min(bar.h.min(bar.w) / 2.0);
            let pr = RECT {
                left: wr.left + bar.x.round() as i32 + 1,
                top: wr.top + bar.y.round() as i32 + 1,
                right: wr.left + bar.r().round() as i32 - 1,
                bottom: wr.top + bar.b().round() as i32 - 1,
            };
            if pr != d.plate_rect {
                d.plate_rect = pr;
                glass::place_plate(d.plate, pr.left, pr.top, pr.right - pr.left, pr.bottom - pr.top, rad as i32, None);
            }
        }
        let needs = self.sh.save || self.sh.redraw;
        if needs {
            self.sh.redraw = false;
        }
        self.process_requests();
    }

    /// Recalcula a ampliação etc. e agenda animação se preciso.
    fn kick_anim(&mut self) {
        unsafe {
            SetTimer(Some(self.main), T_ANIM, 15, None);
        }
    }

    fn on_anim(&mut self) {
        let mut any = false;
        for i in 0..self.docks.len() {
            let d = &mut self.docks[i];
            let step = 0.22;
            let mut moved = false;
            if (d.mag - d.mag_target).abs() > 0.001 {
                d.mag += (d.mag_target - d.mag) * step + (d.mag_target - d.mag).signum() * 0.02;
                if (d.mag - d.mag_target).abs() < 0.03 {
                    d.mag = d.mag_target;
                }
                moved = true;
            }
            if (d.hide - d.hide_target).abs() > 0.001 {
                let sp = 0.12;
                d.hide = if d.hide < d.hide_target { (d.hide + sp).min(d.hide_target) } else { (d.hide - sp).max(d.hide_target) };
                moved = true;
                if d.hide >= 1.0 {
                    unsafe {
                        SetTimer(Some(self.main), T_EDGE, 120, None);
                    }
                    glass::show(d.hwnd, false);
                    glass::show(d.plate, false);
                    glass::show(d.pet_hwnd, false);
                    glass::show(d.trigger, true);
                    d.plate_rect = RECT::default();
                }
            }
            let now = tick_ms();
            let before = d.bounce.len();
            // abrindo: a animação só para quando a janela aparece (ou após 30 s)
            let slots = &d.slots;
            d.launching.retain(|k, t| now.saturating_sub(*t) < 30_000 && !slots.iter().any(|s| s.key == *k && !s.wins.is_empty()));
            let launching = &d.launching;
            d.bounce.retain(|k, t| now.saturating_sub(*t) < 1700 || launching.contains_key(k));
            if !d.bounce.is_empty() || before != d.bounce.len() {
                // quadros a ~30 por segundo bastam
                if !d.launching.is_empty() && d.bounce.len() == d.launching.len() && !moved && now.saturating_sub(d.anim_last) < 30 {
                    any = true;
                    continue;
                }
                d.anim_last = now;
                moved = true;
            }
            if moved {
                any = true;
                if d.hide < 1.0 {
                    self.render(i);
                }
            }
        }
        if !any {
            unsafe {
                let _ = KillTimer(Some(self.main), T_ANIM);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Janelas
    // -----------------------------------------------------------------------

    fn assign_ws_for(&self, mon: isize) -> u32 {
        for d in &self.docks {
            if d.monitor.0 as isize == mon {
                return d.ws;
            }
        }
        self.docks.first().map(|d| d.ws).unwrap_or(1)
    }

    pub fn refresh_windows(&mut self) {
        let map: Vec<(isize, u32)> = self.docks.iter().map(|d| (d.monitor.0 as isize, d.ws)).collect();
        let default_ws = self.docks.first().map(|d| d.ws).unwrap_or(1);
        let assign = move |mon: isize| -> u32 { map.iter().find(|(m, _)| *m == mon).map(|x| x.1).unwrap_or(default_ws) };
        let changed = self.wins.refresh(&assign);
        // janelas novas de um app recém-aberto: terminou a animação de abertura
        if changed {
            for d in self.docks.iter_mut() {
                let keys: Vec<String> = d.bounce.keys().cloned().collect();
                for k in keys {
                    if let Some(sl) = d.slots.iter().find(|s| s.key == k) {
                        let t0 = d.bounce[&k];
                        if !sl.wins.is_empty() && tick_ms().saturating_sub(t0) > 500 {
                            d.bounce.remove(&k);
                        }
                    }
                }
            }
            self.rebuild_all();
            self.learn_launch();
            if self.wins.dirty_save {
                unsafe {
                    SetTimer(Some(self.main), T_SAVE, 2000, None);
                }
            }
        }
    }

    /// Depois de abrir um app fixado: se uma janela nova apareceu e não foi
    /// reconhecida como dele (ex.: o atalho aponta para um atualizador),
    /// guarda o executável dela para reconhecer da próxima vez.
    fn learn_launch(&mut self) {
        let Some(p) = self.pending_launch.clone() else { return };
        if tick_ms().saturating_sub(p.t) > 60_000 {
            self.pending_launch = None;
            return;
        }
        if !p.pinned {
            return;
        }
        let key = format!("pin:{}", p.launch.to_lowercase());
        let have_wins = self.docks.iter().any(|d| d.slots.iter().any(|s| s.key == key && !s.wins.is_empty()));
        if have_wins {
            self.pending_launch = None;
            return;
        }
        // janelas novas que ficaram como "abertos soltos"
        let loose: Vec<HWND> = self.docks.iter().flat_map(|d| d.slots.iter()).filter(|s| s.kind == SlotKind::Running).flat_map(|s| s.wins.iter().copied()).collect();
        let waited = tick_ms().saturating_sub(p.t) as f64 / 1000.0;
        let found = self.wins.wins.iter().find(|w| {
            loose.contains(&w.hwnd) && w.seen + 300 >= p.t && !w.exe.is_empty() && process_age_secs(w.pid).map(|a| a <= waited + 3.0).unwrap_or(false)
        });
        let Some(w) = found.cloned() else { return };
        let exe = w.exe.to_lowercase();
        let mut learned = false;
        for ws in self.cfg.workspaces.iter_mut() {
            for it in ws.items.iter_mut() {
                if let DockItem::App(a) = it {
                    if a.launch == p.launch && !a.alt.contains(&exe) {
                        a.alt.push(exe.clone());
                        learned = true;
                    }
                }
            }
        }
        self.pending_launch = None;
        if learned {
            self.mark_save();
            self.rebuild_all();
        }
    }

    fn on_foreground(&mut self, h: HWND) {
        if h.0.is_null() {
            return;
        }
        if self.wins.get(h).is_none() {
            // janela que ainda não conhecemos: reenumera em breve
            unsafe {
                SetTimer(Some(self.main), T_WINS, 60, None);
            }
        }
        // outro app ganhou o foco: fecha o painel aberto
        let panel_close = self.panel.as_ref().map(|p| !p.modal && p.p.hwnd != h && tick_ms().saturating_sub(p.opened) > 250).unwrap_or(false);
        if panel_close && !self.menu_open {
            self.close_panel();
        }
        let prev_fg = self.wins.fg;
        self.wins.note_foreground(h);
        if let Some(w) = self.wins.get(h).cloned() {
            // janela de outro ambiente ganhou o foco
            let di = self.dock_for_monitor(w.monitor);
            if let Some(di) = di {
                let cur = self.docks[di].ws;
                if w.ws != cur && self.cfg.general.only_current_ws && tick_ms() > self.wins.switching_until {
                    let launched = self.pending_launch.as_ref().map(|p| tick_ms().saturating_sub(p.t) < 30_000 && (w.key == p.k || w.exe.to_lowercase().contains(&p.k.to_lowercase()) || w.seen >= p.t.saturating_sub(300))).unwrap_or(false);
                    if launched {
                        if let Some(wm) = self.wins.get_mut(h) {
                            wm.ws = cur;
                        }
                        self.wins.dirty_save = true;
                    } else {
                        self.switch_ws(di, w.ws);
                        return;
                    }
                }
            }
        }
        if prev_fg != h {
            for i in 0..self.docks.len() {
                let had = self.docks[i].slots.iter().any(|s| s.wins.contains(&prev_fg) || s.wins.contains(&h));
                if had {
                    self.rebuild(i);
                }
            }
        }
    }

    fn dock_for_monitor(&self, mon: isize) -> Option<usize> {
        if self.docks.len() == 1 {
            return Some(0);
        }
        self.docks.iter().position(|d| d.monitor.0 as isize == mon)
    }

    pub fn switch_ws(&mut self, di: usize, ws: u32) {
        if di >= self.docks.len() || self.docks[di].ws == ws || self.cfg.ws_index(ws).is_none() {
            return;
        }
        self.close_preview();
        self.close_panel();
        self.docks[di].ws = ws;
        self.wins.switching_until = tick_ms() + 900;
        let multi = self.docks.len() > 1;
        let mon = self.docks[di].monitor.0 as isize;
        let mut best: Option<(HWND, u64)> = None;
        let fg = self.wins.fg;
        for w in self.wins.wins.iter_mut() {
            if multi && w.monitor != mon {
                continue;
            }
            if w.ws == ws {
                if w.min_by_us {
                    winlist::restore_quiet(w.hwnd);
                    w.min_by_us = false;
                }
                if best.map(|b| w.last_active > b.1).unwrap_or(true) {
                    best = Some((w.hwnd, w.last_active));
                }
            } else if !winlist::is_minimized(w.hwnd) {
                winlist::minimize_quiet(w.hwnd);
                w.min_by_us = true;
            }
        }
        let _ = fg;
        if let Some((h, _)) = best {
            winlist::activate(h);
        }
        let mins: Vec<isize> = self.wins.wins.iter().filter(|w| w.min_by_us).map(|w| w.hwnd.0 as isize).collect();
        self.taskbar.note_minimized(mins);
        self.wins.dirty_save = true;
        unsafe {
            SetTimer(Some(self.main), T_SAVE, 2000, None);
        }
        self.rebuild(di);
        self.schedule_pet();
    }

    fn cycle_ws(&mut self, di: usize, dir: i32) {
        let n = self.cfg.workspaces.len();
        if n < 2 || di >= self.docks.len() {
            return;
        }
        let cur = self.cfg.ws_index(self.docks[di].ws).unwrap_or(0) as i32;
        let next = (cur + dir).rem_euclid(n as i32) as usize;
        let id = self.cfg.workspaces[next].id;
        self.switch_ws(di, id);
    }

    fn active_dock(&self) -> usize {
        if self.docks.len() <= 1 {
            return 0;
        }
        let p = glass::cursor_pos();
        let m = unsafe { MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST) };
        self.docks.iter().position(|d| d.monitor == m).unwrap_or(0)
    }

    // -----------------------------------------------------------------------
    // Mensagens
    // -----------------------------------------------------------------------

    fn on_message(&mut self, h: HWND, kind: u32, idx: usize, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
        match kind {
            K_MAIN => self.main_msg(msg, wp, lp),
            K_DOCK => self.dock_msg(idx, msg, wp, lp),
            K_PLATE => {
                if msg == WM_APP_APPBAR {
                    self.appbar_notify(idx, wp, lp);
                    return Some(LRESULT(0));
                }
                None
            }
            K_TRIGGER => {
                if msg == WM_MOUSEMOVE {
                    if idx >= 900 {
                        if let Some((_, m)) = self.follow.get(idx - 900) {
                            self.follow_pending = Some(*m);
                            unsafe {
                                SetTimer(Some(self.main), T_FOLLOW, 280, None);
                            }
                        }
                    } else {
                        // encostou na borda: mostra na hora
                        self.maybe_show();
                    }
                    return Some(LRESULT(0));
                }
                None
            }
            K_PANEL => self.panel_msg(h, msg, wp, lp),
            K_PREVIEW => self.preview_msg(msg, wp, lp),
            K_SETTINGS => self.settings_msg(h, msg, wp, lp),
            K_TOAST => {
                if msg == WM_LBUTTONUP {
                    self.dismiss_toast(h);
                    return Some(LRESULT(0));
                }
                None
            }
            K_EYEDROP => self.eyedrop_msg(msg, wp, lp),
            _ => None,
        }
    }

    fn main_msg(&mut self, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
        if msg == self.shellhook_msg && msg != 0 {
            let code = wp.0 as u32;
            let h = HWND(lp.0 as _);
            match code {
                4 | 0x8004 => self.on_foreground(h),
                0x8006 => {
                    let fg = self.wins.fg;
                    if let Some(w) = self.wins.get_mut(h) {
                        if w.hwnd != fg {
                            w.flashing = true;
                        }
                    }
                    self.rebuild_all();
                }
                _ => unsafe {
                    SetTimer(Some(self.main), T_WINS, 60, None);
                },
            }
            return Some(LRESULT(0));
        }
        if msg == self.taskbar_created_msg && msg != 0 {
            // o Explorer reiniciou
            tray::set_explorer(taskbar::explorer_tray());
            unsafe {
                GetWindowThreadProcessId(taskbar::explorer_tray(), Some(&mut self.explorer_pid));
            }
            self.taskbar.apply(self.cfg.general.taskbar);
            for i in 0..self.docks.len() {
                self.docks[i].appbar = false;
                self.setup_appbar(i);
            }
            return Some(LRESULT(0));
        }
        match msg {
            WM_TIMER => {
                let id = wp.0;
                match id {
                    T_ANIM => self.on_anim(),
                    T_TICK => self.on_tick(),
                    T_SAVE => {
                        unsafe {
                            let _ = KillTimer(Some(self.main), T_SAVE);
                        }
                        self.do_save();
                    }
                    T_WINS => {
                        unsafe {
                            let _ = KillTimer(Some(self.main), T_WINS);
                        }
                        self.refresh_windows();
                    }
                    T_HIDE => {
                        unsafe {
                            let _ = KillTimer(Some(self.main), T_HIDE);
                        }
                        self.maybe_hide();
                    }
                    T_SHOW => {
                        unsafe {
                            let _ = KillTimer(Some(self.main), T_SHOW);
                        }
                        self.maybe_show();
                    }
                    T_PREVIEW => {
                        unsafe {
                            let _ = KillTimer(Some(self.main), T_PREVIEW);
                        }
                        self.show_preview();
                    }
                    T_PREVIEW_HIDE => {
                        unsafe {
                            let _ = KillTimer(Some(self.main), T_PREVIEW_HIDE);
                        }
                        self.maybe_close_preview();
                    }
                    T_SLOW => self.on_slow(),
                    T_PET => self.on_pet(),
                    T_PANEL => self.render_panel(),
                    T_TOAST => self.expire_toasts(),
                    T_EDGE => {
                        if self.docks.iter().any(|d| d.hide >= 1.0) {
                            self.maybe_show();
                        } else {
                            unsafe {
                                let _ = KillTimer(Some(self.main), T_EDGE);
                            }
                        }
                    }
                    T_CLICK => {
                        unsafe {
                            let _ = KillTimer(Some(self.main), T_CLICK);
                        }
                        if let Some((i, s, fx, fy, wid)) = self.pending_click {
                            let ok = self.docks.get(i).and_then(|d| d.slots.get(s)).map(|sl| sl.widget == wid).unwrap_or(false);
                            if ok {
                                self.click_slot(i, s, fx, fy);
                            }
                            self.pending_click = None;
                        }
                    }
                    T_DUMP => {
                        unsafe {
                            let _ = KillTimer(Some(self.main), T_DUMP);
                        }
                        self.dump();
                    }
                    T_FOLLOW => {
                        unsafe {
                            let _ = KillTimer(Some(self.main), T_FOLLOW);
                        }
                        self.follow_to();
                    }
                    _ => {}
                }
                Some(LRESULT(0))
            }
            WM_APP_POLL => {
                let mut ch = self.sh.ics.poll();
                for (_, w) in self.widgets.iter_mut() {
                    ch |= w.poll(&mut self.sh);
                }
                if let Some(c) = self.clock_cal.as_mut() {
                    ch |= c.poll(&mut self.sh);
                }
                if ch {
                    self.render_all();
                    if self.panel.is_some() {
                        self.render_panel();
                    }
                    self.render_settings();
                }
                Some(LRESULT(0))
            }
            WM_APP_TRAY => {
                let n = tray::visible_icons().len();
                let _ = n;
                self.rebuild_all();
                if matches!(self.panel.as_ref().map(|p| &p.kind), Some(PanelKind::Tray)) {
                    self.render_panel();
                }
                Some(LRESULT(0))
            }
            WM_APP_WINS => {
                unsafe {
                    SetTimer(Some(self.main), T_WINS, 80, None);
                }
                Some(LRESULT(0))
            }
            WM_APP_FG => {
                self.on_foreground(HWND(wp.0 as _));
                Some(LRESULT(0))
            }
            WM_APP_STATUS => {
                match wp.0 {
                    1 => self.sh.status.refresh_volume(),
                    2 => self.sh.status.init_audio(),
                    _ => self.sh.status.refresh_net(),
                }
                self.render_all();
                Some(LRESULT(0))
            }
            WM_APP_MEDIA => {
                if let Some(m) = crate::media::take_snapshot() {
                    if m.art_serial != self.sh.media_art_serial {
                        self.sh.media_art_serial = m.art_serial;
                        self.sh.media_art = m.art.as_ref().and_then(|b| gfx().bitmap_from_bytes(b, 256));
                    }
                    self.sh.media = m;
                }
                self.render_all();
                if self.panel.is_some() {
                    self.render_panel();
                }
                Some(LRESULT(0))
            }
            WM_APP_TASKBAR => {
                self.taskbar.hide_now();
                Some(LRESULT(0))
            }
            WM_APP_LAUNCHFAIL => {
                let names: Vec<String> = winlist::LAUNCH_FAILS.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
                for n in names {
                    self.show_toast("Não consegui abrir", &n, false);
                }
                for d in self.docks.iter_mut() {
                    d.launching.clear();
                }
                Some(LRESULT(0))
            }
            WM_APP_SHOW_SETTINGS => {
                self.open_settings(None);
                Some(LRESULT(0))
            }
            WM_POWERBROADCAST => {
                self.sh.status.refresh_battery();
                self.render_all();
                Some(LRESULT(1))
            }
            WM_HOTKEY => {
                self.on_hotkey(wp.0 as i32);
                Some(LRESULT(0))
            }
            WM_DISPLAYCHANGE | WM_DPICHANGED => {
                self.create_docks();
                Some(LRESULT(0))
            }
            WM_SETTINGCHANGE => {
                if wp.0 as u32 == SPI_SETWORKAREA.0 {
                    for d in self.docks.iter_mut() {
                        d.update_monitor();
                    }
                    self.render_all();
                } else {
                    glass::enable_dark_menus(true);
                    if self.cfg.appearance.tint == Tint::Auto {
                        self.render_all();
                    }
                }
                None
            }
            WM_QUERYENDSESSION => Some(LRESULT(1)),
            WM_ENDSESSION => {
                if wp.0 != 0 {
                    self.shutdown();
                }
                Some(LRESULT(0))
            }
            WM_CLOSE => {
                self.quit();
                Some(LRESULT(0))
            }
            _ => None,
        }
    }

    fn appbar_notify(&mut self, i: usize, wp: WPARAM, lp: LPARAM) {
        if i >= self.docks.len() {
            return;
        }
        match wp.0 as u32 {
            ABN_FULLSCREENAPP => {
                let on = lp.0 != 0;
                let fg = unsafe { GetForegroundWindow() };
                let fmon = unsafe { MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST) };
                let applies = !on || fmon == self.docks[i].monitor;
                if applies && self.cfg.general.hide_in_fullscreen {
                    let d = &mut self.docks[i];
                    d.fullscreen = on;
                    if on {
                        d.hide = 1.0;
                        d.hide_target = 1.0;
                        glass::show(d.hwnd, false);
                        glass::show(d.plate, false);
                        glass::show(d.pet_hwnd, false);
                        glass::show(d.trigger, true);
                        d.plate_rect = RECT::default();
                    } else if !self.cfg.general.autohide {
                        self.reveal(i);
                    }
                }
            }
            ABN_POSCHANGED => {
                self.setup_appbar(i);
                self.render(i);
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Ocultar automaticamente
    // -----------------------------------------------------------------------

    fn reveal(&mut self, i: usize) {
        let d = &mut self.docks[i];
        if d.hide > 0.5 {
            glass::show(d.trigger, false);
            glass::show(d.plate, true);
            glass::show(d.hwnd, true);
        }
        d.hide_target = 0.0;
        if d.hide >= 1.0 {
            d.hide = 0.999;
        }
        self.render(i);
        self.kick_anim();
        self.schedule_pet();
    }

    fn maybe_show(&mut self) {
        let p = glass::cursor_pos();
        for i in 0..self.docks.len() {
            let t = self.docks[i].trigger_rect(&self.cfg);
            let near = p.x >= t.left - 1 && p.x <= t.right + 1 && p.y >= t.top - 4 && p.y <= t.bottom + 1;
            if near && self.docks[i].hide > 0.5 {
                // cursor escondido (jogos) não traz a dock
                let mut ci = CURSORINFO { cbSize: std::mem::size_of::<CURSORINFO>() as u32, ..Default::default() };
                unsafe {
                    let _ = GetCursorInfo(&mut ci);
                }
                if ci.flags.0 == 0 {
                    continue;
                }
                self.reveal(i);
                unsafe {
                    SetTimer(Some(self.main), T_HIDE, 1200, None);
                }
            }
        }
    }

    fn should_stay(&self, i: usize) -> bool {
        let d = &self.docks[i];
        if d.mouse.is_some() || d.drag.is_some() || self.menu_open {
            return true;
        }
        if self.panel.as_ref().map(|p| p.dock == i).unwrap_or(false) || self.preview.as_ref().map(|p| p.dock == i).unwrap_or(false) {
            return true;
        }
        let p = glass::cursor_pos();
        let b = d.plate_rect;
        if p.x >= b.left && p.x <= b.right && p.y >= b.top - 4 && p.y <= b.bottom + 8 {
            return true;
        }
        false
    }

    fn maybe_hide(&mut self) {
        let mut retry = false;
        for i in 0..self.docks.len() {
            let auto = self.cfg.general.autohide || self.docks[i].fullscreen;
            if !auto || self.docks[i].hide_target >= 1.0 {
                continue;
            }
            if self.should_stay(i) {
                retry = true;
                continue;
            }
            self.docks[i].hide_target = 1.0;
            self.kick_anim();
        }
        if retry {
            unsafe {
                SetTimer(Some(self.main), T_HIDE, 700, None);
            }
        }
    }

    fn follow_to(&mut self) {
        let Some(m) = self.follow_pending.take() else { return };
        let p = glass::cursor_pos();
        let cur = unsafe { MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST) };
        if cur != m || self.docks.is_empty() {
            return;
        }
        let old = self.docks[0].monitor;
        self.remove_appbar_of(self.docks[0].plate);
        self.docks[0].appbar = false;
        self.docks[0].monitor = m;
        self.docks[0].update_monitor();
        self.docks[0].plate_rect = RECT::default();
        for f in self.follow.iter_mut() {
            if f.1 == m {
                f.1 = old;
            }
        }
        self.setup_appbar(0);
        self.place_triggers();
        self.refresh_windows();
        self.rebuild_all();
    }

    // -----------------------------------------------------------------------
    // Eventos da dock
    // -----------------------------------------------------------------------

    fn dock_msg(&mut self, i: usize, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
        if i >= self.docks.len() {
            return None;
        }
        match msg {
            WM_MOUSEMOVE => {
                let (x, y) = lp_xy(lp);
                self.dock_move(i, x, y);
                Some(LRESULT(0))
            }
            WM_MOUSELEAVE => {
                self.dock_leave(i);
                Some(LRESULT(0))
            }
            WM_LBUTTONDOWN => {
                let (x, y) = lp_xy(lp);
                let g = self.docks[i].geo(&self.cfg);
                let slot = dock::slot_at(&self.docks[i], x, y, &g);
                if let Some(s) = slot {
                    self.docks[i].press = Some((s, x, y));
                } else if self.docks[i].bar.contains(x, y) {
                    self.docks[i].press = Some((usize::MAX, x, y));
                }
                unsafe {
                    SetCapture(self.docks[i].hwnd);
                }
                Some(LRESULT(0))
            }
            WM_LBUTTONUP => {
                unsafe {
                    let _ = ReleaseCapture();
                }
                let (x, y) = lp_xy(lp);
                self.dock_up(i, x, y);
                Some(LRESULT(0))
            }
            WM_LBUTTONDBLCLK => {
                let (x, y) = lp_xy(lp);
                let g = self.docks[i].geo(&self.cfg);
                if let Some(s) = dock::slot_at(&self.docks[i], x, y, &g) {
                    let sl = self.docks[i].slots[s].clone();
                    if sl.kind == SlotKind::Tray {
                        let p = glass::cursor_pos();
                        tray::click(&sl.key, tray::Click::Double, p.x, p.y);
                        return Some(LRESULT(0));
                    }
                    if let Some((pi, ps, _, _, wid)) = self.pending_click {
                        if pi == i && ps == s && sl.widget == wid {
                            unsafe {
                                let _ = KillTimer(Some(self.main), T_CLICK);
                            }
                            self.pending_click = None;
                            if let Some(w) = self.widgets.get_mut(&wid) {
                                self.sh.current_widget = wid;
                                w.on_double_click(&mut self.sh);
                            }
                            self.process_requests();
                            self.render(i);
                            return Some(LRESULT(0));
                        }
                    }
                    // fora disso, vale como um clique comum
                    self.docks[i].press = Some((s, x, y));
                    unsafe {
                        SetCapture(self.docks[i].hwnd);
                    }
                }
                Some(LRESULT(0))
            }
            WM_RBUTTONUP => {
                let (x, y) = lp_xy(lp);
                self.dock_menu(i, x, y);
                Some(LRESULT(0))
            }
            WM_MBUTTONUP => {
                let (x, y) = lp_xy(lp);
                self.dock_middle(i, x, y);
                Some(LRESULT(0))
            }
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                let delta = ((wp.0 >> 16) & 0xFFFF) as i16 as f32 / 120.0;
                let p = glass::cursor_pos();
                let wr = self.docks[i].win;
                self.dock_wheel(i, (p.x - wr.left) as f32, (p.y - wr.top) as f32, if msg == WM_MOUSEHWHEEL { -delta } else { delta });
                Some(LRESULT(0))
            }
            WM_CAPTURECHANGED => {
                if self.docks[i].drag.is_some() && self.docks[i].press.is_some() {
                    self.docks[i].drag = None;
                    self.docks[i].press = None;
                    self.render(i);
                }
                Some(LRESULT(0))
            }
            _ => None,
        }
    }

    fn dock_move(&mut self, i: usize, x: f32, y: f32) {
        let d = &mut self.docks[i];
        if !d.tracking {
            unsafe {
                let mut tme = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: d.hwnd, dwHoverTime: 0 };
                let _ = TrackMouseEvent(&mut tme);
            }
            d.tracking = true;
        }
        let g = d.geo(&self.cfg);
        // começar a arrastar?
        if let Some((s, x0, y0)) = d.press {
            if d.drag.is_none() && ((x - x0).abs() + (y - y0).abs()) > 7.0 * g.sc {
                if s == usize::MAX {
                    d.press = None;
                } else if d.slots[s].kind == SlotKind::Sep && d.slots[s].item.is_none() {
                    let (_, v) = d.unmap(&g, x, y);
                    d.drag = Some(dock::Drag { slot: s, u: 0.0, v, out: false, target: s, resize: true, start_size: self.cfg.appearance.icon_size, start_v: v });
                } else if d.slots[s].draggable() {
                    let (u, v) = d.unmap(&g, x, y);
                    d.drag = Some(dock::Drag { slot: s, u, v, out: false, target: s, resize: false, start_size: 0.0, start_v: v });
                    self.close_preview();
                }
            }
        }
        let d = &mut self.docks[i];
        let (u, v) = d.unmap(&g, x, y);
        if let Some(dr) = d.drag.as_mut() {
            if dr.resize {
                let delta = (v - dr.start_v) / g.sc;
                self.cfg.appearance.icon_size = (dr.start_size + delta * 0.6).clamp(28.0, 96.0).round();
                self.sh.icons.clear();
                self.render(i);
                self.mark_save();
                return;
            }
            dr.u = u;
            dr.v = v;
            dr.out = v > g.margin + g.thick + g.s * 1.1 || v < -g.s;
            self.render(i);
            return;
        }
        d.mouse = Some((x, y));
        d.mag_target = 1.0;
        let prev = d.hover;
        d.hover = dock::slot_at(d, x, y, &g);
        let changed = prev != d.hover;
        if changed {
            d.hover_since = tick_ms();
            let is_app = d.hover.map(|s| d.slots[s].is_app() && !d.slots[s].wins.is_empty()).unwrap_or(false);
            if is_app && self.cfg.general.previews {
                let s = d.hover.unwrap();
                let multi_ok = !self.cfg.general.previews_only_multi || d.slots[s].wins.len() > 1;
                if multi_ok {
                    self.preview_pending = Some((i, s));
                    let delay = if self.preview.is_some() { 60 } else { self.cfg.general.preview_delay_ms.clamp(0, 2000) };
                    unsafe {
                        SetTimer(Some(self.main), T_PREVIEW, delay.max(1), None);
                    }
                }
            } else {
                self.preview_pending = None;
                if self.preview.is_some() {
                    unsafe {
                        SetTimer(Some(self.main), T_PREVIEW_HIDE, 250, None);
                    }
                }
            }
        }
        self.render(i);
        self.kick_anim();
    }

    fn dock_leave(&mut self, i: usize) {
        let d = &mut self.docks[i];
        d.tracking = false;
        if d.drag.is_some() {
            return;
        }
        d.mouse = None;
        d.hover = None;
        d.mag_target = 0.0;
        self.preview_pending = None;
        self.kick_anim();
        self.render(i);
        if self.preview.is_some() {
            unsafe {
                SetTimer(Some(self.main), T_PREVIEW_HIDE, 300, None);
            }
        }
        if self.cfg.general.autohide || self.docks[i].fullscreen {
            unsafe {
                SetTimer(Some(self.main), T_HIDE, 900, None);
            }
        }
    }

    fn dock_up(&mut self, i: usize, x: f32, y: f32) {
        let g = self.docks[i].geo(&self.cfg);
        let press = self.docks[i].press.take();
        if let Some(dr) = self.docks[i].drag.take() {
            if !dr.resize {
                self.finish_drag(i, dr.slot, dr.u, dr.out, &g);
            }
            self.rebuild(i);
            return;
        }
        let Some((ps, _, _)) = press else { return };
        let slot = dock::slot_at(&self.docks[i], x, y, &g);
        if slot == Some(ps) && ps != usize::MAX {
            let r = self.docks[i].slots[ps].rect;
            let fx = ((x - r.x) / r.w.max(1.0)).clamp(0.0, 1.0);
            let fy = ((y - r.y) / r.h.max(1.0)).clamp(0.0, 1.0);
            let (fx, fy) = if self.docks[i].vertical(&self.cfg) { (fy, fx) } else { (fx, fy) };
            self.click_slot(i, ps, fx, fy);
        } else if ps == usize::MAX {
            // clique no vazio: seletor de ambiente nas bolinhas já é um item
        }
    }

    fn finish_drag(&mut self, i: usize, si: usize, u: f32, out: bool, g: &dock::Geo) {
        let sl = self.docks[i].slots[si].clone();
        let Some(wsi) = self.cfg.ws_index(self.docks[i].ws) else { return };
        if out {
            if let Some(item) = sl.item {
                if !matches!(self.cfg.workspaces[wsi].items.get(item), Some(DockItem::Start)) || true {
                    self.cfg.workspaces[wsi].items.remove(item);
                    self.mark_save();
                }
            }
            return;
        }
        // modo livre: reordena dentro do grupo de apps (fixados e abertos)
        if self.cfg.general.mixed_apps && sl.in_block {
            let d = &self.docks[i];
            let visible: Vec<String> = d.slots.iter().filter(|s| s.in_block).map(|s| s.key.clone()).collect();
            let mut idx = 0;
            for (k, s) in d.slots.iter().enumerate() {
                if !s.in_block || k == si {
                    continue;
                }
                let (a, _) = d.unmap(g, s.rect.cx(), s.rect.cy());
                if a < u {
                    idx += 1;
                }
            }
            let vis_wo: Vec<String> = visible.iter().filter(|k| **k != sl.key).cloned().collect();
            let mut full = self.cfg.workspaces[wsi].app_order.clone();
            for k in &visible {
                if !full.contains(k) {
                    full.push(k.clone());
                }
            }
            full.retain(|k| *k != sl.key);
            let pos = if idx == 0 {
                vis_wo.first().and_then(|f| full.iter().position(|k| k == f)).unwrap_or(0)
            } else {
                vis_wo.get(idx - 1).and_then(|p| full.iter().position(|k| k == p)).map(|p| p + 1).unwrap_or(full.len())
            };
            full.insert(pos.min(full.len()), sl.key.clone());
            full.truncate(300);
            self.cfg.workspaces[wsi].app_order = full;
            self.mark_save();
            return;
        }
        // nova posição: o item mais próximo de onde foi solto. Widgets podem
        // ir para depois dos apps abertos (zona 1) ou da bandeja (zona 2).
        let d = &self.docks[i];
        let is_widget = sl.kind == SlotKind::Widget;
        let mut nearest: Option<(usize, f32)> = None;
        for (k, s) in d.slots.iter().enumerate() {
            if k == si || s.kind == SlotKind::Sep {
                continue;
            }
            if !is_widget && s.zone != 0 {
                continue;
            }
            let (a, _) = d.unmap(g, s.rect.cx(), s.rect.cy());
            let dist = (a - u).abs();
            if nearest.map(|n| dist < n.1).unwrap_or(true) {
                nearest = Some((k, dist));
            }
        }
        let (target_zone, target_item) = match nearest {
            Some((k, _)) => {
                let s = &d.slots[k];
                let (a, _) = d.unmap(g, s.rect.cx(), s.rect.cy());
                let z = if is_widget { s.zone } else { 0 };
                (z, s.item.map(|it| if u > a { it + 1 } else { it }))
            }
            None => (0, None),
        };
        let items = &mut self.cfg.workspaces[wsi].items;
        let target = target_item.unwrap_or(items.len()).min(items.len());
        match (sl.kind.clone(), sl.item) {
            (_, Some(from)) => {
                if from < items.len() {
                    let mut it = items.remove(from);
                    if let DockItem::Widgets { zone, .. } = &mut it {
                        *zone = target_zone;
                    }
                    let t = if target > from { target - 1 } else { target };
                    items.insert(t.min(items.len()), it);
                    self.mark_save();
                }
            }
            (SlotKind::Running, None) => {
                // arrastar um app aberto para a área dos fixados: fixa
                if let Some(&h) = sl.wins.first() {
                    if let Some(w) = self.wins.get(h) {
                        let app = PinnedApp {
                            name: sl.name.clone(),
                            launch: if !w.aumid.is_empty() { format!("shell:AppsFolder\\{}", w.aumid) } else { w.exe.clone() },
                            args: String::new(),
                            exe: w.exe.clone(),
                            aumid: w.aumid.clone(),
                            ..Default::default()
                        };
                        let pinned_count = items.iter().filter(|x| !matches!(x, DockItem::Widgets { .. })).count();
                        items.insert(target.min(pinned_count.max(target)), DockItem::App(app));
                        self.mark_save();
                    }
                }
            }
            _ => {}
        }
    }

    /// Onde abrir o painel da bandeja: perto dos ícones de status, ou da barra.
    fn status_anchor(&self, i: usize) -> RECT {
        let d = &self.docks[i.min(self.docks.len() - 1)];
        let k = d.slots.iter().position(|s| matches!(s.kind, SlotKind::Chevron | SlotKind::Tray | SlotKind::Net | SlotKind::Vol | SlotKind::Bat | SlotKind::Clock));
        match k {
            Some(k) => self.slot_screen_rect(i.min(self.docks.len() - 1), k),
            None => RECT { left: d.win.left + d.bar.x as i32, top: d.win.top + d.bar.y as i32, right: d.win.left + d.bar.r() as i32, bottom: d.win.top + d.bar.b() as i32 },
        }
    }

    fn slot_screen_rect(&self, i: usize, s: usize) -> RECT {
        let d = &self.docks[i];
        let r = d.slots[s].rect;
        RECT { left: d.win.left + r.x as i32, top: d.win.top + r.y as i32, right: d.win.left + r.r() as i32, bottom: d.win.top + r.b() as i32 }
    }

    pub fn click_slot(&mut self, i: usize, s: usize, fx: f32, fy: f32) {
        let sl = self.docks[i].slots[s].clone();
        match sl.kind {
            SlotKind::Start => {
                self.close_panel();
                taskbar::open_start_menu();
            }
            SlotKind::Pinned | SlotKind::Running => self.app_click(i, s),
            SlotKind::Widget => {
                let wid = sl.widget;
                self.sh.current_widget = wid;
                let units = sl.units;
                let wants_dbl = self.widgets.get(&wid).map(|w| w.double_click()).unwrap_or(false);
                if wants_dbl && self.pending_click.is_none() {
                    // espera para ver se vem um segundo clique
                    self.pending_click = Some((i, s, fx, fy, wid));
                    unsafe {
                        SetTimer(Some(self.main), T_CLICK, GetDoubleClickTime().clamp(150, 600), None);
                    }
                    return;
                }
                self.pending_click = None;
                let r = if let Some(w) = self.widgets.get_mut(&wid) { w.on_click(fx, fy, units, &mut self.sh) } else { return };
                if r == widgets::Click::Panel {
                    let anchor = self.slot_screen_rect(i, s);
                    self.toggle_panel(PanelKind::Widget(wid), i, anchor, sl.key.clone());
                } else {
                    self.render(i);
                }
                self.process_requests();
            }
            SlotKind::Folder => {
                let anchor = self.slot_screen_rect(i, s);
                let list = match sl.item.and_then(|it| self.cfg.ws_index(self.docks[i].ws).and_then(|w| self.cfg.workspaces[w].items.get(it).cloned())) {
                    Some(DockItem::Folder { list, .. }) => list,
                    _ => false,
                };
                self.toggle_panel(PanelKind::Folder(sl.icon.clone(), list), i, anchor, sl.key.clone());
            }
            SlotKind::Downloads => {
                let anchor = self.slot_screen_rect(i, s);
                self.toggle_panel(PanelKind::Folder(sl.icon.clone(), false), i, anchor, sl.key.clone());
            }
            SlotKind::Trash => shell_open("shell:RecycleBinFolder"),
            SlotKind::WsDots => {
                let n = self.cfg.workspaces.len();
                if n > 0 {
                    let k = ((fx * n as f32).floor() as usize).min(n - 1);
                    let id = self.cfg.workspaces[k].id;
                    self.switch_ws(i, id);
                }
            }
            SlotKind::Tray => {
                let r = self.slot_screen_rect(i, s);
                tray::click(&sl.key, tray::Click::Left, (r.left + r.right) / 2, (r.top + r.bottom) / 2);
            }
            SlotKind::Chevron => {
                let anchor = self.slot_screen_rect(i, s);
                self.toggle_panel(PanelKind::Tray, i, anchor, "chevron".into());
            }
            SlotKind::Net | SlotKind::Vol | SlotKind::Bat => {
                self.close_panel();
                taskbar::open_quick_settings();
            }
            SlotKind::Clock => {
                let anchor = self.slot_screen_rect(i, s);
                self.toggle_panel(PanelKind::Calendar, i, anchor, "clock".into());
            }
            SlotKind::Spacer | SlotKind::Sep => {}
        }
    }

    fn app_click(&mut self, i: usize, s: usize) {
        let sl = self.docks[i].slots[s].clone();
        self.close_preview();
        self.close_panel();
        if sl.wins.is_empty() {
            if let Some(app) = self.slot_app(i, s) {
                // a animação continua até a janela aparecer (apps pesados demoram)
                self.docks[i].bounce.insert(sl.key.clone(), tick_ms());
                self.docks[i].launching.insert(sl.key.clone(), tick_ms());
                self.render(i);
                winlist::launch(&app, false);
                let k = if !app.aumid.is_empty() { winlist::group_key("", &app.aumid) } else { file_stem(&app.exe) };
                self.pending_launch = Some(PendingLaunch { k, t: tick_ms(), ws: self.docks[i].ws, launch: app.launch.clone(), pinned: sl.kind == SlotKind::Pinned });
                self.kick_anim();
            }
            return;
        }
        let mut wins = sl.wins.clone();
        wins.sort_by_key(|h| std::cmp::Reverse(self.wins.get(*h).map(|w| w.last_active).unwrap_or(0)));
        let fg = unsafe { GetForegroundWindow() };
        if wins.len() == 1 {
            let h = wins[0];
            if fg == h && !winlist::is_minimized(h) {
                winlist::minimize(h);
            } else {
                winlist::activate(h);
            }
        } else if let Some(pos) = wins.iter().position(|h| *h == fg) {
            // várias janelas: passa para a próxima
            let next = wins[(pos + 1) % wins.len()];
            winlist::activate(next);
        } else {
            winlist::activate(wins[0]);
        }
    }

    /// App fixado (ou equivalente) de um item.
    fn slot_app(&self, i: usize, s: usize) -> Option<PinnedApp> {
        let sl = &self.docks[i].slots[s];
        match sl.kind {
            SlotKind::Pinned => {
                let wsi = self.cfg.ws_index(self.docks[i].ws)?;
                match self.cfg.workspaces[wsi].items.get(sl.item?)? {
                    DockItem::App(a) => Some(a.clone()),
                    _ => None,
                }
            }
            SlotKind::Running => {
                let w = self.wins.get(*sl.wins.first()?)?;
                Some(PinnedApp {
                    name: sl.name.clone(),
                    launch: if !w.aumid.is_empty() && w.exe.to_lowercase().ends_with("applicationframehost.exe") { format!("shell:AppsFolder\\{}", w.aumid) } else { w.exe.clone() },
                    args: String::new(),
                    exe: w.exe.clone(),
                    aumid: w.aumid.clone(),
                    ..Default::default()
                })
            }
            _ => None,
        }
    }

    fn dock_middle(&mut self, i: usize, x: f32, y: f32) {
        let g = self.docks[i].geo(&self.cfg);
        let Some(s) = dock::slot_at(&self.docks[i], x, y, &g) else { return };
        let sl = self.docks[i].slots[s].clone();
        match sl.kind {
            SlotKind::Pinned | SlotKind::Running => {
                if let Some(app) = self.slot_app(i, s) {
                    winlist::launch(&app, true);
                    self.docks[i].bounce.insert(sl.key.clone(), tick_ms());
                    self.kick_anim();
                }
            }
            SlotKind::Tray => {
                let p = glass::cursor_pos();
                tray::click(&sl.key, tray::Click::Middle, p.x, p.y);
            }
            SlotKind::Vol => {
                self.sh.status.toggle_mute();
                self.render(i);
            }
            _ => {}
        }
    }

    fn dock_wheel(&mut self, i: usize, x: f32, y: f32, delta: f32) {
        let g = self.docks[i].geo(&self.cfg);
        let slot = dock::slot_at(&self.docks[i], x, y, &g);
        if let Some(s) = slot {
            let sl = self.docks[i].slots[s].clone();
            match sl.kind {
                SlotKind::Widget if sl.stack > 1 => {
                    if let (Some(wsi), Some(item)) = (self.cfg.ws_index(self.docks[i].ws), sl.item) {
                        if let Some(DockItem::Widgets { ids, cur, .. }) = self.cfg.workspaces[wsi].items.get_mut(item) {
                            let n = ids.len();
                            *cur = if delta < 0.0 { (*cur + 1) % n } else { (*cur + n - 1) % n };
                            self.mark_save();
                        }
                    }
                    self.rebuild(i);
                    return;
                }
                SlotKind::Vol => {
                    self.sh.status.change_volume(delta * 0.02);
                    self.render(i);
                    return;
                }
                _ => {}
            }
        }
        if self.cfg.general.wheel_switches_ws {
            self.cycle_ws(i, if delta < 0.0 { 1 } else { -1 });
        }
    }

    // -----------------------------------------------------------------------
    // Menus
    // -----------------------------------------------------------------------

    fn dock_menu(&mut self, i: usize, x: f32, y: f32) {
        let g = self.docks[i].geo(&self.cfg);
        let slot = dock::slot_at(&self.docks[i], x, y, &g);
        let mut items: Vec<MItem> = vec![];
        let ws_id = self.docks[i].ws;
        let wsi = self.cfg.ws_index(ws_id).unwrap_or(0);
        let mut bar_menu = true;
        if let Some(s) = slot {
            let sl = self.docks[i].slots[s].clone();
            bar_menu = false;
            match sl.kind {
                SlotKind::Pinned | SlotKind::Running => {
                    let app = self.slot_app(i, s);
                    if sl.wins.len() > 1 {
                        for h in sl.wins.iter().take(12) {
                            let t = winlist::window_title(*h);
                            let t = if t.chars().count() > 60 { t.chars().take(58).collect::<String>() + "…" } else { t };
                            items.push(mic(&t, Act::Activate(h.0 as isize), *h == self.wins.fg));
                        }
                        items.push(MItem::Sep);
                    }
                    if let Some(a) = &app {
                        items.push(mi("Nova janela", Act::NewWindow(a.clone())));
                        items.push(MItem::Sep);
                        if sl.kind == SlotKind::Pinned {
                            items.push(mi("Remover do Dock", Act::Unpin(sl.item.unwrap_or(0))));
                        } else {
                            items.push(mi("Manter no Dock", Act::Pin(a.clone(), sl.key.clone())));
                        }
                    }
                    if !sl.wins.is_empty() && self.cfg.workspaces.len() > 1 {
                        let hs: Vec<isize> = sl.wins.iter().map(|h| h.0 as isize).collect();
                        let sub: Vec<MItem> = self.cfg.workspaces.iter().filter(|w| w.id != ws_id).map(|w| mi(&w.name, Act::MoveTo(hs.clone(), w.id))).collect();
                        items.push(MItem::Sub("Mover janelas para".into(), sub));
                    }
                    if let Some(a) = &app {
                        if !a.exe.is_empty() && !a.exe.to_lowercase().ends_with("applicationframehost.exe") {
                            items.push(mi("Mostrar no Explorador", Act::Explorer(a.exe.clone())));
                        }
                    }
                    if !sl.wins.is_empty() {
                        items.push(MItem::Sep);
                        let hs: Vec<isize> = sl.wins.iter().map(|h| h.0 as isize).collect();
                        items.push(mi(if sl.wins.len() > 1 { "Fechar todas as janelas" } else { "Fechar janela" }, Act::Close(hs)));
                        items.push(mi("Encerrar", Act::Quit(self.wins.get(sl.wins[0]).map(|w| w.key.clone()).unwrap_or_default())));
                        let pids: Vec<u32> = sl.wins.iter().filter_map(|h| self.wins.get(*h).map(|w| w.pid)).collect();
                        items.push(mi("Forçar encerramento", Act::Kill(pids)));
                        if sl.kind == SlotKind::Running {
                            if let Some(w) = self.wins.get(sl.wins[0]) {
                                let key = if !w.aumid.is_empty() && w.exe.to_lowercase().ends_with("applicationframehost.exe") {
                                    w.aumid.to_lowercase()
                                } else {
                                    std::path::Path::new(&w.exe).file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()
                                };
                                if !key.is_empty() {
                                    items.push(MItem::Sep);
                                    items.push(mi("Não mostrar como aberto", Act::HideApp(key)));
                                }
                            }
                        }
                    }
                }
                SlotKind::Widget => {
                    let wid = sl.widget;
                    if let Some(w) = self.widgets.get(&wid) {
                        for (mid, t) in w.menu() {
                            items.push(mi(&t, Act::WidgetMenu(wid, mid)));
                        }
                        if !w.menu().is_empty() {
                            items.push(MItem::Sep);
                        }
                    }
                    items.push(mi("Editar widget…", Act::EditWidget(wid)));
                    if let Some(item) = sl.item {
                        let mut sub = vec![];
                        for (k, it) in self.cfg.workspaces[wsi].items.iter().enumerate() {
                            if k == item {
                                continue;
                            }
                            if let DockItem::Widgets { ids, .. } = it {
                                if let Some(other) = ids.first() {
                                    let same = self.cfg.widget(*other).map(|w| w.units) == self.cfg.widget(wid).map(|w| w.units);
                                    if same {
                                        let n = self.cfg.widget(*other).map(|w| w.name.clone()).unwrap_or_default();
                                        sub.push(mi(&n, Act::StackWith(item, *other)));
                                    }
                                }
                            }
                        }
                        if !sub.is_empty() {
                            items.push(MItem::Sub("Empilhar com".into(), sub));
                        }
                        if sl.stack > 1 {
                            items.push(mi("Tirar da pilha", Act::Unstack(item)));
                            items.push(mi("Desfazer a pilha", Act::UnstackAll(item)));
                        }
                        items.push(mi("Remover do Dock", Act::RemoveItem(item)));
                    }
                }
                SlotKind::Start => {
                    items.push(mi("Menu de acesso rápido (Win+X)", Act::Keys(vec![VK_LWIN.0, VK_X.0])));
                    items.push(mi("Gerenciador de Tarefas", Act::Open("taskmgr.exe".into())));
                    items.push(mi("Configurações", Act::Open("ms-settings:".into())));
                    if let Some(item) = sl.item {
                        items.push(MItem::Sep);
                        items.push(mi("Remover do Dock", Act::RemoveItem(item)));
                    }
                }
                SlotKind::Folder => {
                    if let Some(item) = sl.item {
                        let (list, sort) = match self.cfg.workspaces[wsi].items.get(item) {
                            Some(DockItem::Folder { list, sort, .. }) => (*list, *sort),
                            _ => (false, 0),
                        };
                        items.push(MItem::Sub("Ver como".into(), vec![mic("Grade", Act::FolderList(item, false), !list), mic("Lista", Act::FolderList(item, true), list)]));
                        items.push(MItem::Sub(
                            "Ordenar por".into(),
                            vec![mic("Nome", Act::FolderSort(item, 0), sort == 0), mic("Tipo", Act::FolderSort(item, 1), sort == 1), mic("Data de modificação", Act::FolderSort(item, 2), sort == 2)],
                        ));
                        items.push(mi("Abrir no Explorador", Act::Open(sl.icon.clone())));
                        items.push(MItem::Sep);
                        items.push(mi("Remover do Dock", Act::RemoveItem(item)));
                    }
                }
                SlotKind::Downloads => items.push(mi("Abrir no Explorador", Act::Open(sl.icon.clone()))),
                SlotKind::Trash => {
                    items.push(mi("Abrir", Act::Open("shell:RecycleBinFolder".into())));
                    items.push(mi("Esvaziar Lixeira", Act::EmptyTrash));
                }
                SlotKind::Tray => {
                    let r = self.slot_screen_rect(i, s);
                    tray::click(&sl.key, tray::Click::Right, (r.left + r.right) / 2, r.top);
                    return;
                }
                SlotKind::Net => {
                    items.push(mi("Configurações de rede e internet", Act::Open("ms-settings:network".into())));
                    items.push(mi("Wi-Fi disponíveis", Act::Open("ms-availablenetworks:".into())));
                    items.push(MItem::Sep);
                    items.push(mi("Configurações da barra", Act::Settings));
                }
                SlotKind::Vol => {
                    items.push(mi("Mixer de volume", Act::Open("ms-settings:apps-volume".into())));
                    items.push(mi("Configurações de som", Act::Open("ms-settings:sound".into())));
                    items.push(mi(if self.sh.status.muted { "Ativar som" } else { "Sem som" }, Act::Keys(vec![VK_VOLUME_MUTE.0])));
                }
                SlotKind::Bat => {
                    items.push(mi("Energia e bateria", Act::Open("ms-settings:batterysaver".into())));
                }
                SlotKind::Clock => {
                    items.push(mi("Ajustar data e hora", Act::Open("ms-settings:dateandtime".into())));
                    items.push(mi("Central de notificações", Act::Keys(vec![VK_LWIN.0, VK_N.0])));
                    items.push(mi("Ícones da bandeja…", Act::TrayPanel));
                }
                SlotKind::Chevron => {
                    items.push(mi("Escolher ícones na barra…", Act::Settings));
                }
                SlotKind::Spacer => {
                    if let Some(item) = sl.item {
                        items.push(mi("Remover espaçador", Act::RemoveItem(item)));
                        items.push(MItem::Sep);
                    }
                    bar_menu = true;
                }
                SlotKind::Sep if sl.item.is_some() => {
                    items.push(mi("Remover separador", Act::RemoveItem(sl.item.unwrap())));
                    items.push(MItem::Sep);
                    bar_menu = true;
                }
                SlotKind::WsDots | SlotKind::Sep => bar_menu = true,
            }
            if !bar_menu {
                items.push(MItem::Sep);
                items.push(mi("Widgets…", Act::Widgets));
                items.push(mi("Ajustes do Dock…", Act::Settings));
            }
        }
        if bar_menu {
            items.push(mi("Adicionar espaçador pequeno", Act::AddSpacer(false)));
            items.push(mi("Adicionar espaçador grande", Act::AddSpacer(true)));
            items.push(mi("Adicionar separador │", Act::AddDivider));
            items.push(MItem::Sep);
            items.push(mic("Ocultar automaticamente", Act::ToggleAutohide, self.cfg.general.autohide));
            let h = self.cfg.appearance.hover;
            items.push(MItem::Sub(
                "Efeito ao passar o mouse".into(),
                vec![mic("Nenhum", Act::SetHover(Hover::None), h == Hover::None), mic("Escala", Act::SetHover(Hover::Scale), h == Hover::Scale), mic("Ampliação", Act::SetHover(Hover::Magnify), h == Hover::Magnify)],
            ));
            items.push(mic("Mostrar apps abertos", Act::ToggleRunning, self.cfg.general.show_running));
            let mut sub: Vec<MItem> = self.cfg.workspaces.iter().enumerate().map(|(k, w)| mic(&format!("{}\tCtrl+Alt+{}", w.name, k + 1), Act::SwitchWs(w.id), w.id == ws_id)).collect();
            sub.push(MItem::Sep);
            sub.push(mi("Novo workspace…", Act::NewWs));
            items.push(MItem::Sub("Workspace".into(), sub));
            items.push(MItem::Sep);
            if self.cfg.general.show_status {
                items.push(mi("Ícones da bandeja…", Act::TrayPanel));
            }
            items.push(mi("Widgets…", Act::Widgets));
            items.push(mi("Ajustes do Dock…", Act::Settings));
            items.push(MItem::Sep);
            items.push(mi("Sair do GlassDock", Act::Exit));
        }
        self.close_preview();
        let p = glass::cursor_pos();
        self.popup_menu(items, p.x, p.y, i);
    }

    pub fn popup_menu(&mut self, items: Vec<MItem>, x: i32, y: i32, dock_i: usize) {
        let mut acts: Vec<Act> = vec![];
        let hm = unsafe { build_menu(&items, &mut acts) };
        let main = self.main;
        self.menu_open = true;
        let bottom = self.cfg.general.position == DockPos::Bottom;
        defer(move || unsafe {
            let prev = GetForegroundWindow();
            let _ = SetForegroundWindow(main);
            let flags = TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | if bottom { TPM_BOTTOMALIGN } else { TPM_TOPALIGN };
            let cmd = TrackPopupMenuEx(hm, flags.0, x, y, main, None).0;
            let _ = PostMessageW(Some(main), WM_NULL, WPARAM(0), LPARAM(0));
            let _ = DestroyMenu(hm);
            with_app(|a| {
                a.menu_open = false;
                if cmd > 0 {
                    if let Some(act) = acts.get(cmd as usize - 1).cloned() {
                        a.do_act(act, dock_i);
                    }
                }
                if a.cfg.general.autohide {
                    SetTimer(Some(a.main), T_HIDE, 700, None);
                }
            });
            if GetForegroundWindow() == main && !prev.0.is_null() {
                let _ = SetForegroundWindow(prev);
            }
        });
    }

    pub fn do_act(&mut self, act: Act, di: usize) {
        let di = di.min(self.docks.len().saturating_sub(1));
        let ws_id = self.docks.get(di).map(|d| d.ws).unwrap_or(1);
        let wsi = self.cfg.ws_index(ws_id).unwrap_or(0);
        match act {
            Act::Activate(h) => winlist::activate(HWND(h as _)),
            Act::NewWindow(a) => {
                winlist::launch(&a, true);
            }
            Act::Pin(a, old_key) => {
                // no modo livre, o app fixado fica onde já estava
                let new_key = format!("pin:{}", a.launch.to_lowercase());
                for k in self.cfg.workspaces[wsi].app_order.iter_mut() {
                    if *k == old_key {
                        *k = new_key.clone();
                    }
                }
                let items = &mut self.cfg.workspaces[wsi].items;
                let pos = items.iter().rposition(|x| matches!(x, DockItem::App(_) | DockItem::Start)).map(|p| p + 1).unwrap_or(items.len());
                items.insert(pos, DockItem::App(a));
                self.mark_save();
                self.rebuild_all();
            }
            Act::Unpin(item) | Act::RemoveItem(item) => {
                if item < self.cfg.workspaces[wsi].items.len() {
                    self.cfg.workspaces[wsi].items.remove(item);
                    self.mark_save();
                    self.rebuild_all();
                }
            }
            Act::MoveTo(hs, ws) => {
                for h in hs {
                    let hw = HWND(h as _);
                    if let Some(w) = self.wins.get_mut(hw) {
                        w.ws = ws;
                        if ws != ws_id {
                            winlist::minimize_quiet(hw);
                            w.min_by_us = true;
                        }
                    }
                }
                self.wins.dirty_save = true;
                self.rebuild_all();
            }
            Act::Close(hs) => {
                for h in hs {
                    winlist::close(HWND(h as _));
                }
            }
            Act::Quit(key) => {
                for w in self.wins.wins.iter().filter(|w| w.key == key) {
                    winlist::close(w.hwnd);
                }
            }
            Act::Kill(pids) => {
                let mut p = pids;
                p.dedup();
                for pid in p {
                    winlist::kill_process(pid);
                }
            }
            Act::Explorer(p) => show_in_explorer(&p),
            Act::WidgetMenu(wid, mid) => {
                if mid == 1 && self.widgets.get(&wid).map(|w| w.menu().first().map(|m| m.1.starts_with("Abrir") || m.1.starts_with("Ver")).unwrap_or(false)).unwrap_or(false) {
                    // "Abrir painel"/"Ver as cores": abre o painel do widget
                    if let Some(s) = self.docks[di].slots.iter().position(|s| s.widget == wid) {
                        let anchor = self.slot_screen_rect(di, s);
                        let key = self.docks[di].slots[s].key.clone();
                        self.open_panel(PanelKind::Widget(wid), di, anchor, key);
                    }
                    return;
                }
                self.sh.current_widget = wid;
                if let Some(w) = self.widgets.get_mut(&wid) {
                    w.on_menu(mid, &mut self.sh);
                }
                self.process_requests();
                self.render_all();
                self.schedule_pet();
            }
            Act::FileMenu(mid) => {
                if let Some(pn) = self.panel.as_mut() {
                    if let Some(path) = pn.menu_target.take() {
                        match mid {
                            1 => shell_open(&path),
                            2 => show_in_explorer(&path),
                            3 => crate::dragdrop::recycle(&[path]),
                            4 => crate::dragdrop::copy_files_to_clipboard(&[path]),
                            _ => {}
                        }
                    }
                }
                self.render_panel();
            }
            Act::EditWidget(wid) => self.open_settings(Some(crate::settings::Page::EditWidget(wid))),
            Act::StackWith(item, other) => {
                let items = &mut self.cfg.workspaces[wsi].items;
                let other_idx = items.iter().position(|x| matches!(x, DockItem::Widgets { ids, .. } if ids.first() == Some(&other)));
                if let Some(oi) = other_idx {
                    if oi != item {
                        let moved = match items.get(item) {
                            Some(DockItem::Widgets { ids, .. }) => ids.clone(),
                            _ => vec![],
                        };
                        if let Some(DockItem::Widgets { ids, .. }) = items.get_mut(oi) {
                            ids.extend(moved);
                        }
                        items.remove(item);
                    }
                }
                self.mark_save();
                self.rebuild_all();
            }
            Act::Unstack(item) => {
                let items = &mut self.cfg.workspaces[wsi].items;
                if let Some(DockItem::Widgets { ids, cur, zone }) = items.get_mut(item) {
                    if ids.len() > 1 {
                        let c = (*cur).min(ids.len() - 1);
                        let w = ids.remove(c);
                        *cur = 0;
                        let z = *zone;
                        items.insert(item + 1, DockItem::Widgets { ids: vec![w], cur: 0, zone: z });
                    }
                }
                self.mark_save();
                self.rebuild_all();
            }
            Act::UnstackAll(item) => {
                let items = &mut self.cfg.workspaces[wsi].items;
                if let Some(DockItem::Widgets { ids, zone, .. }) = items.get(item).cloned() {
                    items.remove(item);
                    for (k, w) in ids.iter().enumerate() {
                        items.insert(item + k, DockItem::Widgets { ids: vec![*w], cur: 0, zone });
                    }
                }
                self.mark_save();
                self.rebuild_all();
            }
            Act::AddSpacer(large) => {
                let items = &mut self.cfg.workspaces[wsi].items;
                let pos = items.iter().rposition(|x| matches!(x, DockItem::App(_) | DockItem::Start)).map(|p| p + 1).unwrap_or(items.len());
                items.insert(pos, DockItem::Spacer { large });
                self.mark_save();
                self.rebuild_all();
            }
            Act::TrayPanel => {
                let anchor = self.status_anchor(di);
                self.open_panel(PanelKind::Tray, di, anchor, "chevron".into());
            }
            Act::AddDivider => {
                let items = &mut self.cfg.workspaces[wsi].items;
                let pos = items.iter().rposition(|x| matches!(x, DockItem::App(_) | DockItem::Start)).map(|p| p + 1).unwrap_or(items.len());
                items.insert(pos, DockItem::Divider);
                self.mark_save();
                self.rebuild_all();
            }
            Act::ToggleAutohide => {
                self.cfg.general.autohide = !self.cfg.general.autohide;
                self.apply_general();
            }
            Act::SetHover(h) => {
                self.cfg.appearance.hover = h;
                self.mark_save();
                self.render_all();
            }
            Act::ToggleRunning => {
                self.cfg.general.show_running = !self.cfg.general.show_running;
                self.mark_save();
                self.rebuild_all();
            }
            Act::SwitchWs(id) => self.switch_ws(di, id),
            Act::Widgets => self.open_settings(Some(crate::settings::Page::Widgets)),
            Act::Settings => self.open_settings(None),
            Act::NewWs => self.open_settings(Some(crate::settings::Page::Workspaces)),
            Act::Exit => self.quit(),
            Act::EmptyTrash => {
                crate::dragdrop::empty_recycle_bin(self.main);
                self.trash_full = crate::dragdrop::recycle_bin_full();
                self.render_all();
            }
            Act::Open(p) => shell_open(&p),
            Act::FolderList(item, list) => {
                if let Some(DockItem::Folder { list: l, .. }) = self.cfg.workspaces[wsi].items.get_mut(item) {
                    *l = list;
                }
                self.mark_save();
            }
            Act::FolderSort(item, sort) => {
                if let Some(DockItem::Folder { sort: s, .. }) = self.cfg.workspaces[wsi].items.get_mut(item) {
                    *s = sort;
                }
                self.mark_save();
            }
            Act::Keys(keys) => {
                let vks: Vec<VIRTUAL_KEY> = keys.iter().map(|k| VIRTUAL_KEY(*k)).collect();
                winlist::send_keys(&vks);
            }
            Act::TrayPin(key, on) => {
                self.set_tray_pin(&key, on);
            }
            Act::HideApp(key) => {
                if !self.cfg.hidden_apps.contains(&key) {
                    self.cfg.hidden_apps.push(key);
                }
                self.mark_save();
                self.rebuild_all();
            }
        }
    }

    pub fn set_tray_pin(&mut self, key: &str, on: bool) {
        let exe = key.split('|').next().unwrap_or("").to_string();
        if on {
            if !self.cfg.tray_pinned.iter().any(|k| k == key) {
                self.cfg.tray_pinned.push(key.to_string());
            }
        } else {
            self.cfg.tray_pinned.retain(|k| k != key && *k != format!("{}|*", exe));
        }
        self.mark_save();
        self.rebuild_all();
    }

    // -----------------------------------------------------------------------
    // Painéis
    // -----------------------------------------------------------------------

    pub fn toggle_panel(&mut self, kind: PanelKind, dock: usize, anchor: RECT, key: String) {
        if let Some(p) = &self.panel {
            if p.key == key {
                self.close_panel();
                return;
            }
        }
        // o clique que fechou o painel (perda de foco) não deve reabri-lo
        if self.last_panel_close.0 == key && tick_ms().saturating_sub(self.last_panel_close.1) < 250 {
            return;
        }
        self.open_panel(kind, dock, anchor, key);
    }

    pub fn open_panel(&mut self, kind: PanelKind, dock: usize, anchor: RECT, key: String) {
        self.close_panel();
        self.close_preview();
        if matches!(kind, PanelKind::Calendar) && self.clock_cal.is_none() {
            // usa o calendário do usuário, se houver (mostra a agenda dele)
            let data = self.cfg.widgets.iter().find(|w| w.kind == "calendar").map(|w| w.data.clone()).unwrap_or(serde_json::Value::Null);
            self.clock_cal = Some(widgets::make("calendar", &data));
        }
        let mut p = Popup::new(K_PANEL_PLATE, K_PANEL, 0, true, false);
        let dpi = self.docks.get(dock).map(|d| d.dpi).unwrap_or(96);
        p.scale = scale(dpi);
        p.margin = (22.0 * p.scale) as i32;
        p.radius = 16.0;
        let pn = Panel { p, kind, dock, anchor, key, modal: false, opened: tick_ms(), scroll_target: None, menu_target: None, drag_from: None };
        self.panel = Some(pn);
        self.render_panel();
        if let Some(pn) = &self.panel {
            pn.p.show();
            unsafe {
                let _ = SetForegroundWindow(pn.p.hwnd);
            }
        }
    }

    pub fn close_panel(&mut self) {
        if let Some(mut pn) = self.panel.take() {
            self.last_panel_close = (pn.key.clone(), tick_ms());
            pn.p.destroy();
            unsafe {
                let _ = KillTimer(Some(self.main), T_PANEL);
            }
            if self.cfg.general.autohide {
                unsafe {
                    SetTimer(Some(self.main), T_HIDE, 700, None);
                }
            }
            self.render_all();
        }
    }

    fn panel_size(&mut self, kind: &PanelKind) -> (f32, f32) {
        let (w, h) = match kind {
            PanelKind::Widget(id) => self.widgets.get(id).map(|w| w.panel_size(&self.sh)).unwrap_or((300.0, 200.0)),
            PanelKind::Folder(path, list) => {
                let n = crate::widgets::files::scan_dir(path, 40, false).len().max(1);
                if *list {
                    (320.0, (60.0 + n.min(12) as f32 * 34.0).min(480.0))
                } else {
                    (390.0, 70.0 + ((n + 3) / 4).min(4) as f32 * 104.0)
                }
            }
            PanelKind::Tray => {
                let n = tray::visible_icons().len().max(1);
                let cols = n.clamp(4, 6);
                (cols as f32 * 52.0, 64.0 + ((n + cols - 1) / cols) as f32 * 52.0 + 24.0)
            }
            PanelKind::Calendar => self.clock_cal.as_ref().map(|c| c.panel_size(&self.sh)).unwrap_or((320.0, 360.0)),
        };
        (w + 32.0, h + 32.0)
    }

    pub fn render_panel(&mut self) {
        let Some(mut pn) = self.panel.take() else { return };
        let (w, h) = self.panel_size(&pn.kind);
        let sc = pn.p.scale;
        let (pos, work) = self.docks.get(pn.dock).map(|d| (self.cfg.general.position, d.work)).unwrap_or((DockPos::Bottom, RECT::default()));
        let inner = place_near(pn.anchor, (w * sc) as i32, (h * sc) as i32, pos, work, (12.0 * sc) as i32);
        if inner != pn.p.inner_rect() {
            pn.p.place(inner);
        }
        self.sh.panel_scale = sc;
        self.sh.close_panel = false;
        let kind = pn.kind.clone();
        let mut menu_target = pn.menu_target.clone();
        let mut drag_from = pn.drag_from.clone();
        let (_changed, close) = pn.p.paint_ui(false, |ui| {
            self.panel_content(&kind, ui, &mut menu_target, &mut drag_from);
        });
        pn.menu_target = menu_target;
        pn.drag_from = drag_from;
        // atualização periódica
        let iv = match &pn.kind {
            PanelKind::Widget(id) => self.widgets.get(id).and_then(|w| w.panel_interval(&self.sh)),
            PanelKind::Calendar => Some(60000),
            _ => None,
        };
        unsafe {
            match iv {
                Some(ms) => {
                    SetTimer(Some(self.main), T_PANEL, ms, None);
                }
                None => {
                    let _ = KillTimer(Some(self.main), T_PANEL);
                }
            }
        }
        self.panel = Some(pn);
        if close || self.sh.close_panel {
            self.sh.close_panel = false;
            self.close_panel();
        }
        self.process_requests();
    }

    fn panel_content(&mut self, kind: &PanelKind, ui: &mut Ui, menu_target: &mut Option<String>, drag_from: &mut Option<(String, f32, f32)>) {
        match kind {
            PanelKind::Widget(id) => {
                if let Some(w) = self.widgets.get_mut(id) {
                    self.sh.current_widget = *id;
                    w.panel(ui, &mut self.sh);
                }
            }
            PanelKind::Calendar => {
                if let Some(c) = self.clock_cal.as_mut() {
                    c.panel(ui, &mut self.sh);
                }
            }
            PanelKind::Folder(path, list) => self.folder_panel(ui, path, *list, menu_target, drag_from),
            PanelKind::Tray => self.tray_panel(ui),
        }
    }

    fn folder_panel(&mut self, ui: &mut Ui, path: &str, list: bool, menu_target: &mut Option<String>, drag_from: &mut Option<(String, f32, f32)>) {
        let name = std::path::Path::new(path).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
        let r = ui.row(28.0);
        ui.c.text(&name, Rect::new(r.x, r.y, r.w - 150.0 * ui.s, r.h), 15.0 * ui.s, W_SEMIBOLD, TXT, Align::Left);
        if ui.button_k(id("open"), Rect::new(r.r() - 140.0 * ui.s, r.y, 140.0 * ui.s, r.h), "Abrir no Explorador", Btn::Ghost) {
            shell_open(path);
            self.sh.close_panel = true;
        }
        ui.gap(6.0);
        let entries = crate::widgets::files::scan_dir(path, 40, false);
        if entries.is_empty() {
            ui.note("Pasta vazia.");
            return;
        }
        let area = Rect::new(ui.area.x, ui.y, ui.area.w, ui.area.b() - ui.y);
        let (cw, ch, cols) = if list { (area.w, 34.0 * ui.s, 1) } else { (area.w / 4.0, 104.0 * ui.s, 4) };
        let content_h = ((entries.len() + cols - 1) / cols) as f32 * ch;
        let off = ui.begin_scroll(id("fsc"), area, content_h);
        for (k, e) in entries.iter().enumerate() {
            let cell = Rect::new(area.x + cw * (k % cols) as f32, area.y + ch * (k / cols) as f32 - off, cw, ch);
            if cell.b() < area.y || cell.y > area.b() {
                continue;
            }
            let (hov, clk) = ui.hit(idn("fe", k as u64), cell);
            if hov {
                ui.c.fill_rrect(cell.inset(2.0), 8.0 * ui.s, FILL);
            }
            if list {
                if let Some(ic) = self.sh.icons.get(&e.path, 32) {
                    ui.c.bitmap(&ic.bmp, Rect::new(cell.x + 6.0 * ui.s, cell.y + 5.0 * ui.s, 24.0 * ui.s, 24.0 * ui.s), 1.0);
                }
                ui.c.text(&e.name, Rect::new(cell.x + 38.0 * ui.s, cell.y, cell.w - 44.0 * ui.s, cell.h), 12.5 * ui.s, W_NORMAL, TXT, Align::Left);
            } else {
                let ir = Rect::new(cell.cx() - 30.0 * ui.s, cell.y + 8.0 * ui.s, 60.0 * ui.s, 60.0 * ui.s);
                let l = e.path.to_lowercase();
                let img = if [".png", ".jpg", ".jpeg"].iter().any(|x| l.ends_with(x)) { self.sh.icons.thumb(&e.path, 128) } else { self.sh.icons.get(&e.path, 64) };
                if let Some(ic) = img {
                    ui.c.bitmap(&ic.bmp, ir, 1.0);
                }
                ui.c.text_ex(&e.name, Rect::new(cell.x + 4.0, ir.b() + 4.0 * ui.s, cell.w - 8.0, 30.0 * ui.s), 10.5 * ui.s, W_NORMAL, TXT, Align::Center, VAlign::Top, 0, true);
            }
            if clk {
                shell_open(&e.path);
                self.sh.close_panel = true;
            }
            if hov && ui.inp.pressed {
                *drag_from = Some((e.path.clone(), ui.inp.mx, ui.inp.my));
            }
            if ui.right_clicked(cell) {
                *menu_target = Some(e.path.clone());
                self.sh.popup = Some(vec![(1, "Abrir".into()), (2, "Mostrar no Explorador".into()), (4, "Copiar".into()), (0, String::new()), (3, "Mover para a Lixeira".into())]);
                self.sh.current_widget = u32::MAX;
            }
        }
        ui.end_scroll();
        if let Some((p, x0, y0)) = drag_from.clone() {
            if !ui.inp.down {
                *drag_from = None;
            } else if (ui.inp.mx - x0).abs() + (ui.inp.my - y0).abs() > 8.0 * ui.s {
                self.sh.drag_out = Some(vec![p]);
                *drag_from = None;
                ui.st.active = None;
            }
        }
    }

    fn tray_panel(&mut self, ui: &mut Ui) {
        let r = ui.row(24.0);
        ui.c.text("Ícones da bandeja", r, 13.5 * ui.s, W_SEMIBOLD, TXT, Align::Left);
        let r2 = ui.row(18.0);
        ui.c.text("Clique para abrir · alfinete (canto do ícone) para fixar na dock", r2, 11.0 * ui.s, W_NORMAL, TXT2, Align::Left);
        ui.gap(6.0);
        let icons_now = tray::visible_icons();
        let n = icons_now.len();
        if n == 0 {
            ui.note("Nenhum ícone de app agora.");
            return;
        }
        let cols = n.clamp(4, 6);
        let cw = ui.area.w / cols as f32;
        let chh = 52.0 * ui.s;
        let y0 = ui.y;
        let mut pin_toggle: Option<(String, bool)> = None;
        for (k, ic) in icons_now.iter().enumerate() {
            let cell = Rect::new(ui.area.x + cw * (k % cols) as f32, y0 + chh * (k / cols) as f32, cw, chh);
            let (hov, clk) = ui.hit(idn("ti", k as u64), cell);
            let pinned = self.cfg.tray_pinned.iter().any(|p| p == &ic.key || {
                let exe = ic.key.split('|').next().unwrap_or("");
                *p == format!("{}|*", exe)
            });
            if hov {
                ui.c.fill_rrect(cell.inset(2.0), 8.0 * ui.s, FILL_HOVER);
            }
            let key = format!("tray:{}:{}", ic.key, ic.hicon.0 as usize);
            let bmp = self.sh.icons.get_hicon_cached(&key).or_else(|| self.sh.icons.put_hicon(&key, ic.hicon));
            if let Some(b) = bmp {
                ui.c.bitmap(&b.bmp, cell.centered(24.0 * ui.s, 24.0 * ui.s), 1.0);
            }
            // alfinete: grande o bastante para acertar, e visível ao passar o mouse
            let pr = Rect::new(cell.r() - 22.0 * ui.s, cell.y + 1.0 * ui.s, 20.0 * ui.s, 20.0 * ui.s);
            let mut on_pin = false;
            if hov || pinned {
                let (ph, pc) = ui.hit(idn("tp", k as u64), pr);
                on_pin = ph;
                if ph {
                    ui.c.fill_circle(pr.cx(), pr.cy(), 10.0 * ui.s, FILL_HOVER);
                }
                ui.c.glyph(if pinned { 0xE841 } else { 0xE718 }, pr, 11.0 * ui.s, if pinned { ACCENT } else if ph { TXT } else { TXT2 });
                if pc {
                    pin_toggle = Some((ic.key.clone(), !pinned));
                    continue;
                }
            }
            // clicar num ícone abre o app e fecha o painel
            if clk && !on_pin {
                let rp = self.panel.as_ref().map(|p| p.p.rect).unwrap_or(RECT::default());
                tray::click(&ic.key, tray::Click::Left, rp.left + cell.cx() as i32, rp.top + cell.cy() as i32);
                self.sh.close_panel = true;
            }
            if ui.right_clicked(cell) {
                let rp = self.panel.as_ref().map(|p| p.p.rect).unwrap_or(RECT::default());
                tray::click(&ic.key, tray::Click::Right, rp.left + cell.cx() as i32, rp.top + cell.cy() as i32);
                self.sh.close_panel = true;
            }
            if hov && !ic.tip.is_empty() {
                let tr = Rect::new(ui.area.x, ui.area.b() - 20.0 * ui.s, ui.area.w, 20.0 * ui.s);
                ui.c.text(ic.tip.lines().next().unwrap_or(""), tr, 11.0 * ui.s, W_NORMAL, TXT2, Align::Center);
            }
        }
        if let Some((k, on)) = pin_toggle {
            self.set_tray_pin(&k, on);
        }
    }

    fn panel_msg(&mut self, h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
        let Some(pn) = self.panel.as_mut() else { return None };
        if pn.p.hwnd != h {
            return None;
        }
        match msg {
            WM_MOUSEMOVE => {
                pn.p.mouse(lp);
                pn.p.inp.down = wp.0 & 1 != 0;
                unsafe {
                    let mut tme = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: h, dwHoverTime: 0 };
                    let _ = TrackMouseEvent(&mut tme);
                }
            }
            WM_MOUSELEAVE => {
                pn.p.inp.inside = false;
            }
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
                pn.p.mouse(lp);
                pn.p.inp.pressed = true;
                pn.p.inp.down = true;
                pn.p.inp.dbl = msg == WM_LBUTTONDBLCLK;
                unsafe {
                    SetCapture(h);
                }
            }
            WM_LBUTTONUP => {
                pn.p.mouse(lp);
                pn.p.inp.released = true;
                pn.p.inp.down = false;
                unsafe {
                    let _ = ReleaseCapture();
                }
            }
            WM_RBUTTONUP => {
                pn.p.mouse(lp);
                pn.p.inp.rreleased = true;
            }
            WM_MOUSEWHEEL => {
                pn.p.inp.wheel = ((wp.0 >> 16) & 0xFFFF) as i16 as f32 / 120.0;
            }
            WM_KEYDOWN => {
                let vk = wp.0 as u32;
                if vk == VK_ESCAPE.0 as u32 && pn.p.st.focus.is_none() {
                    self.close_panel();
                    return Some(LRESULT(0));
                }
                let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                pn.p.inp.keys.push((vk, ctrl, shift));
            }
            WM_CHAR => {
                if let Some(c) = char::from_u32(wp.0 as u32) {
                    pn.p.inp.chars.push(c);
                }
            }
            WM_ACTIVATE => {
                if (wp.0 & 0xFFFF) as u32 == WA_INACTIVE && !pn.modal && !self.menu_open {
                    let other = HWND(lp.0 as _);
                    let ours = self.docks.iter().any(|d| d.hwnd == other || d.plate == other);
                    if !ours || tick_ms().saturating_sub(pn.opened) > 300 {
                        self.close_panel();
                    }
                }
                return Some(LRESULT(0));
            }
            _ => return None,
        }
        self.render_panel();
        Some(LRESULT(0))
    }

    // -----------------------------------------------------------------------
    // Pedidos feitos pelos widgets
    // -----------------------------------------------------------------------

    pub fn process_requests(&mut self) {
        if self.sh.ics.changed {
            self.sh.ics.changed = false;
            self.cfg.calendars = self.sh.ics.urls.clone();
            self.mark_save();
            self.sh.redraw = true;
        }
        if self.sh.save {
            self.sh.save = false;
            self.mark_save();
        }
        if !self.sh.toasts.is_empty() {
            let list: Vec<widgets::Toast> = self.sh.toasts.drain(..).collect();
            for t in list {
                self.show_toast(&t.title, &t.body, t.sound);
            }
        }
        if let Some(items) = self.sh.popup.take() {
            let wid = self.sh.current_widget;
            let mut mitems = vec![];
            for (id, t) in items {
                if id == 0 && t.is_empty() {
                    mitems.push(MItem::Sep);
                } else if wid == u32::MAX {
                    mitems.push(mi(&t, Act::FileMenu(id)));
                } else {
                    mitems.push(mi(&t, Act::WidgetMenu(wid, id)));
                }
            }
            if let Some(pn) = self.panel.as_mut() {
                pn.modal = true;
            }
            let p = glass::cursor_pos();
            let di = self.panel.as_ref().map(|p| p.dock).unwrap_or(0);
            self.popup_menu(mitems, p.x, p.y, di);
            defer(|| {
                with_app(|a| {
                    if let Some(pn) = a.panel.as_mut() {
                        pn.modal = false;
                        let h = pn.p.hwnd;
                        unsafe {
                            let _ = SetForegroundWindow(h);
                        }
                    }
                });
            });
        }
        if let Some((wid, purpose, kind)) = self.sh.pick.take() {
            let owner = self.settings.as_ref().map(|s| s.p.hwnd).or(self.panel.as_ref().map(|p| p.p.hwnd)).unwrap_or(self.main);
            if let Some(pn) = self.panel.as_mut() {
                pn.modal = true;
            }
            defer(move || {
                let k = match kind {
                    widgets::Pick::Folder => crate::dragdrop::PickKind::Folder,
                    widgets::Pick::Exe => crate::dragdrop::PickKind::Exe,
                    widgets::Pick::Files => crate::dragdrop::PickKind::Any,
                };
                let multi = !matches!(kind, widgets::Pick::Folder);
                let paths = crate::dragdrop::pick(owner, k, multi);
                with_app(|a| {
                    if let Some(pn) = a.panel.as_mut() {
                        pn.modal = false;
                    }
                    if !paths.is_empty() {
                        a.on_picked(wid, purpose, paths);
                    }
                });
            });
        }
        if let Some(files) = self.sh.drag_out.take() {
            let owner = self.panel.as_ref().map(|p| p.p.hwnd).unwrap_or(self.main);
            if let Some(pn) = self.panel.as_mut() {
                pn.modal = true;
            }
            defer(move || {
                crate::dragdrop::drag_files_out(owner, &files);
                with_app(|a| {
                    if let Some(pn) = a.panel.as_mut() {
                        pn.modal = false;
                    }
                    a.close_panel();
                });
            });
        }
        if let Some(wid) = self.sh.eyedrop.take() {
            self.close_panel();
            if self.eyedrop.is_none() {
                self.eyedrop = Eyedrop::start(wid);
                let p = glass::cursor_pos();
                if let Some(e) = self.eyedrop.as_mut() {
                    e.update(p.x, p.y);
                }
            }
        }
        if let Some(app_id) = self.sh.focus_app.take() {
            self.focus_app(&app_id);
        }
        if self.sh.rewatch {
            self.sh.rewatch = false;
            self.rewatch();
            self.register_hotkeys();
        }
        if self.sh.redraw {
            self.sh.redraw = false;
            self.render_all();
        }
    }

    fn on_picked(&mut self, wid: u32, purpose: u32, paths: Vec<String>) {
        if wid == u32::MAX - 1 {
            // seletor dos Ajustes (adicionar app/pasta ao workspace)
            crate::settings::on_picked(self, purpose, paths);
            return;
        }
        if let Some(w) = self.widgets.get_mut(&wid) {
            self.sh.current_widget = wid;
            w.on_picked(purpose, paths, &mut self.sh);
        }
        self.process_requests();
        self.render_all();
        self.render_settings();
        if self.panel.is_some() {
            self.render_panel();
        }
    }

    fn focus_app(&mut self, app_id: &str) {
        let id = app_id.to_lowercase();
        let found = self.wins.wins.iter().filter(|w| {
            let a = w.aumid.to_lowercase();
            let exe = std::path::Path::new(&w.exe).file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            (!a.is_empty() && (a == id || a.contains(&id) || id.contains(&a))) || exe == id || format!("{}.exe", id) == exe || (id == "chrome" && exe == "chrome.exe") || (id == "msedge" && exe == "msedge.exe")
        }).max_by_key(|w| w.last_active).map(|w| w.hwnd);
        match found {
            Some(h) => {
                if self.wins.fg == h {
                    winlist::minimize(h);
                } else {
                    winlist::activate(h);
                }
            }
            None => {}
        }
    }

    // -----------------------------------------------------------------------
    // Prévias
    // -----------------------------------------------------------------------

    fn show_preview(&mut self) {
        let Some((i, s)) = self.preview_pending.take() else { return };
        if i >= self.docks.len() || s >= self.docks[i].slots.len() || self.docks[i].hover != Some(s) || self.panel.is_some() {
            return;
        }
        let sl = self.docks[i].slots[s].clone();
        if sl.wins.is_empty() || self.docks[i].drag.is_some() {
            return;
        }
        if let Some(p) = &self.preview {
            if p.key == sl.key && p.dock == i {
                return;
            }
        }
        self.close_preview();
        let anchor = self.slot_screen_rect(i, s);
        let d = &self.docks[i];
        let mut pv = Preview::new(i, sl.key.clone(), sl.icon.clone(), &sl.wins, anchor, self.cfg.general.position, d.work, d.dpi);
        pv.render(&mut self.sh.icons);
        pv.p.show();
        self.preview = Some(pv);
    }

    pub fn close_preview(&mut self) {
        if let Some(mut p) = self.preview.take() {
            p.destroy();
        }
    }

    fn maybe_close_preview(&mut self) {
        let Some(pv) = &self.preview else { return };
        let p = glass::cursor_pos();
        let r = pv.p.inner_rect();
        let inside = p.x >= r.left && p.x <= r.right && p.y >= r.top - 12 && p.y <= r.bottom + 12;
        let on_slot = self.docks.get(pv.dock).map(|d| d.hover.map(|s| d.slots.get(s).map(|x| x.key == pv.key).unwrap_or(false)).unwrap_or(false)).unwrap_or(false);
        if !inside && !on_slot {
            self.close_preview();
            if self.cfg.general.autohide {
                unsafe {
                    SetTimer(Some(self.main), T_HIDE, 600, None);
                }
            }
        } else {
            unsafe {
                SetTimer(Some(self.main), T_PREVIEW_HIDE, 300, None);
            }
        }
    }

    fn preview_msg(&mut self, msg: u32, _wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
        let pv = self.preview.as_mut()?;
        match msg {
            WM_MOUSEMOVE => {
                pv.p.mouse(lp);
                let hov = pv.item_at(pv.p.inp.mx, pv.p.inp.my);
                unsafe {
                    let mut tme = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: pv.p.hwnd, dwHoverTime: 0 };
                    let _ = TrackMouseEvent(&mut tme);
                }
                pv.hover = hov;
                pv.render(&mut self.sh.icons);
                Some(LRESULT(0))
            }
            WM_MOUSELEAVE => {
                pv.hover = None;
                pv.render(&mut self.sh.icons);
                unsafe {
                    SetTimer(Some(self.main), T_PREVIEW_HIDE, 250, None);
                }
                Some(LRESULT(0))
            }
            WM_LBUTTONUP | WM_MBUTTONUP => {
                pv.p.mouse(lp);
                let (x, y) = (pv.p.inp.mx, pv.p.inp.my);
                if let Some(k) = pv.item_at(x, y) {
                    let h = pv.items[k].hwnd;
                    if msg == WM_MBUTTONUP || pv.close_rect(k).contains(x, y) {
                        winlist::close(h);
                        pv.items.remove(k);
                        if pv.items.is_empty() {
                            self.close_preview();
                        } else {
                            pv.hover = None;
                            pv.render(&mut self.sh.icons);
                        }
                    } else {
                        self.close_preview();
                        winlist::activate(h);
                    }
                }
                Some(LRESULT(0))
            }
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Ajustes
    // -----------------------------------------------------------------------

    pub fn open_settings(&mut self, page: Option<crate::settings::Page>) {
        self.close_panel();
        if self.settings.is_none() {
            let dpi = self.docks.first().map(|d| d.dpi).unwrap_or(96);
            let work = self.docks.first().map(|d| d.work).unwrap_or(RECT { left: 0, top: 0, right: 1280, bottom: 720 });
            self.settings = Some(crate::settings::SettingsWin::new(dpi, work));
        }
        if let Some(p) = page {
            if let Some(s) = self.settings.as_mut() {
                s.page = p;
                s.p.st = UiState::default();
            }
        }
        self.render_settings();
        if let Some(s) = &self.settings {
            s.p.show();
            unsafe {
                let _ = SetForegroundWindow(s.p.hwnd);
            }
        }
    }

    pub fn close_settings(&mut self) {
        if let Some(mut s) = self.settings.take() {
            s.p.destroy();
        }
        self.sh.catalog = None;
        self.do_save();
    }

    pub fn render_settings(&mut self) {
        let Some(mut sw) = self.settings.take() else { return };
        sw.render(self);
        self.settings = Some(sw);
        if self.sh.close_panel {
            self.sh.close_panel = false;
        }
        self.process_requests();
    }

    fn settings_msg(&mut self, h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
        let sw = self.settings.as_mut()?;
        if sw.p.hwnd != h {
            return None;
        }
        match msg {
            WM_MOUSEMOVE => {
                sw.p.mouse(lp);
                sw.p.inp.down = wp.0 & 1 != 0;
            }
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
                sw.p.mouse(lp);
                if sw.in_titlebar(sw.p.inp.mx, sw.p.inp.my) {
                    let hwnd = sw.p.hwnd;
                    defer(move || unsafe {
                        let _ = ReleaseCapture();
                        SendMessageW(hwnd, WM_NCLBUTTONDOWN, Some(WPARAM(HTCAPTION as usize)), Some(LPARAM(0)));
                    });
                    return Some(LRESULT(0));
                }
                sw.p.inp.pressed = true;
                sw.p.inp.down = true;
                unsafe {
                    SetCapture(h);
                }
            }
            WM_LBUTTONUP => {
                sw.p.mouse(lp);
                sw.p.inp.released = true;
                sw.p.inp.down = false;
                unsafe {
                    let _ = ReleaseCapture();
                }
            }
            WM_RBUTTONUP => {
                sw.p.mouse(lp);
                sw.p.inp.rreleased = true;
            }
            WM_MOUSEWHEEL => {
                sw.p.inp.wheel = ((wp.0 >> 16) & 0xFFFF) as i16 as f32 / 120.0;
                // a roda chega com coordenadas de tela
                let p = glass::cursor_pos();
                sw.p.inp.mx = (p.x - sw.p.rect.left) as f32;
                sw.p.inp.my = (p.y - sw.p.rect.top) as f32;
                sw.p.inp.inside = true;
            }
            WM_KEYDOWN => {
                let vk = wp.0 as u32;
                let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                if vk == VK_ESCAPE.0 as u32 && sw.p.st.focus.is_none() {
                    self.close_settings();
                    return Some(LRESULT(0));
                }
                sw.p.inp.keys.push((vk, ctrl, shift));
            }
            WM_CHAR => {
                if let Some(c) = char::from_u32(wp.0 as u32) {
                    sw.p.inp.chars.push(c);
                }
            }
            WM_WINDOWPOSCHANGED => {
                let r = glass::window_rect(h);
                if r != sw.p.rect && r.right > r.left {
                    let dx = r.left - sw.p.rect.left;
                    let dy = r.top - sw.p.rect.top;
                    sw.p.rect = r;
                    let inner = sw.p.inner_rect();
                    let _ = (dx, dy);
                    unsafe {
                        let _ = SetWindowPos(sw.p.plate, None, inner.left, inner.top, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
                    }
                }
                return None;
            }
            WM_CLOSE => {
                self.close_settings();
                return Some(LRESULT(0));
            }
            WM_ACTIVATE => return None,
            _ => return None,
        }
        self.render_settings();
        Some(LRESULT(0))
    }

    /// Reaplica ajustes gerais (barra de tarefas, ocultar, posição…).
    pub fn apply_general(&mut self) {
        self.mark_save();
        self.apply_taskbar();
        taskbar::set_autostart(self.cfg.general.start_with_windows);
        for i in 0..self.docks.len() {
            let auto = self.cfg.general.autohide;
            let d = &mut self.docks[i];
            d.plate_rect = RECT::default();
            if !auto && !d.fullscreen {
                d.hide_target = 0.0;
                if d.hide > 0.5 {
                    glass::show(d.plate, true);
                    glass::show(d.hwnd, true);
                }
                d.hide = 0.0;
            }
            self.setup_appbar(i);
        }
        self.place_triggers();
        self.register_hotkeys();
        self.rebuild_all();
        if self.cfg.general.autohide {
            unsafe {
                SetTimer(Some(self.main), T_HIDE, 1200, None);
            }
        }
    }

    pub fn recreate_docks(&mut self) {
        self.mark_save();
        self.create_docks();
        self.refresh_windows();
    }

    // -----------------------------------------------------------------------
    // Widgets e workspaces (usados pelos Ajustes)
    // -----------------------------------------------------------------------

    pub fn add_widget(&mut self, kind: &str, style: u32, units: u32, data: &str, name: &str) -> u32 {
        let id = self.cfg.alloc_id();
        let v: serde_json::Value = if data.is_empty() { serde_json::Value::Null } else { serde_json::from_str(data).unwrap_or(serde_json::Value::Null) };
        let w = widgets::make(kind, &v);
        let saved = w.save();
        self.widgets.insert(id, w);
        self.cfg.widgets.push(WidgetInstance { id, kind: kind.into(), name: name.into(), style, units, data: saved });
        let di = self.active_dock();
        let ws = self.docks.get(di).map(|d| d.ws).unwrap_or(1);
        if let Some(wsi) = self.cfg.ws_index(ws) {
            self.cfg.workspaces[wsi].items.push(DockItem::Widgets { ids: vec![id], cur: 0, zone: 0 });
        }
        self.mark_save();
        self.ensure_services();
        self.rewatch();
        self.register_hotkeys();
        self.rebuild_all();
        self.schedule_tick();
        id
    }

    pub fn delete_widget(&mut self, id: u32) {
        self.widgets.remove(&id);
        self.cfg.widgets.retain(|w| w.id != id);
        for ws in self.cfg.workspaces.iter_mut() {
            for it in ws.items.iter_mut() {
                if let DockItem::Widgets { ids, cur, .. } = it {
                    ids.retain(|x| *x != id);
                    if *cur >= ids.len() {
                        *cur = 0;
                    }
                }
            }
            ws.items.retain(|it| !matches!(it, DockItem::Widgets { ids, .. } if ids.is_empty()));
        }
        self.mark_save();
        self.rewatch();
        self.rebuild_all();
    }

    /// Mostra/esconde um widget num workspace.
    pub fn set_widget_in_ws(&mut self, id: u32, ws: u32, on: bool) {
        let Some(wsi) = self.cfg.ws_index(ws) else { return };
        let items = &mut self.cfg.workspaces[wsi].items;
        let has = items.iter().any(|it| matches!(it, DockItem::Widgets { ids, .. } if ids.contains(&id)));
        if on && !has {
            items.push(DockItem::Widgets { ids: vec![id], cur: 0, zone: 0 });
        } else if !on && has {
            for it in items.iter_mut() {
                if let DockItem::Widgets { ids, cur, .. } = it {
                    ids.retain(|x| *x != id);
                    if *cur >= ids.len() {
                        *cur = 0;
                    }
                }
            }
            items.retain(|it| !matches!(it, DockItem::Widgets { ids, .. } if ids.is_empty()));
        }
        self.mark_save();
        self.rebuild_all();
    }

    // -----------------------------------------------------------------------
    // Relógio interno: widgets, sistema e pets
    // -----------------------------------------------------------------------

    fn visible_widgets(&self) -> Vec<u32> {
        let mut v = vec![];
        for d in &self.docks {
            if d.hide >= 1.0 && !d.fullscreen && self.cfg.general.autohide {
                // escondida: ainda conta para timers, não para redesenho
            }
            for s in &d.slots {
                if s.kind == SlotKind::Widget {
                    v.push(s.widget);
                }
            }
        }
        v
    }

    fn needs_sys(&self) -> bool {
        let vis = self.visible_widgets();
        let panel_needs = match self.panel.as_ref().map(|p| &p.kind) {
            Some(PanelKind::Widget(id)) => self.widgets.get(id).map(|w| w.needs_sys()).unwrap_or(false),
            _ => false,
        };
        panel_needs || vis.iter().any(|id| self.widgets.get(id).map(|w| w.needs_sys()).unwrap_or(false))
    }

    fn ensure_services(&mut self) {
        let need_media = self.widgets.values().any(|w| w.needs_media());
        if need_media && !self.media_started {
            self.media_started = true;
            self.sh.media_tx = Some(crate::media::start());
        }
    }

    pub fn schedule_tick(&mut self) {
        let now = tick_ms();
        let lt = local_now();
        let ms_into_min = (lt.second * 1000 + lt.ms) as u64;
        let ms_into_sec = lt.ms as u64;
        let mut next = now + 60_000 - ms_into_min + 30;
        let ids: Vec<u32> = self.widgets.keys().copied().collect();
        for id in ids {
            let Some(w) = self.widgets.get(&id) else { continue };
            let Some(iv) = w.interval(&self.sh) else {
                self.due.remove(&id);
                continue;
            };
            let iv = iv.max(250) as u64;
            let due = *self.due.entry(id).or_insert_with(|| {
                if iv == 60_000 {
                    now + 60_000 - ms_into_min + 30
                } else if iv == 1000 {
                    now + 1000 - ms_into_sec + 15
                } else {
                    now + iv
                }
            });
            next = next.min(due);
        }
        let delay = next.saturating_sub(now).clamp(20, 120_000) as u32;
        unsafe {
            SetTimer(Some(self.main), T_TICK, delay, None);
        }
    }

    fn on_tick(&mut self) {
        let now = tick_ms();
        let lt = local_now();
        if self.needs_sys() && now.saturating_sub(self.sys_last) > 900 {
            self.sh.sys.sample();
            self.sys_last = now;
        }
        let vis = self.visible_widgets();
        let mut redraw = false;
        let mut changed_ids: Vec<u32> = vec![];
        let ids: Vec<u32> = self.due.keys().copied().collect();
        for id in ids {
            let due = self.due[&id];
            if due > now + 15 {
                continue;
            }
            let Some(w) = self.widgets.get_mut(&id) else { continue };
            self.sh.current_widget = id;
            let ch = w.tick(&mut self.sh);
            let iv = w.interval(&self.sh).unwrap_or(60_000).max(250) as u64;
            let next = if iv == 60_000 {
                now + 60_000 - (lt.second * 1000 + lt.ms) as u64 + 30
            } else if iv == 1000 {
                now + 1000 - lt.ms as u64 + 15
            } else {
                now + iv
            };
            self.due.insert(id, next);
            if vis.contains(&id) && ch {
                changed_ids.push(id);
            }
        }
        if lt.minute != self.last_minute {
            self.last_minute = lt.minute;
            redraw = true;
            if self.cfg.general.show_clock {
                // a largura do relógio pode mudar (ex.: 9:59 → 10:00)
                let n = self.docks.len();
                for i in 0..n {
                    dock::build(&mut self.docks[i], &self.cfg, &self.wins, &self.widgets, &mut self.sh, n);
                }
            }
        }
        for i in 0..self.docks.len() {
            if self.docks[i].hide >= 1.0 {
                continue;
            }
            if redraw {
                self.render(i);
            } else if !changed_ids.is_empty() {
                self.render_widgets(i, &changed_ids);
            }
        }
        self.process_requests();
        self.schedule_tick();
    }

    fn on_slow(&mut self) {
        if tray::with(|t| !t.own.0.is_null()) {
            if !tray::ensure_first() && tick_ms().saturating_sub(self.last_rebroadcast) > 300_000 {
                self.last_rebroadcast = tick_ms();
                tray::rebroadcast();
            }
            if tray::prune() {
                self.rebuild_all();
            }
        }
        let full = crate::dragdrop::recycle_bin_full();
        if full != self.trash_full {
            self.trash_full = full;
            if self.cfg.general.show_trash {
                self.render_all();
            }
        }
        if self.wins.dirty_save {
            self.wins.save();
        }
        // rede de segurança caso algum evento do Windows se perca
        self.refresh_windows();
        self.taskbar.hide_now();
    }

    pub fn schedule_pet(&mut self) {
        let mut best: Option<u32> = None;
        for d in &self.docks {
            for s in &d.slots {
                if s.kind == SlotKind::Widget {
                    if let Some(w) = self.widgets.get(&s.widget) {
                        if w.is_pet() {
                            let ms = 100;
                            best = Some(best.map(|b| b.min(ms)).unwrap_or(ms));
                        }
                    }
                }
            }
        }
        unsafe {
            match best {
                Some(ms) => {
                    SetTimer(Some(self.main), T_PET, ms, None);
                }
                None => {
                    let _ = KillTimer(Some(self.main), T_PET);
                    for d in &self.docks {
                        glass::show(d.pet_hwnd, false);
                    }
                }
            }
        }
    }

    fn on_pet(&mut self) {
        let now = tick_ms();
        let dt = if self.pet_tick == 0 { 0.1 } else { (now - self.pet_tick) as f32 / 1000.0 };
        self.pet_tick = now;
        if now.saturating_sub(self.sys_last) > 950 {
            self.sh.sys.sample();
            self.sys_last = now;
        }
        let mut next_ms = 1000u32;
        let cpu = self.sh.sys.cpu;
        let mem = self.sh.sys.mem;
        let pos = self.cfg.general.position;
        for di in 0..self.docks.len() {
            let pet_slot = self.docks[di].slots.iter().position(|s| s.kind == SlotKind::Widget && self.widgets.get(&s.widget).map(|w| w.is_pet()).unwrap_or(false));
            let Some(ps) = pet_slot else {
                glass::show(self.docks[di].pet_hwnd, false);
                continue;
            };
            let wid = self.docks[di].slots[ps].widget;
            let d = &mut self.docks[di];
            let Some(pet) = self.widgets.get_mut(&wid).and_then(|w| w.as_pet()) else { continue };
            let hidden = d.hide >= 0.99;
            if hidden && !pet.stays() {
                glass::show(d.pet_hwnd, false);
                continue;
            }
            // percurso: a barra inteira (embaixo) ou o próprio tile
            let tile = d.slots[ps].rect;
            let bar = d.bar;
            let s = d.s * pet.size_k();
            let roam = pet.roams() && pos == DockPos::Bottom;
            let (area_x, area_w, ground_y) = if roam { (bar.x + s * 0.4, bar.w - s * 0.8, bar.y + 2.0) } else { (tile.x + s * 0.3, (tile.w - s * 0.6).max(1.0), tile.b() - tile.h * 0.16) };
            // só redesenha quando o quadro do bicho vence
            if now.saturating_sub(d.pet_last) < pet.frame_ms() as u64 - 15 {
                next_ms = next_ms.min((pet.frame_ms() as u64 - now.saturating_sub(d.pet_last)) as u32);
                continue;
            }
            d.pet_last = now;
            let load = if pet.tracks_mem() { mem } else { cpu };
            pet.step(dt.min(0.5), load, area_w);
            next_ms = next_ms.min(pet.frame_ms());
            let px = area_x + area_w * pet.x;
            let ow = (area_w + s * 2.0).ceil() as i32;
            let oh = (s * 1.25).ceil() as i32;
            let ox = d.win.left + (area_x - s) as i32;
            let mut oy = d.win.top + (ground_y - s * 1.05) as i32;
            if hidden {
                let e = d.mon;
                oy = e.bottom - oh;
            }
            let cx_local = px - (area_x - s);
            let ground_local = s * 1.05;
            let food = pet.food_x().map(|f| area_x + area_w * f - (area_x - s));
            let hwnd = d.pet_hwnd;
            d.pet_surf.paint(hwnd, ox, oy, ow, oh, |c| {
                if let Some(fx) = food {
                    pet.draw_food(c, fx, ground_local, s);
                }
                pet.draw_creature(c, cx_local, ground_local, s);
            });
            if !glass::is_visible(hwnd) {
                glass::show(hwnd, true);
            }
        }
        unsafe {
            SetTimer(Some(self.main), T_PET, next_ms.clamp(50, 1000), None);
        }
    }

    // -----------------------------------------------------------------------
    // Pastas vigiadas e atalhos
    // -----------------------------------------------------------------------

    pub fn rewatch(&mut self) {
        WATCH.with(|w| {
            for h in w.borrow_mut().drain(..) {
                unsafe {
                    let _ = FindCloseChangeNotification(h);
                }
            }
        });
        self.watch_ids.clear();
        let mut handles = vec![];
        for (id, w) in &self.widgets {
            for dir in w.watch_dirs() {
                if dir.is_empty() {
                    continue;
                }
                let wd = wide(&dir);
                unsafe {
                    if let Ok(h) = windows::Win32::Storage::FileSystem::FindFirstChangeNotificationW(pcw(&wd), false, windows::Win32::Storage::FileSystem::FILE_NOTIFY_CHANGE_FILE_NAME | windows::Win32::Storage::FileSystem::FILE_NOTIFY_CHANGE_LAST_WRITE) {
                        if !h.is_invalid() {
                            handles.push(h);
                            self.watch_ids.push(*id);
                        }
                    }
                }
            }
        }
        WATCH.with(|w| *w.borrow_mut() = handles);
    }

    fn on_watch(&mut self, idx: usize) {
        let Some(&id) = self.watch_ids.get(idx) else { return };
        if let Some(w) = self.widgets.get_mut(&id) {
            if w.poll(&mut self.sh) {
                self.render_all();
                if matches!(self.panel.as_ref().map(|p| &p.kind), Some(PanelKind::Widget(x)) if *x == id) {
                    self.render_panel();
                }
            }
        }
    }

    pub fn register_hotkeys(&mut self) {
        unsafe {
            for id in self.hotkeys.drain(..) {
                let _ = UnregisterHotKey(Some(self.main), id);
            }
            let mods = MOD_CONTROL | MOD_ALT | MOD_NOREPEAT;
            if self.cfg.general.ws_hotkeys {
                for k in 1..=9u32 {
                    if RegisterHotKey(Some(self.main), k as i32, mods, 0x30 + k).is_ok() {
                        self.hotkeys.push(k as i32);
                    }
                }
            }
            if self.cfg.general.focus_hotkey && RegisterHotKey(Some(self.main), 20, mods, 0x44).is_ok() {
                self.hotkeys.push(20);
            }
            for (i, (_, w)) in self.widgets.iter().enumerate() {
                if let Some((m, vk)) = w.hotkey() {
                    let id = 100 + i as i32;
                    if RegisterHotKey(Some(self.main), id, HOT_KEY_MODIFIERS(m) | MOD_NOREPEAT, vk).is_ok() {
                        self.hotkeys.push(id);
                    }
                }
            }
        }
    }

    fn on_hotkey(&mut self, id: i32) {
        match id {
            1..=9 => {
                let k = (id - 1) as usize;
                if let Some(ws) = self.cfg.workspaces.get(k) {
                    let wid = ws.id;
                    let di = self.active_dock();
                    self.switch_ws(di, wid);
                    if self.cfg.general.autohide {
                        // mostra a dock por um instante ao trocar
                        self.reveal(di);
                        unsafe {
                            SetTimer(Some(self.main), T_HIDE, 1200, None);
                        }
                    }
                }
            }
            20 => self.focus_dock(),
            30..=36 => self.focus_key(id),
            _ if id >= 100 => {
                let k = (id - 100) as usize;
                if let Some((&wid, _)) = self.widgets.iter().nth(k) {
                    if let Some(w) = self.widgets.get_mut(&wid) {
                        self.sh.current_widget = wid;
                        w.on_hotkey(&mut self.sh);
                    }
                    self.process_requests();
                }
            }
            _ => {}
        }
    }

    /// Ctrl+Alt+D: navegação pelo teclado (setas, Enter/Espaço, Esc).
    fn focus_dock(&mut self) {
        let di = self.active_dock();
        if self.docks.is_empty() {
            return;
        }
        self.reveal(di);
        let first = self.docks[di].slots.iter().position(|s| !matches!(s.kind, SlotKind::Sep | SlotKind::Spacer)).unwrap_or(0);
        self.docks[di].kb_focus = Some(first);
        unsafe {
            for (id, vk) in [(30, VK_LEFT), (31, VK_RIGHT), (32, VK_UP), (33, VK_DOWN), (34, VK_RETURN), (35, VK_SPACE), (36, VK_ESCAPE)] {
                if RegisterHotKey(Some(self.main), id, MOD_NOREPEAT, vk.0 as u32).is_ok() {
                    self.hotkeys.push(id);
                }
            }
        }
        self.render(di);
    }

    fn end_focus(&mut self) {
        unsafe {
            for id in 30..=36 {
                let _ = UnregisterHotKey(Some(self.main), id);
            }
        }
        self.hotkeys.retain(|x| !(30..=36).contains(x));
        for i in 0..self.docks.len() {
            if self.docks[i].kb_focus.take().is_some() {
                self.render(i);
            }
        }
        if self.cfg.general.autohide {
            unsafe {
                SetTimer(Some(self.main), T_HIDE, 600, None);
            }
        }
    }

    fn focus_key(&mut self, id: i32) {
        let Some(di) = self.docks.iter().position(|d| d.kb_focus.is_some()) else {
            self.end_focus();
            return;
        };
        let n = self.docks[di].slots.len();
        let cur = self.docks[di].kb_focus.unwrap_or(0);
        let step = |from: usize, dir: i32, slots: &[dock::Slot]| -> usize {
            let mut k = from as i32;
            for _ in 0..n {
                k = (k + dir).rem_euclid(n as i32);
                if !matches!(slots[k as usize].kind, SlotKind::Sep | SlotKind::Spacer) {
                    return k as usize;
                }
            }
            from
        };
        match id {
            30 | 32 => self.docks[di].kb_focus = Some(step(cur, -1, &self.docks[di].slots)),
            31 | 33 => self.docks[di].kb_focus = Some(step(cur, 1, &self.docks[di].slots)),
            34 | 35 => {
                self.end_focus();
                self.click_slot(di, cur, 0.5, 0.5);
                return;
            }
            _ => {
                self.end_focus();
                return;
            }
        }
        self.render(di);
    }

    // -----------------------------------------------------------------------
    // Avisos
    // -----------------------------------------------------------------------

    pub fn show_toast(&mut self, title: &str, body: &str, sound: bool) {
        let d = self.docks.first();
        let work = d.map(|d| d.work).unwrap_or_default();
        let dpi = d.map(|d| d.dpi).unwrap_or(96);
        self.toast_serial += 1;
        let mut t = ToastWin::new(self.toast_serial, title, body, work, dpi, self.toasts.len(), sound);
        t.render();
        t.p.show();
        self.toasts.push(t);
        unsafe {
            SetTimer(Some(self.main), T_TOAST, 1000, None);
        }
    }

    fn expire_toasts(&mut self) {
        let now = tick_ms();
        let mut changed = false;
        self.toasts.retain_mut(|t| {
            if now >= t.until {
                t.p.destroy();
                changed = true;
                false
            } else {
                true
            }
        });
        if self.toasts.is_empty() {
            unsafe {
                let _ = KillTimer(Some(self.main), T_TOAST);
            }
        }
        let _ = changed;
    }

    fn dismiss_toast(&mut self, h: HWND) {
        self.toasts.retain_mut(|t| {
            if t.p.hwnd == h {
                t.p.destroy();
                false
            } else {
                true
            }
        });
    }

    // -----------------------------------------------------------------------
    // Conta-gotas
    // -----------------------------------------------------------------------

    fn eyedrop_msg(&mut self, msg: u32, wp: WPARAM, _lp: LPARAM) -> Option<LRESULT> {
        let e = self.eyedrop.as_mut()?;
        match msg {
            WM_MOUSEMOVE => {
                let p = glass::cursor_pos();
                e.update(p.x, p.y);
                Some(LRESULT(0))
            }
            WM_LBUTTONUP => {
                let p = glass::cursor_pos();
                let (r, g, b) = e.pixel(p.x, p.y);
                let wid = e.widget;
                let hex = format!("#{:02X}{:02X}{:02X}", r, g, b);
                let mut e = self.eyedrop.take().unwrap();
                e.destroy();
                if let Some(w) = self.widgets.get_mut(&wid) {
                    self.sh.current_widget = wid;
                    w.on_picked(100, vec![hex.clone()], &mut self.sh);
                }
                self.process_requests();
                self.render_all();
                Some(LRESULT(0))
            }
            WM_RBUTTONUP => {
                let mut e = self.eyedrop.take().unwrap();
                e.destroy();
                Some(LRESULT(0))
            }
            WM_KEYDOWN => {
                if wp.0 as u16 == VK_ESCAPE.0 {
                    let mut e = self.eyedrop.take().unwrap();
                    e.destroy();
                }
                Some(LRESULT(0))
            }
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Salvar e sair
    // -----------------------------------------------------------------------

    pub fn mark_save(&mut self) {
        unsafe {
            SetTimer(Some(self.main), T_SAVE, 1500, None);
        }
    }

    pub fn do_save(&mut self) {
        for wi in self.cfg.widgets.iter_mut() {
            if let Some(w) = self.widgets.get(&wi.id) {
                wi.data = w.save();
            }
        }
        self.cfg.save();
        if self.wins.dirty_save {
            self.wins.save();
        }
        self.sync_ws_list();
    }

    pub fn quit(&mut self) {
        self.quitting = true;
        unsafe {
            PostQuitMessage(0);
        }
    }

    fn shutdown(&mut self) {
        self.do_save();
        self.close_panel();
        self.close_preview();
        if let Some(mut s) = self.settings.take() {
            s.p.destroy();
        }
        for t in self.toasts.iter_mut() {
            t.p.destroy();
        }
        unsafe {
            for h in self.hooks.drain(..) {
                let _ = UnhookWinEvent(h);
            }
            let _ = DeregisterShellHookWindow(self.main);
            for id in self.hotkeys.drain(..) {
                let _ = UnregisterHotKey(Some(self.main), id);
            }
        }
        for d in &self.docks {
            self.remove_appbar_of(d.plate);
        }
        for d in self.docks.iter_mut() {
            d.destroy();
        }
        tray::stop();
        // as janelas guardadas nos outros ambientes voltam
        for w in &self.wins.wins {
            if w.min_by_us {
                winlist::restore_quiet(w.hwnd);
            }
        }
        self.taskbar.restore();
        self.sh.status.shutdown();
    }
}

// ---------------------------------------------------------------------------
// Menus nativos
// ---------------------------------------------------------------------------

unsafe fn build_menu(items: &[MItem], acts: &mut Vec<Act>) -> HMENU {
    let hm = CreatePopupMenu().unwrap_or_default();
    for it in items {
        match it {
            MItem::Sep => {
                let _ = AppendMenuW(hm, MF_SEPARATOR, 0, None);
            }
            MItem::Disabled(t) => {
                let w = wide(t);
                let _ = AppendMenuW(hm, MF_STRING | MF_GRAYED, 0, pcw(&w));
            }
            MItem::Item(t, a, checked) => {
                acts.push(a.clone());
                let w = wide(t);
                let flags = MF_STRING | if *checked { MF_CHECKED } else { MF_UNCHECKED };
                let _ = AppendMenuW(hm, flags, acts.len(), pcw(&w));
            }
            MItem::Sub(t, sub) => {
                let sm = build_menu(sub, acts);
                let w = wide(t);
                let _ = AppendMenuW(hm, MF_POPUP | MF_STRING, sm.0 as usize, pcw(&w));
            }
        }
    }
    hm
}

// ---------------------------------------------------------------------------
// Soltar arquivos na dock
// ---------------------------------------------------------------------------

pub fn drop_hover(dock: usize, sx: i32, sy: i32) -> windows::Win32::System::Ole::DROPEFFECT {
    use windows::Win32::System::Ole::*;
    with_app(|a| {
        let Some(d) = a.docks.get(dock) else { return DROPEFFECT_NONE };
        let g = d.geo(&a.cfg);
        let (x, y) = ((sx - d.win.left) as f32, (sy - d.win.top) as f32);
        let slot = dock::slot_at(d, x, y, &g);
        let eff = match slot.map(|s| d.slots[s].kind.clone()) {
            Some(SlotKind::Trash) => DROPEFFECT_MOVE,
            Some(SlotKind::Folder) | Some(SlotKind::Downloads) => DROPEFFECT_COPY,
            Some(SlotKind::Widget) => DROPEFFECT_COPY,
            Some(SlotKind::Pinned) | Some(SlotKind::Running) => DROPEFFECT_COPY,
            _ => DROPEFFECT_LINK,
        };
        if a.docks[dock].drop_hover != slot {
            a.docks[dock].drop_hover = slot;
            a.docks[dock].mouse = Some((x, y));
            a.docks[dock].mag_target = 1.0;
            a.render(dock);
            a.kick_anim();
        }
        eff
    })
    .unwrap_or(DROPEFFECT_NONE)
}

pub fn drop_leave(dock: usize) {
    with_app(|a| {
        if let Some(d) = a.docks.get_mut(dock) {
            d.drop_hover = None;
            d.mouse = None;
            d.mag_target = 0.0;
        }
        a.render(dock);
        a.kick_anim();
    });
}

pub fn drop_files(dock: usize, sx: i32, sy: i32, files: Vec<String>) -> windows::Win32::System::Ole::DROPEFFECT {
    use windows::Win32::System::Ole::*;
    let r = with_app(|a| {
        let Some(d) = a.docks.get(dock) else { return DROPEFFECT_NONE };
        let g = d.geo(&a.cfg);
        let (x, y) = ((sx - d.win.left) as f32, (sy - d.win.top) as f32);
        let slot = dock::slot_at(d, x, y, &g);
        let sl = slot.map(|s| d.slots[s].clone());
        a.docks[dock].drop_hover = None;
        a.docks[dock].mouse = None;
        a.docks[dock].mag_target = 0.0;
        if files.is_empty() {
            return DROPEFFECT_NONE;
        }
        let wsi = a.cfg.ws_index(a.docks[dock].ws).unwrap_or(0);
        match sl {
            Some(sl) if sl.kind == SlotKind::Trash => {
                let f = files.clone();
                defer(move || crate::dragdrop::recycle(&f));
                DROPEFFECT_MOVE
            }
            Some(sl) if matches!(sl.kind, SlotKind::Folder | SlotKind::Downloads) => {
                let f = files.clone();
                let dir = sl.icon.clone();
                defer(move || crate::dragdrop::copy_into(&f, &dir, false));
                DROPEFFECT_COPY
            }
            Some(sl) if sl.kind == SlotKind::Widget => {
                if let Some(w) = a.widgets.get_mut(&sl.widget) {
                    a.sh.current_widget = sl.widget;
                    w.on_drop(&files, &mut a.sh);
                }
                a.process_requests();
                DROPEFFECT_COPY
            }
            Some(sl) if sl.kind == SlotKind::Pinned => {
                // abre os arquivos com o app
                if let Some(app) = a.slot_app(dock, slot.unwrap()) {
                    let args = files.iter().map(|f| format!("\"{}\"", f)).collect::<Vec<_>>().join(" ");
                    let target = if app.exe.is_empty() { app.launch.clone() } else { app.exe.clone() };
                    shell_exec(&target, Some(&args), None);
                }
                DROPEFFECT_COPY
            }
            _ => {
                // fixa apps, pastas e arquivos
                let items = &mut a.cfg.workspaces[wsi].items;
                let pos = items.iter().rposition(|x| matches!(x, DockItem::App(_) | DockItem::Start | DockItem::Folder { .. })).map(|p| p + 1).unwrap_or(items.len());
                let mut k = 0;
                for f in &files {
                    let p = std::path::Path::new(f);
                    let item = if p.is_dir() {
                        DockItem::Folder { path: f.clone(), list: false, sort: 2 }
                    } else {
                        DockItem::App(icons::pinned_from_path(f))
                    };
                    items.insert(pos + k, item);
                    k += 1;
                }
                a.mark_save();
                DROPEFFECT_LINK
            }
        }
    });
    with_app(|a| {
        a.kick_anim();
        a.rebuild_all();
    });
    run_deferred();
    r.unwrap_or(windows::Win32::System::Ole::DROPEFFECT_NONE)
}

// mantém o compilador feliz com tipos usados só em algumas plataformas
#[allow(dead_code)]
fn _unused(_: PCWSTR) {}
