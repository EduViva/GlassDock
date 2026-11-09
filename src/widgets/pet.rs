// Mascote: um bichinho que dorme, senta, anda ou corre conforme a carga do
// processador (ou da memória). Ele anda por cima da dock numa camada
// própria e pequena, então só essa camada é redesenhada enquanto ele se mexe.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::*;
use crate::ui::*;

pub const SPECIES: [&str; 9] = ["Gato", "Cachorro", "Passarinho", "Dragão", "Dino", "Guerreira", "Guerreiro", "Papai Noel", "Fantasminha"];

const FURS: [[(u32, u32); 4]; 9] = [
    [(0xF2A65A, 0xFFF1E0), (0x3B3B3F, 0xEDEDED), (0xB0B0B8, 0xFFFFFF), (0xFFFFFF, 0xF4C7C3)],
    [(0xC68B59, 0xF6E3CF), (0x2E2E33, 0xD9A066), (0xF1E3C8, 0xFFFFFF), (0x8B5A2B, 0xE8C9A0)],
    [(0x4FC3F7, 0xFFF59D), (0xFFD54F, 0xFF8A65), (0xE57373, 0xFFF3E0), (0x81C784, 0xFFFDE7)],
    [(0x66BB6A, 0xFFF59D), (0xE53935, 0xFFCC80), (0x7E57C2, 0xB3E5FC), (0x263238, 0xFF7043)],
    [(0x8BC34A, 0xF0F4C3), (0xFF8A65, 0xFFE0B2), (0x4DB6AC, 0xE0F2F1), (0x9575CD, 0xEDE7F6)],
    [(0xC62828, 0xFFCCBC), (0x1565C0, 0xFFE0B2), (0x6A1B9A, 0xD7CCC8), (0x2E7D32, 0xFFE0B2)],
    [(0x37474F, 0xFFCCBC), (0x8D6E63, 0xFFE0B2), (0x1565C0, 0xD7CCC8), (0xB71C1C, 0xFFE0B2)],
    [(0xD32F2F, 0xFFE0B2), (0x2E7D32, 0xFFE0B2), (0x1565C0, 0xFFE0B2), (0x6D4C41, 0xFFE0B2)],
    [(0xF5F5F5, 0x37474F), (0xB3E5FC, 0x37474F), (0xE1BEE7, 0x4A148C), (0xC8E6C9, 0x1B5E20)],
];

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct Cfg {
    species: u32,
    fur: u32,
    size: u32,
    track: u32,
    reading: bool,
    roam: bool,
    stay: bool,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg { species: 0, fur: 0, size: 1, track: 0, reading: true, roam: true, stay: false }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Sleep,
    Sit,
    Walk,
    Run,
    Eat,
}

pub struct Pet {
    c: Cfg,
    pub x: f32,
    dir: f32,
    pub mode: Mode,
    load: f32,
    frame: u32,
    food: Option<f32>,
    eat_left: f32,
    hearts: f32,
    idle_turn: f32,
}

impl Pet {
    pub fn load(v: &Value) -> Pet {
        Pet { c: jget(v), x: 0.5, dir: 1.0, mode: Mode::Sit, load: 0.0, frame: 0, food: None, eat_left: 0.0, hearts: 0.0, idle_turn: 3.0 }
    }

    pub fn roams(&self) -> bool {
        self.c.roam
    }

    pub fn stays(&self) -> bool {
        self.c.stay
    }

    pub fn tracks_mem(&self) -> bool {
        self.c.track == 1
    }

    pub fn size_k(&self) -> f32 {
        [0.55, 0.72, 0.92][self.c.size as usize % 3]
    }

    /// Intervalo do próximo quadro (ms).
    pub fn frame_ms(&self) -> u32 {
        match self.mode {
            Mode::Sleep => 1000,
            Mode::Sit => 500,
            Mode::Eat => 150,
            Mode::Walk => 110,
            Mode::Run => 70,
        }
    }

