// Configuração persistida em %APPDATA%\GlassDock\config.json
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::util::*;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct PinnedApp {
    pub name: String,
    /// O que executar: caminho de .exe/.lnk ou "shell:AppsFolder\<AUMID>"
    pub launch: String,
    pub args: String,
    /// Caminho do executável (para casar com janelas abertas)
    pub exe: String,
    /// AppUserModelID (apps da Store e apps com ID explícito)
    pub aumid: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "t")]
pub enum DockItem {
    Start,
    App(PinnedApp),
    /// `zone`: 0 = junto dos apps fixados, 1 = depois dos apps abertos,
    /// 2 = depois da bandeja e do relógio
    Widgets { ids: Vec<u32>, #[serde(default)] cur: usize, #[serde(default)] zone: u8 },
    Spacer { large: bool },
    Folder { path: String, #[serde(default)] list: bool, #[serde(default)] sort: u32 },
    /// separador visível (barrinha)
    Divider,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Workspace {
    pub id: u32,
    pub name: String,
    pub color: u32,
    pub items: Vec<DockItem>,
    /// ordem livre do grupo de apps (chaves de fixados, abertos e espaçadores)
    pub app_order: Vec<String>,
}

impl Default for Workspace {
    fn default() -> Self {
        Workspace { id: 1, name: "Trabalho".into(), color: 0, items: vec![], app_order: vec![] }
    }
}

pub const WS_COLORS: [(u32, &str); 6] = [
    (0x0A84FF, "Azul"),
    (0xFF375F, "Rosa"),
    (0xFF9F0A, "Laranja"),
    (0x30D158, "Verde"),
    (0xBF5AF2, "Roxo"),
    (0x8E8E93, "Grafite"),
];

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct WidgetInstance {
    pub id: u32,
    pub kind: String,
    pub name: String,
    pub style: u32,
    pub units: u32,
    pub data: Value,
}

impl Default for WidgetInstance {
    fn default() -> Self {
        WidgetInstance { id: 0, kind: String::new(), name: String::new(), style: 0, units: 1, data: Value::Null }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskbarMode {
    /// Esconde a barra de tarefas por completo enquanto o app roda
    Hide,
    /// Coloca a barra no ocultar automaticamente
    AutoHide,
    /// Mantém a barra visível (a dock fica acima dela)
    Keep,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockPos {
    Bottom,
    Top,
    Left,
    Right,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hover {
    None,
    Scale,
    Magnify,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchAnim {
    Bounce,
    Pulse,
    Shake,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Indicator {
    Dot,
    Dash,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    Dark,
    Light,
    Auto,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct General {
    pub taskbar: TaskbarMode,
    pub autohide: bool,
    pub reserve_space: bool,
    pub position: DockPos,
    pub all_monitors: bool,
    pub apps_from_all_monitors: bool,
    pub only_current_ws: bool,
    pub show_running: bool,
    pub show_trash: bool,
    pub show_downloads: bool,
    pub ws_selector: bool,
    pub start_with_windows: bool,
    pub previews: bool,
    pub preview_delay_ms: u32,
    pub previews_only_multi: bool,
    pub show_clock: bool,
    pub show_date: bool,
    pub clock_align: u32,
    pub show_status: bool,
    pub wheel_switches_ws: bool,
    pub ws_hotkeys: bool,
    pub focus_hotkey: bool,
    pub follow_pointer: bool,
    pub hide_in_fullscreen: bool,
    /// apps fixados e abertos juntos, com ordem livre
    pub mixed_apps: bool,
    /// ícones de status e bandeja: 0 em linha, 1 em grade (2 linhas), 2 compacto
    pub status_layout: u32,
}

impl Default for General {
    fn default() -> Self {
        General {
            taskbar: TaskbarMode::Hide,
            autohide: false,
            reserve_space: true,
            position: DockPos::Bottom,
            all_monitors: false,
            apps_from_all_monitors: false,
            only_current_ws: true,
            show_running: true,
            show_trash: true,
            show_downloads: true,
            ws_selector: true,
            start_with_windows: false,
            previews: true,
            preview_delay_ms: 450,
            previews_only_multi: false,
            show_clock: true,
            show_date: true,
            clock_align: 2,
            show_status: true,
            wheel_switches_ws: true,
            ws_hotkeys: true,
            focus_hotkey: true,
            follow_pointer: true,
            hide_in_fullscreen: true,
            mixed_apps: true,
            status_layout: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Appearance {
    pub icon_size: f32,
    pub hover: Hover,
    pub magnify: f32,
    pub launch_anim: LaunchAnim,
    pub indicator: Indicator,
    pub indicator_ws_color: bool,
    pub plates: bool,
    pub tint: Tint,
    pub opacity: f32,
    pub labels: bool,
    /// 0 composição, 1 acrílico, 2 desfoque clássico, 3 transparente
    pub glass_mode: u32,
    /// fundo (placa) em todos os ícones, não só nos que não são quadrados
    pub plate_all: bool,
    /// cor da placa (#RRGGBB)
    pub plate_color: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance {
            icon_size: 46.0,
            hover: Hover::Magnify,
            magnify: 1.45,
            launch_anim: LaunchAnim::Bounce,
            indicator: Indicator::Dot,
            indicator_ws_color: false,
            plates: true,
            tint: Tint::Dark,
            opacity: 0.52,
            labels: true,
            glass_mode: 1,
            plate_all: false,
            plate_color: "#FFFFFF".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub workspaces: Vec<Workspace>,
    pub widgets: Vec<WidgetInstance>,
    pub general: General,
    pub appearance: Appearance,
    /// Ícones da bandeja fixados na dock (chave = exe|uid ou guid)
    pub tray_pinned: Vec<String>,
    pub tray_initialized: bool,
    pub next_id: u32,
    /// Comando de status line do Claude Code que existia antes (encadeado)
    pub claude_prev_statusline: String,
    /// Apps que nunca aparecem como abertos (exe ou AppUserModelID, minúsculo)
    pub hidden_apps: Vec<String>,
    /// Agendas ICS do app inteiro
    pub calendars: Vec<String>,
    pub hidden_defaults: bool,
}

impl Config {
    pub fn path() -> std::path::PathBuf {
        data_dir().join("config.json")
    }

    pub fn load() -> Config {
        let p = Self::path();
        let mut c: Config = read_json(&p).unwrap_or_default();
        if c.workspaces.is_empty() {
            c = Config::first_run();
        }
        if !c.hidden_defaults {
            // apps que rodam em segundo plano mas mantêm uma janela "visível"
            c.hidden_defaults = true;
            for a in ["raycast.exe"] {
                if !c.hidden_apps.iter().any(|x| x == a) {
                    c.hidden_apps.push(a.into());
                }
            }
        }
        c
    }

    pub fn is_hidden_app(&self, exe: &str, aumid: &str) -> bool {
        if self.hidden_apps.is_empty() {
            return false;
        }
        let name = std::path::Path::new(exe).file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        let a = aumid.to_lowercase();
        self.hidden_apps.iter().any(|h| (!name.is_empty() && *h == name) || (!a.is_empty() && *h == a))
    }

    pub fn save(&self) {
        write_json_atomic(&Self::path(), self);
    }

    pub fn alloc_id(&mut self) -> u32 {
        self.next_id = self.next_id.max(100) + 1;
        self.next_id
    }

    fn first_run() -> Config {
        let mut c = Config { version: 1, next_id: 100, ..Default::default() };
        let apps = crate::icons::default_pins();
        let mut w_ids = vec![];
        for (kind, units) in [("music", 3u32), ("calendar", 1), ("weather", 2), ("activity", 1)] {
            let id = c.alloc_id();
            let mut wi = WidgetInstance { id, kind: kind.into(), units, ..Default::default() };
            wi.name = crate::widgets::catalog_name(kind).to_string();
            if kind == "calendar" {
                wi.style = 3;
            }
            if kind == "weather" {
                wi.style = 1;
            }
            c.widgets.push(wi);
            w_ids.push(id);
        }
        let mk = |id: u32, name: &str, color: u32, with_widgets: bool| {
            let mut items = vec![DockItem::Start];
            items.extend(apps.iter().cloned().map(DockItem::App));
            if with_widgets {
                for id in &w_ids {
                    items.push(DockItem::Widgets { ids: vec![*id], cur: 0, zone: 0 });
                }
            }
            Workspace { id, name: name.into(), color, items, app_order: vec![] }
        };
        c.workspaces = vec![mk(1, "Trabalho", 0, true), mk(2, "Pessoal", 3, true), mk(3, "Foco", 4, false)];
        c
    }

    pub fn widget(&self, id: u32) -> Option<&WidgetInstance> {
        self.widgets.iter().find(|w| w.id == id)
    }

    pub fn ws_index(&self, id: u32) -> Option<usize> {
        self.workspaces.iter().position(|w| w.id == id)
    }
}
