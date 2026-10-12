use std::path::{Path, PathBuf};

use super::policy::{Game, System};
use super::ui;

const WIDTH: i32 = 1280;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
impl Bounds {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x + self.width as i32
            && y < self.y + self.height as i32
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Credits,
    Settings,
    Previous,
    Next,
    System(System),
    Game(usize),
}

#[derive(Clone, Debug)]
pub struct MenuLayout {
    pub credits: Bounds,
    pub settings: Bounds,
    pub previous: Option<Bounds>,
    pub next: Option<Bounds>,
    pub systems: Vec<System>,
    pub system_buttons: Vec<Bounds>,
    pub game_indices: Vec<usize>,
    pub visible_game_indices: Vec<usize>,
    pub game_buttons: Vec<Bounds>,
    pub indicators: Vec<Bounds>,
    pub shown_game_index: Option<usize>,
}
impl MenuLayout {
    pub fn target_at(&self, x: i32, y: i32) -> Option<Target> {
        if self.credits.contains(x, y) {
            return Some(Target::Credits);
        }
        if self.settings.contains(x, y) {
            return Some(Target::Settings);
        }
        if self.previous.is_some_and(|b| b.contains(x, y)) {
            return Some(Target::Previous);
        }
        if self.next.is_some_and(|b| b.contains(x, y)) {
            return Some(Target::Next);
        }
        self.system_buttons
            .iter()
            .zip(&self.systems)
            .find_map(|(b, s)| b.contains(x, y).then_some(Target::System(*s)))
            .or_else(|| {
                self.game_buttons
                    .iter()
                    .zip(&self.visible_game_indices)
                    .find_map(|(b, i)| b.contains(x, y).then_some(Target::Game(*i)))
            })
    }
}