    /// Avança a animação; `width` é a largura do percurso em px.
    pub fn step(&mut self, dt: f32, load: f32, width: f32) -> bool {
        self.load = self.load * 0.7 + load * 0.3;
        self.frame = self.frame.wrapping_add(1);
        if self.hearts > 0.0 {
            self.hearts = (self.hearts - dt).max(0.0);
        }
        if self.eat_left > 0.0 {
            self.eat_left -= dt;
            self.mode = Mode::Eat;
            if self.eat_left <= 0.0 {
                self.hearts = 2.0;
                self.mode = Mode::Sit;
            }
            return true;
        }
        if let Some(fx) = self.food {
            self.mode = Mode::Run;
            let d = fx - self.x;
            self.dir = d.signum();
            let sp = 260.0 * dt / width.max(1.0);
            if d.abs() <= sp {
                self.x = fx;
                self.food = None;
                self.eat_left = 2.2;
            } else {
                self.x += sp * self.dir;
            }
            return true;
        }
        let l = self.load;
        let new_mode = if l < 0.08 {
            Mode::Sleep
        } else if l < 0.25 {
            Mode::Sit
        } else if l < 0.6 {
            Mode::Walk
        } else {
            Mode::Run
        };
        self.mode = new_mode;
        let speed = match self.mode {
            Mode::Walk => 40.0,
            Mode::Run => 120.0,
            _ => 0.0,
        };
        if speed > 0.0 {
            self.x += self.dir * speed * dt / width.max(1.0);
            if self.x > 0.97 {
                self.x = 0.97;
                self.dir = -1.0;
            }
            if self.x < 0.03 {
                self.x = 0.03;
                self.dir = 1.0;
            }
        } else {
            self.idle_turn -= dt;
            if self.idle_turn <= 0.0 {
                self.idle_turn = 4.0 + (self.frame % 5) as f32;
                if self.mode == Mode::Sit {
                    self.dir = -self.dir;
                }
            }
        }
        true
    }

    pub fn feed(&mut self) {
        let fx = if self.x > 0.5 { (self.x - 0.35).max(0.05) } else { (self.x + 0.35).min(0.95) };
        self.food = Some(fx);
    }

    pub fn food_x(&self) -> Option<f32> {
        self.food
    }

