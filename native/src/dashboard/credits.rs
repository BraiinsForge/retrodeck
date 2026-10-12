//! The credits page: bounded FOSS credits parsing and the original crawl renderer.
//!
//! This module deliberately owns no dashboard policy.  `Credits` is a small
//! deterministic page state which the menu can embed and drive from its clock.

use crate::canvas;

pub const WIDTH: u32 = 1280;
pub const HEIGHT: u32 = 480;
pub const CREDITS_PATH: &str = "/mnt/data/nes-deck/menu/credits.tsv";
pub const ARCHIVE_PATH: &str = "/mnt/data/nes-deck/licenses";

const SCALE: u32 = 4;
const MAX_FILE_BYTES: usize = 32_768;
const MAX_PROJECTS: usize = 64;
const MAX_PROJECT_BYTES: usize = 48;
const MAX_ROLE_BYTES: usize = 64;
const MAX_LICENSE_BYTES: usize = 64;
const MAX_LINE_WIDTH: u32 = 1040;
const LINE_ADVANCE: i32 = 44;
const SECTION_GAP: i32 = 28;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Credit {
    pub project: String,
    pub role: String,
    pub license: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrawlLine {
    pub text: String,
    pub source_y: i32,
    pub source_width: u32,
    pub source_height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Crawl {
    pub lines: Vec<CrawlLine>,
    pub static_lines: Vec<String>,
    pub content_height: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Close,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Touch {
    pub x: i32,
    pub y: i32,
    pub pressed: bool,
    pub released: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    Close,
}

#[derive(Clone, Debug)]
pub struct Credits {
    pub crawl: Crawl,
    elapsed_ms: i64,
    pressed_target: Option<Target>,
}

fn ascii(value: &str) -> String {
    value
        .chars()
        .map(|character| if character.is_ascii() { character } else { '?' })
        .collect()
}
fn text_width(text: &str, scale: u32) -> u32 {
    text.len()
        .saturating_mul(6)
        .saturating_mul(scale as usize)
        .saturating_sub(scale as usize) as u32
}
fn draw_text(x: i32, y: i32, text: &str, scale: u32, color: u32) {
    let shown = ascii(text);
    for (index, byte) in shown.bytes().enumerate() {
        canvas::draw_glyph(x + (index as i32 * 6 * scale as i32), y, byte, scale, color);
    }
}
fn draw_centered(x: i32, y: i32, width: u32, height: u32, text: &str, scale: u32, color: u32) {
    let shown = ascii(text);
    let text_width = text_width(&shown, scale);
    let text_height = 7 * scale;
    draw_text(
        x + (width as i32 - text_width as i32).max(0) / 2,
        y + (height as i32 - text_height as i32).max(0) / 2,
        &shown,
        scale,
        color,
    );
}
fn fit_text(text: &str, maximum: u32, scale: u32) -> String {
    let shown = ascii(text);
    if text_width(&shown, scale) <= maximum {
        return shown;
    }
    let capacity = if maximum > 0 {
        ((maximum + scale) / (6 * scale)) as usize
    } else {
        0
    };
    match capacity {
        0 => String::new(),
        1..=3 => shown.chars().take(capacity).collect(),
        _ => {
            let mut result: String = shown.chars().take(capacity - 3).collect();
            result.push_str("...");
            result
        }
    }
}
fn wrap(text: &str) -> Vec<String> {
    let mut remaining = ascii(text);
    let max_chars = ((MAX_LINE_WIDTH + SCALE) / (6 * SCALE)) as usize;
    let mut result = Vec::new();
    while !remaining.is_empty() {
        remaining = remaining.trim_start_matches(' ').to_owned();
        if remaining.is_empty() {
            break;
        }
        if text_width(&remaining, SCALE) <= MAX_LINE_WIDTH {
            result.push(remaining);
            break;
        }
        let end = remaining.len().min(max_chars + 1);
        let split = remaining[..end]
            .rfind(' ')
            .filter(|&n| n != 0)
            .unwrap_or(max_chars);
        result.push(remaining[..split].to_owned());
        remaining = remaining[split..].to_owned();
    }
    result
}
fn valid_field(field: &str, max: usize) -> bool {
    !field.is_empty()
        && field.len() <= max
        && field
            .bytes()
            .all(|b| (0x20..=0x7e).contains(&b) && b != b'\t')
}

/// Parse the exact three-column credits format used by the Lisp page.
pub fn parse(input: &str) -> Result<Vec<Credit>, String> {
    if input.len() > MAX_FILE_BYTES {
        return Err("credits file exceeds 32768 bytes".into());
    }
    let mut result = Vec::new();
    for (number, raw) in input.split('\n').enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 3
            || !valid_field(fields[0], MAX_PROJECT_BYTES)
            || !valid_field(fields[1], MAX_ROLE_BYTES)
            || !valid_field(fields[2], MAX_LICENSE_BYTES)
        {
            return Err(format!("invalid credits row {}", number + 1));
        }
        if result.iter().any(|c: &Credit| c.project == fields[0]) {
            return Err(format!("duplicate credits project {}", fields[0]));
        }
        result.push(Credit {
            project: fields[0].into(),
            role: fields[1].into(),
            license: fields[2].into(),
        });
        if result.len() > MAX_PROJECTS {
            return Err("credits contain more than 64 projects".into());
        }
    }
    if result.is_empty() {
        return Err("credits contain no projects".into());
    }
    Ok(result)
}

pub fn load(path: &std::path::Path) -> Result<Vec<Credit>, String> {
    if !path.is_absolute() {
        return Err("credits path must be absolute".into());
    }
    let data =
        std::fs::read(path).map_err(|e| format!("cannot read credits {}: {e}", path.display()))?;
    if data.len() > MAX_FILE_BYTES {
        return Err("credits file exceeds 32768 bytes".into());
    }
    let text = String::from_utf8(data).map_err(|_| "credits file is not UTF-8".to_owned())?;
    parse(&text)
}

fn append_lines(lines: &mut Vec<CrawlLine>, cursor: &mut i32, text: &str) {
    for shown in wrap(text) {
        let width = text_width(&shown, SCALE);
        lines.push(CrawlLine {
            text: shown,
            source_y: *cursor,
            source_width: width,
            source_height: 7 * SCALE,
        });
        *cursor += LINE_ADVANCE;
    }
}

pub fn make_crawl(credits: &[Credit]) -> Crawl {
    let mut cursor = 0;
    let mut lines = Vec::new();
    let mut static_lines = Vec::new();
    if !credits.is_empty() {
        append_lines(&mut lines, &mut cursor, "RETRO DECK");
        append_lines(&mut lines, &mut cursor, "BUILT ON FREE SOFTWARE");
        cursor += SECTION_GAP;
        for credit in credits {
            static_lines.push(ascii(&format!("{} / {}", credit.project, credit.license)));
            append_lines(&mut lines, &mut cursor, &credit.project);
            append_lines(&mut lines, &mut cursor, &credit.role);
            append_lines(&mut lines, &mut cursor, &credit.license);
            cursor += SECTION_GAP;
        }
        append_lines(&mut lines, &mut cursor, "LICENSE TEXT ARCHIVE");
        append_lines(&mut lines, &mut cursor, ARCHIVE_PATH);
        cursor += SECTION_GAP;
        append_lines(&mut lines, &mut cursor, "THANK YOU");
    }
    Crawl {
        lines,
        static_lines,
        content_height: cursor,
    }
}

impl Credits {
    pub fn new(credits: Vec<Credit>) -> Self {
        Self {
            crawl: make_crawl(&credits),
            elapsed_ms: 0,
            pressed_target: None,
        }
    }
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }
    pub fn tick(&mut self, now_ms: i64) {
        self.elapsed_ms = now_ms.max(0);
    }
    pub fn animated(&self) -> bool {
        !self.crawl.lines.is_empty() && self.crawl.content_height > 0
    }
    pub fn next_deadline(&self) -> Option<i64> {
        self.animated()
            .then_some(self.elapsed_ms.saturating_add(33))
    }
    pub fn target_at(x: i32, y: i32) -> Option<Target> {
        (1212..1268)
            .contains(&x)
            .then_some(Target::Close)
            .filter(|_| (12..68).contains(&y))
    }
    pub fn touch(&mut self, report: Touch) -> Option<Effect> {
        let target = Self::target_at(report.x, report.y);
        if report.pressed {
            self.pressed_target = target;
        }
        if report.released {
            let old = self.pressed_target.take();
            if old == target && target == Some(Target::Close) {
                return Some(Effect::Close);
            }
        }
        None
    }

    pub fn clear_pressed(&mut self) {
        self.pressed_target = None;
    }

    pub fn render(
        &self,
        reduced_motion: bool,
        background: u32,
        accent: u32,
        text: u32,
        muted: u32,
    ) -> Result<(), String> {
        canvas::clear(background);
        if !reduced_motion {
            for index in 0..96_i32 {
                if index % 7 != 0 {
                    let x = (index * 193 + 47) % WIDTH as i32;
                    let y = (index * 83 + 29) % HEIGHT as i32;
                    let size = if index % 11 == 0 { 2 } else { 1 };
                    canvas::fill_rect(x, y, size, size, muted);
                }
            }
        }
        if !self.animated() {
            draw_centered(80, 180, 1120, 120, "CREDITS UNAVAILABLE", 3, text);
        } else if reduced_motion {
            self.render_static(accent, text, muted);
        } else {
            canvas::clear_text_masks();
            let cycle = self.crawl.content_height.saturating_add(4000) as u32;
            canvas::configure_projection(
                self.elapsed_ms,
                1,
                20,
                cycle,
                420,
                4000,
                56,
                72,
                104,
                210,
                480,
                accent,
            )?;
            for line in &self.crawl.lines {
                let handle = canvas::load_text_mask(line.text.as_bytes(), SCALE)?;
                canvas::draw_projected_text(handle, line.source_y)?;
            }
        }
        self.draw_close(muted);
        Ok(())
    }
    fn render_static(&self, accent: u32, text: u32, muted: u32) {
        draw_text(20, 20, "FOSS CREDITS", 2, accent);
        draw_text(20, 48, "PROJECT / LICENSE", 1, muted);
        let rows = 16usize;
        let columns = (self.crawl.static_lines.len() + rows - 1).max(1) / rows;
        let column_width = (WIDTH as i32 - 48) / columns as i32;
        for (index, line) in self.crawl.static_lines.iter().enumerate() {
            let column = index / rows;
            let row = index % rows;
            draw_text(
                24 + column as i32 * column_width,
                78 + row as i32 * 22,
                &fit_text(line, (column_width - 20) as u32, 1),
                1,
                text,
            );
        }
        draw_text(20, 458, ARCHIVE_PATH, 1, muted);
    }
    fn draw_close(&self, color: u32) {
        let cx = 1212 + 28;
        let cy = 12 + 28;
        for offset in (-12..=12).step_by(4) {
            canvas::fill_rect(cx + offset, cy + offset, 4, 4, color);
            canvas::fill_rect(cx + offset, cy - offset, 4, 4, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_and_rejects_credits() {
        assert_eq!(parse("# x\na\tb\tMIT\n").unwrap()[0].project, "a");
        assert!(parse("a\tb\tMIT\na\tb\tBSD\n").is_err());
    }
    #[test]
    fn crawl_geometry_is_deterministic() {
        let page = Credits::new(vec![Credit {
            project: "A".into(),
            role: "author".into(),
            license: "MIT".into(),
        }]);
        assert_eq!(page.crawl.lines[0].source_y, 0);
        assert_eq!(page.crawl.lines.last().unwrap().text, "THANK YOU");
    }
    #[test]
    fn close_requires_matching_release() {
        let mut page = Credits::empty();
        assert_eq!(
            page.touch(Touch {
                x: 1220,
                y: 20,
                pressed: true,
                released: false
            }),
            None
        );
        assert_eq!(
            page.touch(Touch {
                x: 100,
                y: 20,
                pressed: false,
                released: true
            }),
            None
        );
        assert_eq!(
            page.touch(Touch {
                x: 1220,
                y: 20,
                pressed: false,
                released: true
            }),
            None
        );
        assert_eq!(
            page.touch(Touch {
                x: 1220,
                y: 20,
                pressed: true,
                released: true
            }),
            Some(Effect::Close)
        );
    }
}