#[derive(Clone, Debug)]
pub struct MenuRenderer {
    pub cover_directory: PathBuf,
    pub settings_icon: Option<PathBuf>,
    rasters: std::cell::RefCell<std::collections::HashMap<(PathBuf, u32), Option<u32>>>,
}
impl Default for MenuRenderer {
    fn default() -> Self {
        Self {
            cover_directory: PathBuf::new(),
            settings_icon: None,
            rasters: Default::default(),
        }
    }
}
impl MenuRenderer {
    pub fn new(cover_directory: impl Into<PathBuf>, settings_icon: Option<PathBuf>) -> Self {
        Self {
            cover_directory: cover_directory.into(),
            settings_icon,
            rasters: Default::default(),
        }
    }
    pub fn render(
        &self,
        games: &[Game],
        active_system: System,
        position: usize,
        status: &str,
    ) -> MenuLayout {
        let bg = ui::color("background");
        ui::fill(0, 0, 1280, 480, bg);
        let credits = Bounds::new(12, 412, 56, 56);
        let settings = Bounds::new(1212, 412, 56, 56);
        ui::centered_in_rect(
            credits.x,
            credits.y,
            credits.width,
            credits.height,
            "(c)",
            2,
            ui::color("footer"),
        );
        self.draw_settings(settings);
        let systems = populated_systems(games);
        let mut system_buttons = Vec::new();
        let count = systems.len();
        if count > 0 {
            let width = (1168 - 8 * (count - 1) as u32) / count as u32;
            for (index, system) in systems.iter().enumerate() {
                let x = 56 + index as i32 * (width as i32 + 8);
                let b = Bounds::new(x, 76, width, 52);
                system_buttons.push(b);
                ui::pixel_panel(
                    x,
                    76,
                    width,
                    52,
                    ui::color(if *system == active_system {
                        "active"
                    } else {
                        "background"
                    }),
                    ui::color("accent"),
                    4,
                );
                let label = system.label();
                ui::centered_in_rect(
                    x,
                    76,
                    width,
                    52,
                    label,
                    ui::fit_text_scale(label, width.saturating_sub(16), 2, 1),
                    ui::color("text"),
                );
            }
        }
        let pairs: Vec<(usize, &Game)> = games
            .iter()
            .enumerate()
            .filter(|(_, g)| g.system == active_system)
            .collect();
        let game_indices = pairs.iter().map(|(i, _)| *i).collect::<Vec<_>>();
        let mut visible_game_indices = Vec::new();
        let mut game_buttons = Vec::new();
        let mut indicators = Vec::new();
        let mut shown = None;
        let mut previous = None;
        let mut next = None;
        if !pairs.is_empty() {
            let selected = position % pairs.len();
            shown = Some(pairs[selected].0);
            let visible_count = pairs.len().min(3);
            let first = if pairs.len() <= visible_count {
                0
            } else if selected == 0 {
                0
            } else if selected + 1 >= pairs.len() {
                pairs.len() - visible_count
            } else {
                selected - 1
            };
            let row_width =
                visible_count as i32 * 216 + (visible_count.saturating_sub(1) as i32) * 36;
            let mut x = (WIDTH - row_width) / 2;
            for (index, game) in pairs[first..first + visible_count].iter() {
                let b = Bounds::new(x, 154, 216, 264);
                visible_game_indices.push(*index);
                game_buttons.push(b);
                self.draw_card(b, game, *index == pairs[selected].0);
                x += 252;
            }
            if pairs.len() > 1 {
                previous = Some(Bounds::new(156, 232, 80, 100));
                next = Some(Bounds::new(1044, 232, 80, 100));
                draw_arrow(previous.unwrap(), true, ui::color("footer"));
                draw_arrow(next.unwrap(), false, ui::color("footer"));
            }
            let iw: i32 = 16;
            let ih: u32 = 8;
            let gap: i32 = 8;
            let total = pairs.len() as i32 * iw + (pairs.len().saturating_sub(1) as i32) * gap;
            let mut ix = (WIDTH - total) / 2;
            for i in 0..pairs.len() {
                let b = Bounds::new(ix, 438, iw as u32, ih);
                indicators.push(b);
                ui::outline(
                    ix,
                    438,
                    iw as u32,
                    ih,
                    ui::color(if i == selected {
                        "footer"
                    } else {
                        "control-border"
                    }),
                    2,
                );
                ix += iw + gap;
            }
        }
        if !status.is_empty() {
            ui::centered_in_rect(
                100,
                452,
                1080,
                24,
                status,
                ui::fit_text_scale(status, 1060, 2, 1),
                ui::color("footer"),
            );
        }
        MenuLayout {
            credits,
            settings,
            previous,
            next,
            systems,
            system_buttons,
            game_indices,
            visible_game_indices,
            game_buttons,
            indicators,
            shown_game_index: shown,
        }
    }
    fn cached_raster(&self, path: &Path, color: u32, png: bool) -> Option<u32> {
        let key = (path.to_path_buf(), color);
        if let Some(value) = self.rasters.borrow().get(&key) {
            return *value;
        }
        let value = if png {
            crate::canvas::load_png_raster(path, 23, 23)
        } else {
            crate::canvas::load_cover_raster(path, color)
        }
        .ok();
        self.rasters.borrow_mut().insert(key, value);
        value
    }
    fn draw_settings(&self, b: Bounds) {
        let size = 50.min(b.width).min(b.height).max(1);
        let x = b.x + ((b.width - size) / 2) as i32;
        let y = b.y + ((b.height - size) / 2) as i32;
        if let Some(handle) = self
            .settings_icon
            .as_deref()
            .and_then(|p| self.cached_raster(p, 0, true))
        {
            if crate::canvas::draw_raster(handle, x, y, size, size).is_ok() {
                return;
            }
        }
        const ROWS: [&str; 23] = [
            "..........000..........",
            ".........03320.........",
            "...000...03220...000...",
            "..0333000222220003220..",
            "..0322222222222232220..",
            "..0322222222222222210..",
            "...03222100000222210...",
            "...0222100.1.0022220...",
            "...022100..1..002220...",
            ".0022200...1...0022200.",
            "0332220...010...0222220",
            "03222201111011110222220",
            "0222220...010...0222210",
            ".0022200...1...0022200.",
            "...022200..1..003220...",
            "...0222200.1.0032220...",
            "...03222200000322220...",
            "..0322222222222222220..",
            "..0222222222222222220..",
            "..0221000222220003110..",
            "...000...02220...000...",
            ".........02210.........",
            "..........000..........",
        ];
        const COLORS: [u32; 4] = [0, 0x2e2e2e, 0x727272, 0xa0a0a0];
        for ty in 0..size {
            for tx in 0..size {
                let shade = ROWS[(ty * 23 / size) as usize].as_bytes()[(tx * 23 / size) as usize];
                if shade != b'.' {
                    ui::fill(
                        x + tx as i32,
                        y + ty as i32,
                        1,
                        1,
                        COLORS[(shade - b'0') as usize],
                    );
                }
            }
        }
    }
    fn draw_card(&self, b: Bounds, game: &Game, selected: bool) {
        ui::pixel_panel(
            b.x,
            b.y,
            b.width,
            b.height,
            ui::color(if selected { "active" } else { "background" }),
            ui::color("accent"),
            4,
        );
        let art = Bounds::new(b.x + 8, b.y + 8, b.width - 16, b.width - 16);
        let path = game
            .cover
            .clone()
            .unwrap_or_else(|| self.cover_directory.join(format!("{}.png", game.id)));
        let drawn = self
            .cached_raster(&path, game.color, false)
            .is_some_and(|h| {
                h != 0 && crate::canvas::draw_raster(h, art.x, art.y, art.width, art.height).is_ok()
            });
        if !drawn {
            draw_logo_or_cartridge(art, game);
        }
        let label = ui::fit_text_width(&game.title, b.width.saturating_sub(28), 2);
        ui::centered_in_rect(
            b.x + 8,
            b.y + b.width as i32,
            b.width - 16,
            b.height - b.width - 8,
            &label,
            2,
            ui::color("text"),
        );
    }
}
fn populated_systems(games: &[Game]) -> Vec<System> {
    [
        System::Nes,
        System::Gb,
        System::Gbc,
        System::Gba,
        System::Zx,
        System::Deck,
    ]
    .into_iter()
    .filter(|s| games.iter().any(|g| g.system == *s))
    .collect()
}
fn draw_arrow(b: Bounds, left: bool, color: u32) {
    let cx = b.x + b.width as i32 / 2;
    let cy = b.y + b.height as i32 / 2;
    let blocks = [
        (28, -2, 4, 4),
        (24, -6, 4, 4),
        (20, -10, 4, 4),
        (16, -14, 4, 4),
        (12, -18, 4, 4),
        (8, -22, 4, 10),
        (-28, -12, 36, 4),
        (-28, -8, 4, 16),
        (-28, 8, 36, 4),
        (8, 12, 4, 10),
        (12, 14, 4, 4),
        (16, 10, 4, 4),
        (20, 6, 4, 4),
        (24, 2, 4, 4),
    ];
    for (x, y, w, h) in blocks {
        let xx = if left { cx - x - w } else { cx + x };
        ui::fill(xx, cy + y, w as u32, h as u32, color);
    }
}
fn draw_logo_or_cartridge(b: Bounds, g: &Game) {
    let c = g.color;
    let cx = b.x + b.width as i32 / 2;
    let cy = b.y + b.height as i32 / 2;
    if g.system != System::Deck {
        draw_cartridge(b, c);
        return;
    }
    match g.id.as_str() {
        "ten-seconds" => ui::centered_in_rect(b.x, b.y, b.width, b.height, "10.00", 5, c),
        "lisp-repl" => ui::centered_in_rect(b.x, b.y, b.width, b.height, "(LISP)", 4, c),
        "lua-repl" => {
            ui::pixel_panel(
                b.x + 24,
                b.y + 46,
                b.width - 48,
                b.height - 92,
                ui::color("background"),
                c,
                4,
            );
            ui::centered_in_rect(b.x, b.y, b.width, b.height, "LUA>", 4, c);
        }
        "python-repl" => {
            ui::centered_in_rect(b.x, b.y, b.width, b.height, ">>>", 6, c);
            ui::fill(cx - 54, cy + 34, 108, 6, c);
        }
        "scheme-repl" => {
            ui::centered_in_rect(b.x, b.y, b.width, b.height, "(SCHEME)", 3, c);
            ui::fill(cx - 34, cy + 30, 68, 5, c);
        }
        "chiptunes" => {
            for (i, height) in [34, 62, 92, 48, 112, 74, 42, 86, 56]
                .into_iter()
                .enumerate()
            {
                ui::fill(
                    cx - 86 + i as i32 * 20,
                    cy - height / 2,
                    10,
                    height as u32,
                    c,
                );
            }
        }
        "terminal" => {
            let x = b.x + 30;
            let y = b.y + 44;
            let w = b.width - 60;
            ui::outline(x, y, w, 96, c, 4);
            ui::centered_in_rect(x, y, w, 96, ">_", 5, ui::color("text"));
            ui::fill(cx - 6, y + 96, 12, 18, c);
            ui::fill(cx - 44, y + 114, 88, 4, c);
        }
        "node-mode" => {
            for (x, y, w, h) in [
                (-48, -72, 96, 12),
                (-60, -60, 120, 12),
                (-72, -48, 144, 96),
                (-60, 48, 120, 12),
                (-48, 60, 96, 12),
            ] {
                ui::fill(cx + x, cy + y, w, h, c);
            }
            let white = ui::color("white");
            ui::fill(cx - 22, cy - 58, 6, 116, white);
            ui::fill(cx - 6, cy - 58, 6, 116, white);
            ui::centered_in_rect(b.x, b.y, b.width, b.height, "B", 14, white);
        }
        "reboot" => {
            for (x, y, w, h) in [
                (-5, -58, 10, 54),
                (-48, -34, 22, 8),
                (26, -34, 22, 8),
                (-58, -26, 8, 54),
                (50, -26, 8, 54),
                (-48, 28, 16, 8),
                (32, 28, 16, 8),
                (-32, 36, 64, 8),
            ] {
                ui::fill(cx + x, cy + y, w, h, c);
            }
        }
        _ => draw_cartridge(b, c),
    }
}
fn draw_cartridge(b: Bounds, c: u32) {
    let x = b.x + 34;
    let y = b.y + 28;
    let w = b.width.saturating_sub(68);
    let h = b.height.saturating_sub(56);
    ui::pixel_panel(x, y, w, h, ui::color("background"), c, 4);
    ui::fill(x + 24, y + 26, w.saturating_sub(48), 8, c);
    ui::fill(x + 24, y + 46, w.saturating_sub(48), 4, c);
    ui::fill(x + 20, y + h as i32 - 30, w.saturating_sub(40), 10, c);
}

#[cfg(test)]
#[path = "menu_tests.rs"]
mod tests;
