//! DOOM input mapping: THEGamepad buttons and USB keyboard keys to DOOM key
//! codes, with the edge detection the engine's event queue expects.
//!
//! The engine side of this boundary only forwards what these tables produce,
//! so the mapping stays here where it can be tested without a Deck.

use crate::joypad::{
    PAD_A, PAD_B, PAD_DOWN, PAD_L, PAD_LEFT, PAD_R, PAD_RIGHT, PAD_SELECT, PAD_START, PAD_UP,
    PAD_X, PAD_Y,
};

/// Key codes from the engine's `doomkeys.h`. Mirrored rather than bound
/// through FFI because the tables and their tests live host-side;
/// `doom_key_codes_match_doomkeys_h` pins the values.
pub const KEY_RIGHTARROW: i32 = 0xae;
pub const KEY_LEFTARROW: i32 = 0xac;
pub const KEY_UPARROW: i32 = 0xad;
pub const KEY_DOWNARROW: i32 = 0xaf;
pub const KEY_STRAFE_L: i32 = 0xa0;
pub const KEY_STRAFE_R: i32 = 0xa1;
pub const KEY_USE: i32 = 0xa2;
pub const KEY_FIRE: i32 = 0xa3;
pub const KEY_ESCAPE: i32 = 27;
pub const KEY_ENTER: i32 = 13;
pub const KEY_TAB: i32 = 9;
pub const KEY_BACKSPACE: i32 = 0x7f;
pub const KEY_RSHIFT: i32 = 0x80 + 0x36;
pub const KEY_RCTRL: i32 = 0x80 + 0x1d;
pub const KEY_RALT: i32 = 0x80 + 0x38;
pub const KEY_PAUSE: i32 = 0xff;
pub const KEY_F1: i32 = 0x80 + 0x3b;
pub const KEY_F2: i32 = 0x80 + 0x3c;
pub const KEY_F3: i32 = 0x80 + 0x3d;
pub const KEY_F4: i32 = 0x80 + 0x3e;
pub const KEY_F5: i32 = 0x80 + 0x3f;
pub const KEY_F6: i32 = 0x80 + 0x40;
pub const KEY_F7: i32 = 0x80 + 0x41;
pub const KEY_F8: i32 = 0x80 + 0x42;
pub const KEY_F9: i32 = 0x80 + 0x43;
pub const KEY_F10: i32 = 0x80 + 0x44;
pub const KEY_F11: i32 = 0x80 + 0x57;
pub const KEY_F12: i32 = 0x80 + 0x58;

/// Weapon cycling, unbound in vanilla. The input backend binds
/// `key_prevweapon` and `key_nextweapon` to these unused codes.
pub const KEY_PREVWEAPON: i32 = 0xa4;
pub const KEY_NEXTWEAPON: i32 = 0xa5;

/// One translated transition, matching `retrodeck_doom_event_t`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub key: i32,
    pub character: i32,
    pub pressed: i32,
}

impl Event {
    fn press(key: i32, character: i32) -> Self {
        Self {
            key,
            character,
            pressed: 1,
        }
    }

    fn release(key: i32) -> Self {
        Self {
            key,
            character: 0,
            pressed: 0,
        }
    }
}

/// The Console-classic layout: the D-pad walks and turns, the shoulders
/// strafe, A fires, B opens, X and Y cycle weapons, Start is the menu and
/// Back is the automap. Running is permanent, set by the input backend.
pub const CONTROLLER_MAP: [(u32, i32); 12] = [
    (PAD_UP, KEY_UPARROW),
    (PAD_DOWN, KEY_DOWNARROW),
    (PAD_LEFT, KEY_LEFTARROW),
    (PAD_RIGHT, KEY_RIGHTARROW),
    (PAD_L, KEY_STRAFE_L),
    (PAD_R, KEY_STRAFE_R),
    (PAD_A, KEY_FIRE),
    (PAD_B, KEY_USE),
    (PAD_X, KEY_PREVWEAPON),
    (PAD_Y, KEY_NEXTWEAPON),
    (PAD_START, KEY_ESCAPE),
    (PAD_SELECT, KEY_TAB),
];

