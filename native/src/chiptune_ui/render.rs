use super::{Mode, PlayerState, display_text, format_time};
use crate::canvas;

const BACKGROUND: u32 = 0;
const ORANGE: u32 = quantize(0xfe6c27);
const ACTIVE: u32 = quantize(0x4d372d);
const TEXT: u32 = 0xffffff;
const GREEN: u32 = quantize(0x87af87);
const RED: u32 = quantize(0xaf8787);
const MUTED: u32 = quantize(0x969696);
const INDICATOR: u32 = quantize(0x6c6c6c);

const fn quantize(color: u32) -> u32 {
    (((color >> 19) & 31) * 255 / 31) << 16
        | (((color >> 10) & 63) * 255 / 63) << 8
        | ((color >> 3) & 31) * 255 / 31
}

fn rect(x: i32, y: i32, width: i32, height: i32, color: u32) {
    if width > 0 && height > 0 {
        canvas::fill_rect(
            16 + 2 * x,
            16 + 2 * y,
            2 * width as u32,
            2 * height as u32,
            color,
        );
    }
}

fn stroke(x: i32, y: i32, width: i32, height: i32, thickness: i32, color: u32) {
    rect(x, y, width, thickness, color);
    rect(x, y + height - thickness, width, thickness, color);
    rect(x, y, thickness, height, color);
    rect(x + width - thickness, y, thickness, height, color);
}

fn cut(x: i32, y: i32, width: i32, height: i32, color: u32) {
    rect(x + 2, y, width - 4, height, color);
    rect(x, y + 2, width, height - 4, color);
}

fn panel(x: i32, y: i32, width: i32, height: i32) {
    cut(x, y, width, height, ORANGE);
    cut(x + 2, y + 2, width - 4, height - 4, ACTIVE);
}

fn text_width(text: &str, scale: i32) -> i32 {
    if text.is_empty() {
        0
    } else {
        (text.len() as i32 * 6 - 1) * scale
    }
}

fn text(x: i32, y: i32, text: &str, scale: i32, color: u32) {
    for (index, character) in text.bytes().enumerate() {
        canvas::draw_glyph(
            16 + 2 * x + index as i32 * 12 * scale,
            16 + 2 * y,
            character,
            2 * scale as u32,
            color,
        );
    }
}

fn centered(y: i32, shown: &str, scale: i32, color: u32) {
    text(
        ((624 - text_width(shown, scale)).div_euclid(2)).max(0),
        y,
        shown,
        scale,
        color,
    );
}

fn line(mut x: i32, mut y: i32, to_x: i32, to_y: i32, color: u32) {
    let dx = (to_x - x).abs();
    let sx = if x < to_x { 1 } else { -1 };
    let dy = -(to_y - y).abs();
    let sy = if y < to_y { 1 } else { -1 };
    let mut error = dx + dy;
    loop {
        rect(x, y, 1, 1, color);
        if x == to_x && y == to_y {
            break;
        }
        let doubled = 2 * error;
        if doubled >= dy {
            error += dy;
            x += sx;
        }
        if doubled <= dx {
            error += dx;
            y += sy;
        }
    }
}

fn arrow(x: i32, y: i32, right: bool) {
    let direction = if right { -1 } else { 1 };
    line(x, y, x + direction * 4, y - 4, TEXT);
    line(x, y, x + direction * 4, y + 4, TEXT);
}

fn triangle(x: i32, y: i32, right: bool) {
    for row in (-6_i32..=6).step_by(2) {
        let width = 14 - row.abs() * 2;
        let left = if right { x - 6 } else { x + 6 - width };
        rect(left, y + row, width, 2, TEXT);
    }
}

fn mode_icon(mode: Mode) {
    let (x, y) = (159, 194);
    if mode == Mode::Shuffle {
        line(x - 12, y - 5, x - 7, y - 5, TEXT);
        line(x - 7, y - 5, x + 6, y + 5, TEXT);
        line(x - 12, y + 5, x - 7, y + 5, TEXT);
        line(x - 7, y + 5, x + 6, y - 5, TEXT);
        line(x + 6, y - 5, x + 10, y - 5, TEXT);
        line(x + 6, y + 5, x + 10, y + 5, TEXT);
        arrow(x + 12, y - 5, true);
        arrow(x + 12, y + 5, true);
    } else {
        line(x - 11, y - 5, x + 9, y - 5, TEXT);
        arrow(x + 11, y - 5, true);
        line(x + 11, y + 5, x - 9, y + 5, TEXT);
        arrow(x - 11, y + 5, false);
        if mode == Mode::LoopOne {
            text(x - 2, y - 3, "1", 1, TEXT);
        }
    }
}