    /// Desenha o bicho com os pés em `ground` (y), centro em `cx`.
    pub fn draw_creature(&self, c: &Canvas, cx: f32, ground: f32, s: f32) {
        let (main, accent) = FURS[self.c.species as usize % 9][self.c.fur as usize % 4];
        let main = Color::hex(main);
        let acc = Color::hex(accent);
        let ink = Color::rgba(30, 30, 36, 1.0);
        let f = self.dir;
        let t = self.frame as f32;
        let moving = matches!(self.mode, Mode::Walk | Mode::Run);
        let leg = if moving { (t * if self.mode == Mode::Run { 1.6 } else { 1.0 }).sin() } else { 0.0 };
        let bob = if moving { (t * 2.0).sin().abs() * s * 0.04 } else if self.mode == Mode::Eat { (t * 1.5).sin().abs() * s * 0.05 } else { 0.0 };
        let sleeping = self.mode == Mode::Sleep;
        // sombra
        c.fill_ellipse(cx, ground, s * 0.32, s * 0.05, Color(0.0, 0.0, 0.0, 0.25));
        let eye = |x: f32, y: f32| {
            if sleeping {
                c.line(x - s * 0.03, y, x + s * 0.03, y, ink, (s * 0.025).max(1.0));
            } else {
                c.fill_circle(x, y, s * 0.032, ink);
                c.fill_circle(x + s * 0.01 * f, y - s * 0.01, s * 0.01, WHITE);
            }
        };
        match self.c.species {
            0 | 1 | 3 | 4 => {
                // quadrúpedes: gato, cachorro, dragão, dino
                let sit = matches!(self.mode, Mode::Sit | Mode::Sleep);
                let by = ground - s * if sit { 0.26 } else { 0.3 } - bob;
                // pernas
                if !sleeping {
                    for (i, lx) in [-0.18f32, -0.08, 0.1, 0.2].iter().enumerate() {
                        let ph = if i % 2 == 0 { leg } else { -leg };
                        let x = cx + lx * s * f;
                        c.line(x, by + s * 0.08, x + ph * s * 0.06, ground - s * 0.01, main.mix(BLACK, 0.15), s * 0.07);
                    }
                }
                // rabo
                let tx = cx - f * s * 0.3;
                let wag = (t * 0.8).sin() * s * 0.05;
                if self.c.species == 3 || self.c.species == 4 {
                    c.fill_poly(&[(cx - f * s * 0.15, by - s * 0.02), (tx - f * s * 0.14, by + s * 0.08 + wag), (cx - f * s * 0.15, by + s * 0.12)], main);
                } else {
                    c.line(cx - f * s * 0.22, by, tx - f * s * 0.08, by - s * 0.18 + wag, main, s * 0.06);
                }
                // corpo
                c.fill_ellipse(cx, by + s * 0.04, s * 0.27, s * if sleeping { 0.13 } else { 0.15 }, main);
                c.fill_ellipse(cx + f * s * 0.04, by + s * 0.09, s * 0.15, s * 0.07, acc);
                if self.c.species == 3 {
                    // asas e espinhos do dragão
                    c.fill_poly(&[(cx - f * s * 0.05, by - s * 0.06), (cx - f * s * 0.2, by - s * 0.28 + (t * 0.5).sin() * s * 0.04), (cx + f * s * 0.08, by - s * 0.08)], acc.a(0.9));
                }
                if self.c.species == 4 {
                    for k in 0..3 {
                        let x = cx - f * s * (0.12 - k as f32 * 0.1);
                        c.fill_poly(&[(x - s * 0.04, by - s * 0.1), (x, by - s * 0.2), (x + s * 0.04, by - s * 0.1)], acc);
                    }
                }
                // cabeça
                let hx = cx + f * s * if sleeping { 0.2 } else { 0.24 };
                let hy = by - s * if sleeping { 0.0 } else { 0.12 };
                c.fill_circle(hx, hy, s * 0.15, main);
                match self.c.species {
                    0 => {
                        for e in [-1.0f32, 1.0] {
                            let ex = hx + e * s * 0.08;
                            c.fill_poly(&[(ex - s * 0.05, hy - s * 0.1), (ex, hy - s * 0.23), (ex + s * 0.05, hy - s * 0.1)], main);
                            c.fill_poly(&[(ex - s * 0.025, hy - s * 0.11), (ex, hy - s * 0.18), (ex + s * 0.025, hy - s * 0.11)], acc);
                        }
                        for k in [-1.0f32, 1.0] {
                            c.line(hx + f * s * 0.06, hy + s * 0.03, hx + f * s * 0.2, hy + s * 0.02 * k, ink.a(0.5), 1.0);
                        }
                    }
                    1 => {
                        c.fill_ellipse(hx - f * s * 0.1, hy, s * 0.05, s * 0.11, main.mix(BLACK, 0.25));
                        c.fill_ellipse(hx + f * s * 0.12, hy + s * 0.04, s * 0.08, s * 0.06, acc);
                        c.fill_circle(hx + f * s * 0.18, hy + s * 0.02, s * 0.03, ink);
                    }
                    3 => {
                        c.fill_poly(&[(hx - s * 0.04, hy - s * 0.12), (hx - f * s * 0.06, hy - s * 0.25), (hx + s * 0.03, hy - s * 0.13)], acc);
                        if self.mode == Mode::Run {
                            c.fill_circle(hx + f * s * 0.24, hy + s * 0.04, s * 0.04, ORANGE.a(0.8));
                        }
                    }
                    _ => {
                        c.fill_ellipse(hx + f * s * 0.1, hy + s * 0.05, s * 0.1, s * 0.06, main);
                    }
                }
                eye(hx + f * s * 0.06, hy - s * 0.02);
            }
            2 => {
                // passarinho
                let by = ground - s * 0.2 - bob * 2.0;
                c.line(cx - s * 0.04, by + s * 0.1, cx - s * 0.04 + leg * s * 0.03, ground, ORANGE, s * 0.03);
                c.line(cx + s * 0.04, by + s * 0.1, cx + s * 0.04 - leg * s * 0.03, ground, ORANGE, s * 0.03);
                c.fill_ellipse(cx, by, s * 0.17, s * 0.14, main);
                c.fill_ellipse(cx + f * s * 0.03, by + s * 0.04, s * 0.1, s * 0.07, acc);
                let flap = if moving { (t * 2.5).sin() * s * 0.08 } else { 0.0 };
                c.fill_poly(&[(cx - f * s * 0.02, by - s * 0.02), (cx - f * s * 0.18, by - s * 0.02 - flap), (cx - f * s * 0.08, by + s * 0.07)], main.mix(BLACK, 0.2));
                c.fill_poly(&[(cx + f * s * 0.16, by - s * 0.04), (cx + f * s * 0.26, by), (cx + f * s * 0.16, by + s * 0.03)], ORANGE);
                eye(cx + f * s * 0.08, by - s * 0.05);
            }
            8 => {
                // fantasminha (flutua)
                let fy = ground - s * 0.36 - (t * 0.6).sin() * s * 0.05;
                let w = s * 0.2;
                let mut pts = vec![(cx - w, fy)];
                for k in 0..=6 {
                    let x = cx - w + 2.0 * w * k as f32 / 6.0;
                    let y = fy + s * 0.28 + if k % 2 == 0 { 0.0 } else { -s * 0.06 } + (t * 0.8 + k as f32).sin() * s * 0.015;
                    pts.push((x, y));
                }
                pts.push((cx + w, fy));
                c.fill_poly(&pts, main.a(0.92));
                c.fill_circle(cx, fy, w, main.a(0.92));
                eye(cx + f * s * 0.02 - s * 0.06, fy);
                eye(cx + f * s * 0.02 + s * 0.06, fy);
                if !sleeping {
                    c.fill_ellipse(cx + f * s * 0.02, fy + s * 0.08, s * 0.03, s * 0.04, acc.a(0.8));
                }
            }
            _ => {
                // humanoides: guerreira, guerreiro, papai noel
                let by = ground - s * 0.42 - bob;
                let sit = matches!(self.mode, Mode::Sit | Mode::Sleep);
                let skin = acc;
                for (k, ph) in [(-1.0f32, leg), (1.0, -leg)] {
                    let lx = cx + k * s * 0.05;
                    let fy = if sit { ground - s * 0.04 } else { ground };
                    c.line(lx, by + s * 0.22, lx + ph * s * 0.08 + if sit { f * s * 0.12 } else { 0.0 }, fy, Color::rgba(60, 50, 45, 1.0), s * 0.07);
                }
                c.fill_rrect(Rect::new(cx - s * 0.12, by, s * 0.24, s * 0.26), s * 0.06, main);
                if self.c.species == 7 {
                    c.fill_rect(Rect::new(cx - s * 0.12, by + s * 0.14, s * 0.24, s * 0.04), Color::rgba(40, 30, 25, 1.0));
                    c.fill_rect(Rect::new(cx - s * 0.12, by + s * 0.22, s * 0.24, s * 0.04), WHITE);
                }
                // braço com espada
                let ax = cx + f * s * 0.12;
                c.line(ax, by + s * 0.06, ax + f * s * 0.08, by + s * 0.16 - leg * s * 0.04, skin, s * 0.05);
                if self.c.species != 7 {
                    c.line(ax + f * s * 0.08, by + s * 0.16, ax + f * s * 0.2, by - s * 0.06, Color::hex(0xCFD8DC), s * 0.03);
                } else {
                    c.fill_rrect(Rect::new(cx - f * s * 0.22 - s * 0.08, by + s * 0.02, s * 0.16, s * 0.18), s * 0.04, Color::hex(0x8D6E63));
                }
                let hx = cx;
                let hy = by - s * 0.1;
                c.fill_circle(hx, hy, s * 0.12, skin);
                match self.c.species {
                    5 => {
                        c.fill_ellipse(hx - f * s * 0.08, hy + s * 0.02, s * 0.07, s * 0.15, Color::hex(0xFFB300));
                        c.fill_rrect(Rect::new(hx - s * 0.13, hy - s * 0.14, s * 0.26, s * 0.07), s * 0.03, Color::hex(0xB0BEC5));
                    }
                    6 => {
                        c.fill_rrect(Rect::new(hx - s * 0.13, hy - s * 0.15, s * 0.26, s * 0.1), s * 0.04, Color::hex(0xB0BEC5));
                        c.fill_poly(&[(hx, hy - s * 0.15), (hx - f * s * 0.05, hy - s * 0.25), (hx + f * s * 0.06, hy - s * 0.15)], RED);
                    }
                    _ => {
                        c.fill_poly(&[(hx - s * 0.13, hy - s * 0.06), (hx + f * s * 0.2, hy - s * 0.26), (hx + s * 0.13, hy - s * 0.06)], RED);
                        c.fill_circle(hx + f * s * 0.2, hy - s * 0.26, s * 0.04, WHITE);
                        c.fill_ellipse(hx, hy + s * 0.08, s * 0.11, s * 0.08, WHITE);
                    }
                }
                eye(hx + f * s * 0.05, hy - s * 0.01);
            }
        }
        if sleeping {
            let zx = cx + f * s * 0.3;
            let k = (self.frame % 3) as f32;
            c.text("z", Rect::new(zx, ground - s * (0.55 + k * 0.08), s * 0.3, s * 0.2), s * (0.14 + k * 0.03), W_BOLD, WHITE.a(0.85), Align::Left);
        }
        if self.hearts > 0.0 {
            let hy = ground - s * (0.7 + (2.0 - self.hearts) * 0.15);
            c.text("❤", Rect::new(cx - s * 0.2, hy, s * 0.4, s * 0.2), s * 0.16, W_BOLD, PINK.a(self.hearts.min(1.0)), Align::Center);
        }
    }

