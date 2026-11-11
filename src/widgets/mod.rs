// Widgets: tiles vivos dentro da dock que abrem um painel no clique.
use serde_json::Value;
use std::sync::mpsc::Sender;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;

use crate::gfx::*;
use crate::icons::{CatalogApp, IconCache};
use crate::media::MediaInfo;
use crate::status::Status;
use crate::sysmon::SysMon;
use crate::ui::Ui;
use crate::util::*;

pub mod ai;
pub mod apps;
pub mod calendar;
pub mod clock;
pub mod eyedrop;
pub mod files;
pub mod music;
pub mod notes;
pub mod pet;
pub mod system;
pub mod timers;
pub mod weather;

pub struct Tile<'a> {
    pub c: &'a Canvas,
    pub r: Rect,
    /// tamanho de um ícone (px) no instante do desenho
    pub s: f32,
    pub units: u32,
    pub style: u32,
    pub hover: bool,
    pub name: &'a str,
}

impl<'a> Tile<'a> {
    pub fn wide(&self) -> bool {
        self.units >= 2
    }
    pub fn fs(&self, k: f32) -> f32 {
        self.s * k
    }
    /// Cartão padrão do widget (vidro escuro).
    pub fn card(&self) {
        let rad = self.s * 0.24;
        self.c.fill_rrect(self.r, rad, Color::rgba(22, 22, 26, 0.84));
        self.c.fill_rrect_vgrad(self.r, rad, &[(0.0, Color(1.0, 1.0, 1.0, 0.09)), (0.5, Color(1.0, 1.0, 1.0, 0.0))]);
        self.c.stroke_rrect(self.r.inset(0.5), rad, Color(1.0, 1.0, 1.0, 0.11), 1.0);
    }
    pub fn card_color(&self, top: Color, bottom: Color) {
        let rad = self.s * 0.24;
        self.c.fill_rrect_vgrad(self.r, rad, &[(0.0, top), (1.0, bottom)]);
        self.c.fill_rrect_vgrad(self.r, rad, &[(0.0, Color(1.0, 1.0, 1.0, 0.14)), (0.45, Color(1.0, 1.0, 1.0, 0.0))]);
        self.c.stroke_rrect(self.r.inset(0.5), rad, Color(1.0, 1.0, 1.0, 0.16), 1.0);
    }
    pub fn rad(&self) -> f32 {
        self.s * 0.24
    }
    /// Área útil interna (com margem)
    pub fn inner(&self) -> Rect {
        self.r.inset_xy(self.s * 0.14, self.s * 0.12)
    }
    /// Texto que encolhe até caber na largura.
    pub fn fit_text(&self, t: &str, r: Rect, max_size: f32, weight: u32, col: Color, align: Align) {
        let mut size = max_size;
        let (w, _) = self.c.g.measure(t, size, weight);
        if w > r.w && w > 0.0 {
            size = (size * r.w / w).max(max_size * 0.62);
        }
        self.c.text(t, r, size, weight, col, align);
    }
    pub fn fit_display(&self, t: &str, r: Rect, max_size: f32, weight: u32, col: Color, align: Align) {
        let mut size = max_size;
        let (w, _) = self.c.g.measure_fam(t, size, weight, 2);
        if w > r.w && w > 0.0 {
            size = (size * r.w / w).max(max_size * 0.4);
        }
        self.c.display(t, r, size, weight, col, align);
    }
}

#[derive(PartialEq)]
pub enum Click {
    Panel,
    Handled,
}

pub enum Pick {
    Folder,
    Files,
    Exe,
}

#[derive(Clone)]
pub struct Toast {
    pub title: String,
    pub body: String,
    pub sound: bool,
}

pub struct Shared {
    pub sys: SysMon,
    pub status: Status,
    pub media: MediaInfo,
    pub media_tx: Option<Sender<crate::media::Cmd>>,
    pub media_art: Option<ID2D1Bitmap>,
    pub media_art_serial: u64,
    /// último momento (unix, s) em que havia música tocando (ou pausada, ao abrir)
    pub media_last_playing: i64,
    pub icons: IconCache,
    pub toasts: Vec<Toast>,
    pub ws_list: Vec<(u32, String, u32)>,
    pub now: DateTime,
    pub save: bool,
    pub redraw: bool,
    pub close_panel: bool,
    pub pick: Option<(u32, u32, Pick)>,
    pub eyedrop: Option<u32>,
    pub focus_app: Option<String>,
    pub catalog: Option<Vec<CatalogApp>>,
    pub panel_scale: f32,
    pub open_settings_for: Option<u32>,
    pub main_hwnd: windows::Win32::Foundation::HWND,
    pub current_widget: u32,
    /// menu nativo pedido por um widget (itens; o escolhido vai para on_menu)
    pub popup: Option<Vec<(u32, String)>>,
    /// arquivos para arrastar para fora
    pub drag_out: Option<Vec<String>>,
    /// pastas vigiadas / atalhos mudaram
    pub rewatch: bool,
    /// agendas ICS compartilhadas por todos os calendários
    pub ics: calendar::IcsStore,
}