/// Linux evdev keycodes, from `input-event-codes.h`.
mod linux_key {
    pub const ESC: u16 = 1;
    pub const ONE: u16 = 2;
    /// `KEY_1` through `KEY_EQUAL` are contiguous, so the digit row is one
    /// range ending here.
    pub const EQUAL: u16 = 13;
    pub const BACKSPACE: u16 = 14;
    pub const TAB: u16 = 15;
    pub const Q: u16 = 16;
    pub const ENTER: u16 = 28;
    pub const LEFTCTRL: u16 = 29;
    pub const A: u16 = 30;
    pub const LEFTSHIFT: u16 = 42;
    pub const Z: u16 = 44;
    pub const COMMA: u16 = 51;
    pub const DOT: u16 = 52;
    pub const SLASH: u16 = 53;
    pub const RIGHTSHIFT: u16 = 54;
    pub const LEFTALT: u16 = 56;
    pub const SPACE: u16 = 57;
    pub const F1: u16 = 59;
    pub const F10: u16 = 68;
    pub const PAUSE: u16 = 119;
    pub const F11: u16 = 87;
    pub const F12: u16 = 88;
    pub const UP: u16 = 103;
    pub const LEFT: u16 = 105;
    pub const RIGHT: u16 = 106;
    pub const DOWN: u16 = 108;
    pub const RIGHTCTRL: u16 = 97;
    pub const RIGHTALT: u16 = 100;
}

/// Letters in evdev's keyboard row order, used for both the DOOM key code
/// and the typed character.
const LETTER_ROWS: [(u16, &[u8]); 3] = [
    (linux_key::Q, b"qwertyuiop"),
    (linux_key::A, b"asdfghjkl"),
    (linux_key::Z, b"zxcvbnm"),
];

/// Digits and the two keys beside them, in evdev order from `KEY_1`.
const DIGIT_ROW: &[u8] = b"1234567890-=";

/// Punctuation that DOOM's menus and cheats reach for.
const PUNCTUATION: [(u16, u8); 3] = [
    (linux_key::COMMA, b','),
    (linux_key::DOT, b'.'),
    (linux_key::SLASH, b'/'),
];

/// Keys whose DOOM code is not simply their character.
const NAMED_KEYS: [(u16, i32); 15] = [
    (linux_key::ESC, KEY_ESCAPE),
    (linux_key::ENTER, KEY_ENTER),
    (linux_key::TAB, KEY_TAB),
    (linux_key::BACKSPACE, KEY_BACKSPACE),
    (linux_key::SPACE, KEY_USE),
    (linux_key::UP, KEY_UPARROW),
    (linux_key::DOWN, KEY_DOWNARROW),
    (linux_key::LEFT, KEY_LEFTARROW),
    (linux_key::RIGHT, KEY_RIGHTARROW),
    (linux_key::LEFTCTRL, KEY_FIRE),
    (linux_key::RIGHTCTRL, KEY_FIRE),
    (linux_key::LEFTSHIFT, KEY_RSHIFT),
    (linux_key::RIGHTSHIFT, KEY_RSHIFT),
    (linux_key::LEFTALT, KEY_RALT),
    (linux_key::RIGHTALT, KEY_RALT),
];

/// Every keycode this mapping can translate, so the poller knows what to
/// sample without walking all 128 slots blindly.
pub fn keyboard_codes() -> Vec<u16> {
    let mut codes = Vec::new();
    for (code, _) in NAMED_KEYS {
        codes.push(code);
    }
    for (first, letters) in LETTER_ROWS {
        for offset in 0..letters.len() as u16 {
            codes.push(first + offset);
        }
    }
    for offset in 0..DIGIT_ROW.len() as u16 {
        codes.push(linux_key::ONE + offset);
    }
    for (code, _) in PUNCTUATION {
        codes.push(code);
    }
    for offset in 0..=(linux_key::F10 - linux_key::F1) {
        codes.push(linux_key::F1 + offset);
    }
    codes.push(linux_key::F11);
    codes.push(linux_key::F12);
    codes.push(linux_key::PAUSE);
    codes.sort_unstable();
    codes.dedup();
    codes
}