fn waveform(visual: &[i16]) {
    rect(96, 84, 432, 44, BACKGROUND);
    rect(96, 105, 432, 1, MUTED);
    if visual.len() == 1470 {
        for x in 0..432 {
            let frame = x * 735 / 432;
            let mixed = (i32::from(visual[frame * 2]) + i32::from(visual[frame * 2 + 1])) / 2;
            let height = (mixed.abs() / 1050).min(20);
            rect(
                96 + x as i32,
                if mixed < 0 { 106 } else { 105 - height },
                1,
                height.max(1),
                ORANGE,
            );
        }
    }
}

fn progress(state: &PlayerState) {
    let length = state
        .metadata
        .as_ref()
        .map_or(-1, |metadata| metadata.length_ms);
    let progress = if length > 0 {
        (i64::from(state.position) * 432 / i64::from(length)).clamp(0, 432) as i32
    } else {
        0
    };
    rect(96, 134, 432, 3, BACKGROUND);
    rect(96, 134, progress, 3, GREEN);
}

pub(super) fn playback(state: &PlayerState) {
    waveform(&state.visual);
    progress(state);
    rect(96, 143, text_width("88:88", 1) + 2, 7, ACTIVE);
    text(96, 143, &format_time(state.position), 1, TEXT);
}

pub(super) fn full(state: &PlayerState, volume: i32) {
    canvas::clear(0);
    rect(0, 0, 624, 224, BACKGROUND);
    panel(236, 4, 152, 29);
    centered(12, "CHIPTUNES", 1, TEXT);
    for offset in (-8..=8).step_by(2) {
        rect(585 + offset, 20 + offset, 2, 2, TEXT);
        rect(585 + offset, 20 - offset, 2, 2, TEXT);
    }
    let label = if volume > 0 {
        format!("VOL {volume}")
    } else {
        "VOL OFF".to_owned()
    };
    text(8, 14, &label, 1, if volume > 0 { GREEN } else { RED });
    panel(78, 42, 468, 120);
    if let Some(metadata) = &state.metadata {
        centered(50, &display_text(&metadata.title, 45), 2, TEXT);
        centered(70, &display_text(&metadata.subtitle, 72), 1, MUTED);
        waveform(&state.visual);
        progress(state);
        let end_time = if metadata.length_ms > 0 {
            format_time(metadata.length_ms)
        } else {
            "--:--".to_owned()
        };
        text(96, 143, &format_time(state.position), 1, TEXT);
        text(528 - text_width(&end_time, 1), 143, &end_time, 1, TEXT);
        let details = format!(
            "{}  FILE {}/{}  TRACK {}/{}",
            display_text(&metadata.system, 18),
            state.file_index + 1,
            state.files.len(),
            metadata.track_index + 1,
            metadata.track_count
        );
        centered(143, &display_text(&details, 56), 1, MUTED);
        let visible = state.files.len().min(40);
        let first = if state.files.len() > visible {
            state
                .file_index
                .saturating_sub(visible / 2)
                .min(state.files.len() - visible)
        } else {
            0
        };
        let row_width = visible as i32 * 6 + (visible as i32 - 1) * 4;
        for index in 0..visible {
            stroke(
                (624 - row_width) / 2 + index as i32 * 10,
                166,
                6,
                4,
                1,
                if first + index == state.file_index {
                    ORANGE
                } else {
                    INDICATOR
                },
            );
        }
    } else {
        centered(72, "NO CHIPTUNES FOUND", 2, TEXT);
        centered(103, &display_text(&state.status, 72), 1, MUTED);
        centered(
            126,
            "AY GBS GYM HES KSS NSF NSFE OGG SAP SPC VGM VGZ",
            1,
            TEXT,
        );
    }
    mode_icon(state.mode);
    rect(251, 187, 2, 14, TEXT);
    triangle(262, 194, false);
    if state.paused {
        triangle(362, 194, true);
    } else {
        rect(358, 187, 3, 14, TEXT);
        rect(365, 187, 3, 14, TEXT);
    }
    triangle(464, 194, true);
    rect(473, 187, 2, 14, TEXT);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chiptune::{Metadata as NativeMetadata, Player};
    use std::path::Path;

    #[test]
    fn ready_and_decoded_frames_match_reference() {
        let mut state = PlayerState::new(vec!["crazy.ogg".into()], 1, String::new());
        state.metadata = Some(super::super::Metadata::compose(
            Path::new("crazy.ogg"),
            NativeMetadata {
                title: b"crazy".to_vec(),
                artist: vec![],
                author: vec![],
                system: vec![],
                length_ms: 50000,
                track_index: 0,
                track_count: 1,
            },
        ));
        full(&state, 42);
        assert_eq!(canvas::rgb565_hash(), 0xb3f8e405ba8146cd);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../chiptunes/crazy.ogg");
        let mut player = Player::open(&path).unwrap();
        state.metadata = Some(super::super::Metadata::compose(&path, player.metadata()));
        for _ in 0..4 {
            player.decode().unwrap();
        }
        state.visual = player.samples().to_vec();
        state.position = player.position_ms();
        full(&state, 42);
        assert_eq!(canvas::rgb565_hash(), 0xe58bd71b6bd7e1cd);
    }
}
