//! Thin Deck rendering and touch geometry around the third-party Tamagotchi P1 core.

use crate::canvas;
use std::cell::RefCell;
use std::rc::Rc;
use tamalib::{Button, Screen, ICONS_COUNT, SCREEN_HEIGHT, SCREEN_WIDTH};

const LCD_SCALE: u32 = 16;
const LCD_WIDTH: u32 = SCREEN_WIDTH as u32 * LCD_SCALE;
const LCD_HEIGHT: u32 = SCREEN_HEIGHT as u32 * LCD_SCALE;
const LCD_X: i32 = (canvas::WIDTH as i32 - LCD_WIDTH as i32) / 2;
const LCD_Y: i32 = 92;
const BUTTON_Y: i32 = 384;
const BUTTON_HEIGHT: u32 = 72;
const LCD_BACKGROUND: u32 = 0xc8d8a0;
const LCD_FOREGROUND: u32 = 0x1d2c14;
const BORDER: u32 = 0xfe6c27;
const TEXT: u32 = 0xeeeeee;
const P1_FIRMWARE_BYTES: usize = 12 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeckButton {
    Left,
    Middle,
    Right,
}

impl DeckButton {
    pub fn tamalib(self) -> Button {
        match self {
            Self::Left => Button::LEFT,
            Self::Middle => Button::MIDDLE,
            Self::Right => Button::RIGHT,
        }
    }
}

pub struct ScreenState {
    pixels: [bool; SCREEN_WIDTH * SCREEN_HEIGHT],
    icons: [bool; ICONS_COUNT],
    dirty: bool,
}

impl ScreenState {
    fn new() -> Self {
        Self {
            pixels: [false; SCREEN_WIDTH * SCREEN_HEIGHT],
            icons: [false; ICONS_COUNT],
            dirty: true,
        }
    }

    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }
}

struct DeckScreen {
    state: Rc<RefCell<ScreenState>>,
}

impl Screen for DeckScreen {
    fn update(&mut self) {
        self.state.borrow_mut().dirty = true;
    }

    fn set_pixel(&mut self, x: usize, y: usize, value: bool) {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            self.state.borrow_mut().pixels[y * SCREEN_WIDTH + x] = value;
        }
    }

    fn set_icon(&mut self, icon: usize, value: bool) {
        if icon < ICONS_COUNT {
            self.state.borrow_mut().icons[icon] = value;
        }
    }
}

pub fn make_screen() -> (Rc<RefCell<dyn Screen>>, Rc<RefCell<ScreenState>>) {
    let state = Rc::new(RefCell::new(ScreenState::new()));
    let screen: Rc<RefCell<dyn Screen>> = Rc::new(RefCell::new(DeckScreen {
        state: state.clone(),
    }));
    (screen, state)
}

pub fn valid_firmware(bytes: &[u8]) -> bool {
    bytes.len() == P1_FIRMWARE_BYTES
}

pub fn button_at(x: i32, y: i32) -> Option<DeckButton> {
    if !(BUTTON_Y..BUTTON_Y + BUTTON_HEIGHT as i32).contains(&y) || x < 0 {
        return None;
    }
    match x as u32 * 3 / canvas::WIDTH {
        0 => Some(DeckButton::Left),
        1 => Some(DeckButton::Middle),
        2 => Some(DeckButton::Right),
        _ => None,
    }
}

pub fn render(state: &ScreenState) {
    canvas::clear(0x000000);
    draw_text("TAMAGOTCHI P1", 484, 28, 3, TEXT);
    draw_text("HOLD 2S TO RETURN", 500, 66, 2, 0xafafaf);

    canvas::fill_rect(
        LCD_X - 20,
        LCD_Y - 20,
        LCD_WIDTH + 40,
        LCD_HEIGHT + 40,
        BORDER,
    );
    canvas::fill_rect(
        LCD_X - 12,
        LCD_Y - 12,
        LCD_WIDTH + 24,
        LCD_HEIGHT + 24,
        LCD_FOREGROUND,
    );
    canvas::fill_rect(LCD_X, LCD_Y, LCD_WIDTH, LCD_HEIGHT, LCD_BACKGROUND);
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            if state.pixels[y * SCREEN_WIDTH + x] {
                canvas::fill_rect(
                    LCD_X + (x as u32 * LCD_SCALE) as i32,
                    LCD_Y + (y as u32 * LCD_SCALE) as i32,
                    LCD_SCALE,
                    LCD_SCALE,
                    LCD_FOREGROUND,
                );
            }
        }
    }

    for (icon, active) in state.icons.iter().copied().enumerate() {
        let x = LCD_X + 12 + icon as i32 * 62;
        canvas::fill_rect(
            x,
            LCD_Y + 12,
            36,
            8,
            if active { LCD_FOREGROUND } else { 0xa8b888 },
        );
    }

    for (index, label) in ["LEFT", "MIDDLE", "RIGHT"].iter().enumerate() {
        let x = index as i32 * (canvas::WIDTH / 3) as i32 + 16;
        canvas::fill_rect(x, BUTTON_Y, canvas::WIDTH / 3 - 32, BUTTON_HEIGHT, 0x303030);
        canvas::fill_rect(x, BUTTON_Y, canvas::WIDTH / 3 - 32, 3, BORDER);
        let text_width = label.len() as i32 * 18;
        draw_text(
            label,
            x + ((canvas::WIDTH / 3 - 32) as i32 - text_width) / 2,
            BUTTON_Y + 24,
            3,
            TEXT,
        );
    }
}

fn draw_text(text: &str, x: i32, y: i32, scale: u32, color: u32) {
    for (index, byte) in text.bytes().enumerate() {
        canvas::draw_glyph(x + index as i32 * 6 * scale as i32, y, byte, scale, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_complete_p1_firmware_size() {
        assert!(!valid_firmware(&[]));
        assert!(!valid_firmware(&vec![0; P1_FIRMWARE_BYTES - 2]));
        assert!(valid_firmware(&vec![0; P1_FIRMWARE_BYTES]));
        assert!(!valid_firmware(&vec![0; P1_FIRMWARE_BYTES + 2]));
    }

    #[test]
    fn divides_the_deck_button_row_into_three() {
        assert_eq!(button_at(0, BUTTON_Y), Some(DeckButton::Left));
        assert_eq!(button_at(426, BUTTON_Y + 20), Some(DeckButton::Left));
        assert_eq!(button_at(427, BUTTON_Y + 20), Some(DeckButton::Middle));
        assert_eq!(button_at(854, BUTTON_Y + 20), Some(DeckButton::Right));
        assert_eq!(button_at(1279, BUTTON_Y + 20), Some(DeckButton::Right));
        assert_eq!(button_at(300, BUTTON_Y - 1), None);
        assert_eq!(button_at(-1, BUTTON_Y), None);
    }
}