impl Shared {
    pub fn toast(&mut self, title: &str, body: &str) {
        self.toasts.push(Toast { title: title.into(), body: body.into(), sound: true });
    }
    pub fn media_cmd(&self, c: crate::media::Cmd) {
        if let Some(tx) = &self.media_tx {
            let _ = tx.send(c);
        }
    }
    pub fn catalog(&mut self) -> &Vec<CatalogApp> {
        if self.catalog.is_none() {
            self.catalog = Some(crate::icons::catalog());
        }
        self.catalog.as_ref().unwrap()
    }
}

pub trait Widget {
    fn draw(&mut self, t: &Tile, sh: &mut Shared);
    /// O widget some da dock agora? (ex.: player sem música tocando)
    fn hidden(&self, _sh: &Shared) -> bool {
        false
    }
    fn save(&self) -> Value;
    /// Intervalo (ms) para redesenhar o tile; None = só por evento.
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        None
    }
    fn needs_sys(&self) -> bool {
        false
    }
    fn needs_media(&self) -> bool {
        false
    }
    fn on_click(&mut self, _fx: f32, _fy: f32, _units: u32, _sh: &mut Shared) -> Click {
        Click::Panel
    }
    /// O widget usa clique duplo (o clique simples espera um instante).
    fn double_click(&self) -> bool {
        false
    }
    fn on_double_click(&mut self, _sh: &mut Shared) {}
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (300.0, 200.0)
    }
    fn panel(&mut self, _ui: &mut Ui, _sh: &mut Shared) {}
    fn panel_interval(&self, _sh: &Shared) -> Option<u32> {
        None
    }
    /// Opções próprias no editor do widget.
    fn settings(&mut self, _ui: &mut Ui, _sh: &mut Shared) {}
    fn menu(&self) -> Vec<(u32, String)> {
        vec![]
    }
    fn on_menu(&mut self, _id: u32, _sh: &mut Shared) {}
    /// Dados de fundo chegaram.
    fn poll(&mut self, _sh: &mut Shared) -> bool {
        false
    }
    /// Chamado a cada intervalo; devolve true se o tile precisa ser redesenhado.
    fn tick(&mut self, _sh: &mut Shared) -> bool {
        true
    }
    fn styles(&self) -> &'static [&'static str] {
        &[]
    }
    fn max_units(&self) -> u32 {
        4
    }
    fn on_drop(&mut self, _files: &[String], _sh: &mut Shared) -> bool {
        false
    }
    fn on_picked(&mut self, _purpose: u32, _paths: Vec<String>, _sh: &mut Shared) {}
    /// Diretórios a vigiar (mudanças chamam `poll`).
    fn watch_dirs(&self) -> Vec<String> {
        vec![]
    }
    fn hotkey(&self) -> Option<(u32, u32)> {
        None
    }
    fn on_hotkey(&mut self, _sh: &mut Shared) {}
    /// Pets desenham numa camada própria por cima da dock.
    fn is_pet(&self) -> bool {
        false
    }
    fn as_pet(&mut self) -> Option<&mut pet::Pet> {
        None
    }
}

pub struct CatalogEntry {
    pub kind: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub group: &'static str,
    pub units: u32,
    pub style: u32,
    pub data: &'static str,
}

macro_rules! entry {
    ($k:expr, $n:expr, $d:expr, $g:expr, $u:expr) => {
        CatalogEntry { kind: $k, name: $n, desc: $d, group: $g, units: $u, style: 0, data: "" }
    };
    ($k:expr, $n:expr, $d:expr, $g:expr, $u:expr, $s:expr, $data:expr) => {
        CatalogEntry { kind: $k, name: $n, desc: $d, group: $g, units: $u, style: $s, data: $data }
    };
}