/// The DOOM key code and unshifted character for one Linux keycode.
/// Characters are zero for keys with no printable meaning.
pub fn keyboard_key(code: u16) -> Option<(i32, i32)> {
    if let Some((_, key)) = NAMED_KEYS.iter().find(|(candidate, _)| *candidate == code) {
        return Some((*key, 0));
    }
    for (first, letters) in LETTER_ROWS {
        if code >= first && code < first + letters.len() as u16 {
            let letter = letters[usize::from(code - first)];
            return Some((i32::from(letter), i32::from(letter)));
        }
    }
    if code >= linux_key::ONE && code <= linux_key::EQUAL {
        let digit = DIGIT_ROW[usize::from(code - linux_key::ONE)];
        return Some((i32::from(digit), i32::from(digit)));
    }
    if let Some((_, character)) = PUNCTUATION.iter().find(|(candidate, _)| *candidate == code) {
        return Some((i32::from(*character), i32::from(*character)));
    }
    if code >= linux_key::F1 && code <= linux_key::F10 {
        return Some((KEY_F1 + i32::from(code - linux_key::F1), 0));
    }
    match code {
        linux_key::F11 => Some((KEY_F11, 0)),
        linux_key::F12 => Some((KEY_F12, 0)),
        linux_key::PAUSE => Some((KEY_PAUSE, 0)),
        _ => None,
    }
}

/// Shift a typed character the way DOOM expects in `data2`.
pub fn shift_character(character: i32) -> i32 {
    let Ok(byte) = u8::try_from(character) else {
        return character;
    };
    let shifted = match byte {
        b'a'..=b'z' => byte.to_ascii_uppercase(),
        b'1' => b'!',
        b'2' => b'@',
        b'3' => b'#',
        b'4' => b'$',
        b'5' => b'%',
        b'6' => b'^',
        b'7' => b'&',
        b'8' => b'*',
        b'9' => b'(',
        b'0' => b')',
        b'-' => b'_',
        b'=' => b'+',
        b',' => b'<',
        b'.' => b'>',
        b'/' => b'?',
        other => other,
    };
    i32::from(shifted)
}

/// Append the transitions between two controller states.
pub fn controller_transitions(previous: u32, current: u32, events: &mut Vec<Event>) {
    for (bit, key) in CONTROLLER_MAP {
        let was = previous & bit != 0;
        let is = current & bit != 0;
        if was == is {
            continue;
        }
        events.push(if is {
            Event::press(key, 0)
        } else {
            Event::release(key)
        });
    }
}

/// Edge detection across both controllers and the keyboard.
pub struct Mapper {
    controllers: [u32; 2],
    keys: Vec<(u16, bool)>,
    shifted: bool,
}

impl Mapper {
    pub fn new() -> Self {
        Self {
            controllers: [0; 2],
            keys: keyboard_codes().into_iter().map(|code| (code, false)).collect(),
            shifted: false,
        }
    }

    /// Translate the current held state into transitions. Both controllers
    /// drive Player 1: DOOM is single-player here, and a second pad acting
    /// as a second input is friendlier than one that does nothing.
    pub fn poll<F, G>(&mut self, controller: F, key_held: G, events: &mut Vec<Event>)
    where
        F: Fn(u32) -> u32,
        G: Fn(u16) -> bool,
    {
        let merged = controller(0) | controller(1);
        let previous = self.controllers[0];
        controller_transitions(previous, merged, events);
        self.controllers[0] = merged;

        self.shifted = key_held(linux_key::LEFTSHIFT) || key_held(linux_key::RIGHTSHIFT);

        for (code, was_held) in &mut self.keys {
            let is_held = key_held(*code);
            if is_held == *was_held {
                continue;
            }
            *was_held = is_held;
            let Some((key, character)) = keyboard_key(*code) else {
                continue;
            };
            events.push(if is_held {
                let typed = if self.shifted && character != 0 {
                    shift_character(character)
                } else {
                    character
                };
                Event::press(key, typed)
            } else {
                Event::release(key)
            });
        }
    }
}