    pub fn draw_food(&self, c: &Canvas, x: f32, ground: f32, s: f32) {
        c.fill_ellipse(x, ground - s * 0.03, s * 0.09, s * 0.04, Color::hex(0x8D6E63));
        c.fill_circle(x - s * 0.03, ground - s * 0.06, s * 0.025, Color::hex(0xFFB74D));
        c.fill_circle(x + s * 0.02, ground - s * 0.065, s * 0.025, Color::hex(0xFF8A65));
    }
}

impl Widget for Pet {
    fn save(&self) -> Value {
        jput(&self.c)
    }
    fn needs_sys(&self) -> bool {
        true
    }
    fn is_pet(&self) -> bool {
        true
    }
    fn as_pet(&mut self) -> Option<&mut Pet> {
        Some(self)
    }
    fn interval(&self, _sh: &Shared) -> Option<u32> {
        if self.c.reading {
            Some(2000)
        } else {
            None
        }
    }
    fn max_units(&self) -> u32 {
        2
    }
    fn draw(&mut self, t: &Tile, sh: &mut Shared) {
        // a "casinha": almofada; o bicho anda por cima da dock na camada própria
        let r = t.r;
        t.c.fill_rrect(r, t.rad(), Color(1.0, 1.0, 1.0, 0.06));
        t.c.stroke_rrect(r.inset(0.5), t.rad(), Color(1.0, 1.0, 1.0, 0.08), 1.0);
        let cushion = Rect::new(r.x + r.w * 0.1, r.b() - r.h * 0.34, r.w * 0.8, r.h * 0.22);
        t.c.fill_rrect(cushion, cushion.h / 2.0, Color::hex(0x9575CD).a(0.55));
        if self.c.reading {
            let v = if self.c.track == 1 { sh.sys.mem } else { sh.sys.cpu };
            t.c.text(&format!("{:.0}%", v * 100.0), Rect::new(r.x, r.y + r.h * 0.06, r.w, r.h * 0.3), t.fs(0.15), W_SEMIBOLD, WHITE.a(0.8), Align::Center);
        }
    }
    fn on_click(&mut self, _fx: f32, _fy: f32, _u: u32, _sh: &mut Shared) -> Click {
        Click::Panel
    }
    fn menu(&self) -> Vec<(u32, String)> {
        vec![(1, "Colocar comida".into())]
    }
    fn on_menu(&mut self, idm: u32, sh: &mut Shared) {
        if idm == 1 {
            self.feed();
            sh.redraw = true;
        }
    }
    fn panel_size(&self, _sh: &Shared) -> (f32, f32) {
        (300.0, 230.0)
    }
    fn panel(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let name = SPECIES[self.c.species as usize % 9];
        let state = match self.mode {
            Mode::Sleep => "dormindo",
            Mode::Sit => "sentado, tranquilo",
            Mode::Walk => "andando, o PC está trabalhando",
            Mode::Run => "correndo, o PC está ocupado",
            Mode::Eat => "comendo",
        };
        panel_header(ui, name, state);
        let r = ui.row(100.0);
        ui.c.fill_rrect(r, 10.0 * ui.s, Color(1.0, 1.0, 1.0, 0.05));
        self.draw_creature(ui.c, r.cx(), r.b() - 12.0 * ui.s, 110.0 * ui.s);
        ui.gap(8.0);
        let r = ui.row(32.0);
        if ui.button_k(id("feed"), Rect::new(r.x, r.y, 140.0 * ui.s, r.h), "Colocar comida", Btn::Primary) {
            self.feed();
            sh.close_panel = true;
        }
        let v = if self.c.track == 1 { sh.sys.mem } else { sh.sys.cpu };
        ui.c.text(&format!("{} {:.0}%", if self.c.track == 1 { "Memória" } else { "Processador" }, v * 100.0), r, 12.0 * ui.s, W_NORMAL, TXT2, Align::Right);
    }
    fn settings(&mut self, ui: &mut Ui, sh: &mut Shared) {
        ui.caption("BICHO");
        let cols = 3;
        for (row_i, chunk) in SPECIES.chunks(cols).enumerate() {
            let r = ui.row(30.0);
            let w = r.w / cols as f32;
            for (i, n) in chunk.iter().enumerate() {
                let idx = (row_i * cols + i) as u32;
                let k = if self.c.species == idx { Btn::Primary } else { Btn::Normal };
                if ui.button_k(idn("sp", idx as u64), Rect::new(r.x + w * i as f32 + 2.0, r.y + 2.0, w - 4.0, r.h - 4.0), n, k) {
                    self.c.species = idx;
                    self.c.fur = 0;
                    sh.save = true;
                }
            }
        }
        ui.caption("PELAGEM");
        let r = ui.row(30.0);
        let cols: Vec<Color> = FURS[self.c.species as usize % 9].iter().map(|c| Color::hex(c.0)).collect();
        if ui.swatches(id("fur"), r, &cols, &mut self.c.fur) {
            sh.save = true;
        }
        let mut ch = ui.labeled_segmented(id("size"), "Tamanho", &["Pequeno", "Médio", "Grande"], &mut self.c.size);
        ch |= ui.labeled_segmented(id("track"), "Acompanha", &["Processador", "Memória"], &mut self.c.track);
        ch |= ui.row_toggle(id("rd"), "Mostrar a leitura", &mut self.c.reading);
        ch |= ui.row_toggle(id("roam"), "Passear pela dock inteira", &mut self.c.roam);
        ch |= ui.row_toggle(id("stay"), "Ficar na tela quando a dock se esconde", &mut self.c.stay);
        if ch {
            sh.save = true;
        }
    }
}