pub const CATALOG: &[CatalogEntry] = &[
    entry!("collection", "Coleção de apps", "Um grupo de apps num tile só. Abre em grade ou lista.", "Apps e arquivos", 1),
    entry!("favorites", "Favoritos", "Os sites que você abre todo dia, um clique cada.", "Apps e arquivos", 2),
    entry!("files", "Arquivos", "A pasta Downloads, ou a que você escolher: o que chegou por último.", "Apps e arquivos", 1),
    entry!("clock", "Relógio", "A sua hora, em números.", "Relógios", 2),
    entry!("clock", "Analógico", "O mesmo relógio, com ponteiros.", "Relógios", 1, 1, r#"{"analog":true}"#),
    entry!("clock", "Relógio mundial", "A hora em outra cidade.", "Relógios", 2, 0, r#"{"tz":"London"}"#),
    entry!("timer", "Timer de foco", "Contagem regressiva com o anel de progresso no ícone.", "Relógios", 1),
    entry!("pomodoro", "Pomodoro", "Foco, pausa curta e pausa longa, com os ciclos do dia.", "Relógios", 1),
    entry!("countdown", "Contagem regressiva", "Quantos dias faltam para a data que importa.", "Relógios", 1),
    entry!("progress", "Progresso do tempo", "Quanto do dia, da semana, do mês ou do ano já foi.", "Relógios", 1),
    entry!("calendar", "Próximo evento", "O próximo compromisso e quanto falta para ele.", "Calendário", 3, 0, ""),
    entry!("calendar", "Data + próximo evento", "A data de hoje e o próximo compromisso ao lado.", "Calendário", 3, 1, ""),
    entry!("calendar", "Data + agenda", "A data e a lista dos próximos compromissos.", "Calendário", 3, 2, ""),
    entry!("calendar", "Data", "Só a data de hoje. O clique mostra o mês e a agenda.", "Calendário", 1, 3, ""),
    entry!("calendar", "Data + hora", "O calendário com a hora grande ao lado: relógio e data num tile só.", "Calendário", 2, 4, ""),
    entry!("calendar", "Hora + data + próximo", "A hora à esquerda, a data e o próximo compromisso com o horário em destaque.", "Calendário", 4, 5, ""),
    entry!("tasks", "Lista de tarefas", "Anote, marque e limpe tarefas direto no painel.", "Notas e mídia", 2),
    entry!("note", "Post-it", "Um recado que fica na dock.", "Notas e mídia", 2),
    entry!("music", "Tocando agora", "O que está tocando, com capa e controles. Spotify, navegador ou qualquer player.", "Notas e mídia", 3),
    entry!("battery", "Bateria · Estado", "A carga e quanto tempo ela ainda dá.", "Sistema", 1, 0, ""),
    entry!("battery", "Bateria · Anel", "A carga como um anel.", "Sistema", 1, 1, ""),
    entry!("battery", "Bateria · Só o ícone", "O ícone de bateria de sempre.", "Sistema", 1, 2, ""),
    entry!("activity", "Atividade · Barras", "Processador e memória em barras, ao vivo.", "Sistema", 1, 0, ""),
    entry!("activity", "Atividade · Números", "Processador e memória em porcentagem.", "Sistema", 2, 1, ""),
    entry!("activity", "Atividade · Anéis", "Processador e memória em anéis.", "Sistema", 1, 2, ""),
    entry!("network", "Rede · Números", "O que está subindo e descendo agora.", "Sistema", 2, 0, ""),
    entry!("network", "Rede · Com gráfico", "Download e upload com o gráfico dos últimos instantes.", "Sistema", 2, 1, ""),
    entry!("keyboard", "Layout do teclado", "Mostra o layout atual e troca com um clique.", "Sistema", 1),
    entry!("eyedropper", "Conta-gotas", "Pegue qualquer cor da tela. O código vai para a área de transferência.", "Ferramentas", 1),
    entry!("screenshots", "Capturas de tela", "As últimas capturas, para arrastar direto aonde precisar.", "Ferramentas", 1),
    entry!("water", "Beber água", "Um copo por vez, a meta do dia e a semana no cartão.", "Ferramentas", 1),
    entry!("pet", "Mascote", "Um bichinho que senta, anda, corre ou dorme conforme o PC trabalha.", "Mascote", 1),
    entry!("claude", "Claude · Limite de 5 horas", "Quanto do limite de 5 horas do seu plano já foi.", "IA", 1, 0, ""),
    entry!("claude", "Claude · Limite semanal", "Quanto do limite semanal já foi.", "IA", 1, 1, ""),
    entry!("claude", "Claude · Tokens", "Tokens gastos no Claude Code.", "IA", 2, 2, ""),
    entry!("codex", "Uso do Codex", "Limites do plano e tokens do Codex.", "IA", 2),
    entry!("opencode", "Uso do OpenCode", "Custo e tokens por provedor no OpenCode.", "IA", 2),
    entry!("weather", "Clima · Agora", "Temperatura e condição agora, na cidade que você escolher.", "Clima", 1, 0, ""),
    entry!("weather", "Clima · Condições", "O tempo agora, com mais detalhes.", "Clima", 2, 1, ""),
    entry!("weather", "Clima · Previsão", "Os próximos 5 dias, lado a lado.", "Clima", 4, 2, ""),
];

pub fn catalog_name(kind: &str) -> &'static str {
    match kind {
        "music" => "Tocando agora",
        "calendar" => "Calendário",
        "weather" => "Clima",
        "activity" => "Atividade",
        _ => CATALOG.iter().find(|e| e.kind == kind).map(|e| e.name).unwrap_or("Widget"),
    }
}

pub fn make(kind: &str, data: &Value) -> Box<dyn Widget> {
    match kind {
        "clock" => Box::new(clock::Clock::load(data)),
        "timer" => Box::new(timers::Timer::load(data)),
        "pomodoro" => Box::new(timers::Pomodoro::load(data)),
        "countdown" => Box::new(timers::Countdown::load(data)),
        "progress" => Box::new(timers::Progress::load(data)),
        "calendar" => Box::new(calendar::Calendar::load(data)),
        "tasks" => Box::new(notes::Tasks::load(data)),
        "note" => Box::new(notes::Note::load(data)),
        "water" => Box::new(notes::Water::load(data)),
        "music" => Box::new(music::Music::load(data)),
        "battery" => Box::new(system::BatteryW::load(data)),
        "activity" => Box::new(system::Activity::load(data)),
        "network" => Box::new(system::Network::load(data)),
        "keyboard" => Box::new(system::Keyboard::load(data)),
        "files" => Box::new(files::Files::load(data, false)),
        "screenshots" => Box::new(files::Files::load(data, true)),
        "collection" => Box::new(apps::Collection::load(data)),
        "favorites" => Box::new(apps::Favorites::load(data)),
        "eyedropper" => Box::new(eyedrop::Eyedropper::load(data)),
        "claude" => Box::new(ai::Claude::load(data)),
        "codex" => Box::new(ai::Codex::load(data)),
        "opencode" => Box::new(ai::OpenCode::load(data)),
        "weather" => Box::new(weather::Weather::load(data)),
        "pet" => Box::new(pet::Pet::load(data)),
        _ => Box::new(clock::Clock::load(data)),
    }
}

/// Lê um campo do JSON com padrão.
pub fn jget<T: serde::de::DeserializeOwned + Default>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_default()
}

pub fn jput<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// Barra de progresso horizontal arredondada.
pub fn bar(c: &Canvas, r: Rect, frac: f32, track: Color, col: Color) {
    c.fill_rrect(r, r.h / 2.0, track);
    if frac > 0.0 {
        let w = (r.w * frac.clamp(0.0, 1.0)).max(r.h);
        c.fill_rrect(Rect::new(r.x, r.y, w, r.h), r.h / 2.0, col);
    }
}

/// Cor de alerta por porcentagem de uso (verde → laranja → vermelho).
pub fn usage_color(p: f32) -> Color {
    if p >= 0.9 {
        RED
    } else if p >= 0.7 {
        ORANGE
    } else {
        GREEN
    }
}

/// Mostra o painel padrão de "precisa configurar".
pub fn panel_header(ui: &mut Ui, title: &str, subtitle: &str) {
    let r = ui.row(26.0);
    ui.c.text(title, r, 15.0 * ui.s, W_SEMIBOLD, crate::ui::TXT, Align::Left);
    if !subtitle.is_empty() {
        let r = ui.row(18.0);
        ui.c.text(subtitle, r, 11.5 * ui.s, W_NORMAL, crate::ui::TXT2, Align::Left);
    }
    ui.gap(8.0);
}