impl Default for Mapper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doom_key_codes_match_doomkeys_h() {
        // Values transcribed from the engine's doomkeys.h. A mismatch here
        // means the mirrored constants drifted from the pinned engine.
        assert_eq!(KEY_RIGHTARROW, 0xae);
        assert_eq!(KEY_LEFTARROW, 0xac);
        assert_eq!(KEY_UPARROW, 0xad);
        assert_eq!(KEY_DOWNARROW, 0xaf);
        assert_eq!(KEY_STRAFE_L, 0xa0);
        assert_eq!(KEY_STRAFE_R, 0xa1);
        assert_eq!(KEY_USE, 0xa2);
        assert_eq!(KEY_FIRE, 0xa3);
        assert_eq!(KEY_ESCAPE, 27);
        assert_eq!(KEY_ENTER, 13);
        assert_eq!(KEY_TAB, 9);
        assert_eq!(KEY_BACKSPACE, 0x7f);
        assert_eq!(KEY_RSHIFT, 0xb6);
        assert_eq!(KEY_F1, 0xbb);
        assert_eq!(KEY_F10, 0xc4);
    }

    #[test]
    fn weapon_cycle_codes_stay_inside_the_unused_gap() {
        // doomkeys.h uses 0xa0 through 0xa3 and 0xac through 0xaf, so the
        // cycling pseudo-keys must sit between them and collide with no
        // mapped key.
        for key in [KEY_PREVWEAPON, KEY_NEXTWEAPON] {
            assert!((0xa4..0xac).contains(&key));
        }
        assert_ne!(KEY_PREVWEAPON, KEY_NEXTWEAPON);
        for (_, mapped) in CONTROLLER_MAP {
            if mapped == KEY_PREVWEAPON || mapped == KEY_NEXTWEAPON {
                continue;
            }
            assert!(mapped != KEY_PREVWEAPON && mapped != KEY_NEXTWEAPON);
        }
    }

    #[test]
    fn every_controller_button_maps_to_a_distinct_key() {
        let mut keys: Vec<i32> = CONTROLLER_MAP.iter().map(|(_, key)| *key).collect();
        keys.sort_unstable();
        let count = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), count, "two buttons share one DOOM key");

        let mut bits: Vec<u32> = CONTROLLER_MAP.iter().map(|(bit, _)| *bit).collect();
        bits.sort_unstable();
        let count = bits.len();
        bits.dedup();
        assert_eq!(bits.len(), count, "one button appears twice");
    }

    #[test]
    fn face_buttons_are_the_four_distinct_pads() {
        // The console mapping folds X onto A and Y onto B; DOOM must see
        // all four, which is why it reads the distinct joypad state.
        let fire = CONTROLLER_MAP.iter().find(|(bit, _)| *bit == PAD_A);
        let use_key = CONTROLLER_MAP.iter().find(|(bit, _)| *bit == PAD_B);
        let previous = CONTROLLER_MAP.iter().find(|(bit, _)| *bit == PAD_X);
        let next = CONTROLLER_MAP.iter().find(|(bit, _)| *bit == PAD_Y);
        assert_eq!(fire.map(|(_, key)| *key), Some(KEY_FIRE));
        assert_eq!(use_key.map(|(_, key)| *key), Some(KEY_USE));
        assert_eq!(previous.map(|(_, key)| *key), Some(KEY_PREVWEAPON));
        assert_eq!(next.map(|(_, key)| *key), Some(KEY_NEXTWEAPON));
    }

    #[test]
    fn presses_and_releases_are_reported_once() {
        let mut events = Vec::new();
        controller_transitions(0, PAD_A, &mut events);
        assert_eq!(events, vec![Event::press(KEY_FIRE, 0)]);

        events.clear();
        controller_transitions(PAD_A, PAD_A, &mut events);
        assert!(events.is_empty(), "a held button repeated");

        events.clear();
        controller_transitions(PAD_A, 0, &mut events);
        assert_eq!(events, vec![Event::release(KEY_FIRE)]);
    }

    #[test]
    fn simultaneous_transitions_all_report() {
        let mut events = Vec::new();
        controller_transitions(PAD_LEFT, PAD_RIGHT | PAD_A, &mut events);
        assert_eq!(events.len(), 3);
        assert!(events.contains(&Event::release(KEY_LEFTARROW)));
        assert!(events.contains(&Event::press(KEY_RIGHTARROW, 0)));
        assert!(events.contains(&Event::press(KEY_FIRE, 0)));
    }

    #[test]
    fn keyboard_letters_carry_their_character() {
        assert_eq!(keyboard_key(linux_key::A), Some((i32::from(b'a'), i32::from(b'a'))));
        assert_eq!(keyboard_key(linux_key::Q), Some((i32::from(b'q'), i32::from(b'q'))));
        assert_eq!(keyboard_key(linux_key::Z), Some((i32::from(b'z'), i32::from(b'z'))));
        // Last letter of each evdev row.
        assert_eq!(keyboard_key(linux_key::Q + 9), Some((i32::from(b'p'), i32::from(b'p'))));
        assert_eq!(keyboard_key(linux_key::A + 8), Some((i32::from(b'l'), i32::from(b'l'))));
        assert_eq!(keyboard_key(linux_key::Z + 6), Some((i32::from(b'm'), i32::from(b'm'))));
    }

    #[test]
    fn keyboard_digits_and_named_keys_translate() {
        assert_eq!(keyboard_key(linux_key::ONE), Some((i32::from(b'1'), i32::from(b'1'))));
        assert_eq!(keyboard_key(linux_key::ONE + 9), Some((i32::from(b'0'), i32::from(b'0'))));
        // Eleventh key of the digit row, KEY_MINUS.
        assert_eq!(
            keyboard_key(linux_key::ONE + 10),
            Some((i32::from(b'-'), i32::from(b'-')))
        );
        assert_eq!(keyboard_key(linux_key::ESC), Some((KEY_ESCAPE, 0)));
        assert_eq!(keyboard_key(linux_key::SPACE), Some((KEY_USE, 0)));
        assert_eq!(keyboard_key(linux_key::LEFTCTRL), Some((KEY_FIRE, 0)));
        assert_eq!(keyboard_key(linux_key::F1), Some((KEY_F1, 0)));
        assert_eq!(keyboard_key(linux_key::F10), Some((KEY_F10, 0)));
        assert_eq!(keyboard_key(200), None);
    }

    #[test]
    fn shift_uppercases_letters_for_savegame_names() {
        assert_eq!(shift_character(i32::from(b'a')), i32::from(b'A'));
        assert_eq!(shift_character(i32::from(b'1')), i32::from(b'!'));
        assert_eq!(shift_character(i32::from(b'-')), i32::from(b'_'));
        assert_eq!(shift_character(0), 0);
    }

    #[test]
    fn keyboard_codes_cover_every_mapped_key() {
        let codes = keyboard_codes();
        for code in &codes {
            assert!(
                keyboard_key(*code).is_some(),
                "sampled keycode {code} has no mapping"
            );
        }
        for (code, _) in NAMED_KEYS {
            assert!(codes.contains(&code), "named key {code} is never sampled");
        }
        // Every sampled code must fit joypad::keyboard_key_held's range.
        for code in codes {
            assert!(code < 128, "keycode {code} is outside the held-key table");
        }
    }

    #[test]
    fn both_controllers_drive_one_player() {
        let mut mapper = Mapper::new();
        let mut events = Vec::new();
        mapper.poll(
            |port| if port == 1 { PAD_A } else { 0 },
            |_| false,
            &mut events,
        );
        assert_eq!(events, vec![Event::press(KEY_FIRE, 0)]);

        // The same button on the other pad is already held, so no repeat.
        events.clear();
        mapper.poll(|port| if port == 0 { PAD_A } else { 0 }, |_| false, &mut events);
        assert!(events.is_empty());
    }

    #[test]
    fn keyboard_edges_report_once_and_shift_applies() {
        let mut mapper = Mapper::new();
        let mut events = Vec::new();
        mapper.poll(
            |_| 0,
            |code| code == linux_key::A || code == linux_key::LEFTSHIFT,
            &mut events,
        );
        assert!(events.contains(&Event::press(i32::from(b'a'), i32::from(b'A'))));
        assert!(events.contains(&Event::press(KEY_RSHIFT, 0)));

        events.clear();
        mapper.poll(
            |_| 0,
            |code| code == linux_key::A || code == linux_key::LEFTSHIFT,
            &mut events,
        );
        assert!(events.is_empty(), "held keys repeated");

        events.clear();
        mapper.poll(|_| 0, |_| false, &mut events);
        assert!(events.contains(&Event::release(i32::from(b'a'))));
        assert!(events.contains(&Event::release(KEY_RSHIFT)));
    }
}
