//! Dashboard bitmap UI primitives shared by the menu and settings views.
use std::sync::{OnceLock, RwLock};

use crate::canvas;

pub const GLYPH_WIDTH: u32 = 5;
pub const GLYPH_HEIGHT: u32 = 7;
pub const GLYPH_ADVANCE: u32 = 6;

static PALETTE: OnceLock<RwLock<Vec<(String, u32)>>> = OnceLock::new();

fn palette() -> &'static RwLock<Vec<(String, u32)>> {
    PALETTE.get_or_init(|| {
        RwLock::new(
            super::policy::DEFAULT_PALETTE_VALUES
                .iter()
                .map(|(role, value)| ((*role).to_owned(), *value))
                .collect(),
        )
    })
}

pub fn install_palette(entries: Vec<(String, u32)>) {
    *palette().write().expect("dashboard palette lock poisoned") = entries;
}

pub fn color(role: &str) -> u32 {
    palette()
        .read()
        .expect("dashboard palette lock poisoned")
        .iter()
        .find(|(name, _)| name == role)
        .map(|(_, value)| *value)
        .unwrap_or_else(|| panic!("unknown dashboard color {role}"))
}

pub fn display_ascii(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect()
}

pub fn bitmap_text_width(text: &str, scale: u32) -> u32 {
    let shown = display_ascii(text);
    if shown.is_empty() {
        0
    } else {
        (shown.len() as u32 * GLYPH_ADVANCE * scale) - scale
    }
}

pub fn fit_text_scale(text: &str, maximum_width: u32, preferred: u32, minimum: u32) -> u32 {
    (minimum..=preferred)
        .rev()
        .find(|scale| bitmap_text_width(text, *scale) <= maximum_width)
        .unwrap_or(minimum)
}

pub fn fit_text_width(text: &str, maximum_width: u32, scale: u32) -> String {
    let shown = display_ascii(text);
    if bitmap_text_width(&shown, scale) <= maximum_width {
        return shown;
    }
    let capacity = if maximum_width > 0 {
        ((maximum_width + scale) / (GLYPH_ADVANCE * scale)) as usize
    } else {
        0
    };
    if capacity == 0 {
        String::new()
    } else if capacity <= 3 {
        shown.chars().take(capacity).collect()
    } else {
        format!(
            "{}...",
            shown.chars().take(capacity - 3).collect::<String>()
        )
    }
}

pub fn text(x: i32, y: i32, value: &str, scale: u32, colour: u32) {
    for (offset, character) in display_ascii(value).bytes().enumerate() {
        canvas::draw_glyph(
            x + (offset as i32 * (GLYPH_ADVANCE * scale) as i32),
            y,
            character,
            scale,
            colour,
        );
    }
}

/// Draw text at the requested top coordinate, centered horizontally in `width`.
pub fn centered_text(x: i32, y: i32, width: u32, value: &str, scale: u32, colour: u32) {
    let text_width = bitmap_text_width(value, scale);
    text(
        x + ((width as i32 - text_width as i32).max(0) / 2),
        y,
        value,
        scale,
        colour,
    );
}

pub fn centered_in_rect(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    value: &str,
    scale: u32,
    colour: u32,
) {
    let top = y + ((height as i32 - (GLYPH_HEIGHT * scale) as i32).max(0) / 2);
    centered_text(x, top, width, value, scale, colour);
}

pub fn fill(x: i32, y: i32, width: u32, height: u32, colour: u32) {
    if width != 0 && height != 0 {
        canvas::fill_rect(x, y, width, height, colour);
    }
}

pub fn outline(x: i32, y: i32, width: u32, height: u32, colour: u32, stroke: u32) {
    fill(x, y, width, stroke, colour);
    fill(x, y + height as i32 - stroke as i32, width, stroke, colour);
    fill(x, y, stroke, height, colour);
    fill(x + width as i32 - stroke as i32, y, stroke, height, colour);
}

pub fn pixel_panel(x: i32, y: i32, width: u32, height: u32, inside: u32, border: u32, cut: u32) {
    if width <= cut * 2 || height <= cut * 2 {
        return;
    }
    fill(x + cut as i32, y, width - cut * 2, height, border);
    fill(x, y + cut as i32, width, height - cut * 2, border);
    let inner_w = width - cut * 2;
    let inner_h = height - cut * 2;
    if inner_w > cut * 2 && inner_h > cut * 2 {
        fill(
            x + cut as i32 * 2,
            y + cut as i32,
            inner_w - cut * 2,
            inner_h,
            inside,
        );
        fill(
            x + cut as i32,
            y + cut as i32 * 2,
            inner_w,
            inner_h - cut * 2,
            inside,
        );
    }
}
