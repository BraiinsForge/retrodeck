use crate::{audio, canvas, controls, fbdev, input, polling, process, wayland};
use std::ffi::OsStr;

const CENTISECOND_NS: u64 = 10_000_000;
const REDRAW_NS: u64 = 33_000_000;
const LIMIT: u16 = 9999;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Mode {
    #[default]
    Ready,
    Running,
    Stopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Tick,
    Touch,
    ControllerA,
    Back,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Cue {
    Start,
    Exact,
    Miss,
}

impl Cue {
    fn tones(self) -> &'static [audio::Tone] {
        match self {
            Self::Start => &[
                audio::Tone {
                    frequency: 523,
                    duration_ms: 28,
                },
                audio::Tone {
                    frequency: 784,
                    duration_ms: 38,
                },
            ],
            Self::Exact => &[
                audio::Tone {
                    frequency: 784,
                    duration_ms: 35,
                },
                audio::Tone {
                    frequency: 1047,
                    duration_ms: 40,
                },
                audio::Tone {
                    frequency: 1319,
                    duration_ms: 55,
                },
            ],
            Self::Miss => &[
                audio::Tone {
                    frequency: 659,
                    duration_ms: 35,
                },
                audio::Tone {
                    frequency: 440,
                    duration_ms: 55,
                },
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct State {
    mode: Mode,
    displayed: u16,
    started_at: u64,
    redraw_at: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Effect {
    redraw: bool,
    cue: Option<Cue>,
    result: Option<(u16, Event)>,
    exit: bool,
}

fn format_time(centiseconds: u16) -> String {
    let shown = centiseconds.min(LIMIT);
    format!("{:02}.{:02}", shown / 100, shown % 100)
}

impl State {
    fn elapsed(self, now: u64) -> u16 {
        (now.saturating_sub(self.started_at) / CENTISECOND_NS).min(u64::from(LIMIT)) as u16
    }

    fn apply(&mut self, event: Event, now: u64) -> Effect {
        match event {
            Event::Back => Effect {
                exit: true,
                ..Effect::default()
            },
            Event::Tick if self.mode == Mode::Running && now >= self.redraw_at => {
                self.displayed = self.elapsed(now);
                self.redraw_at = now.saturating_add(REDRAW_NS);
                Effect {
                    redraw: true,
                    ..Effect::default()
                }
            }
            Event::Tick => Effect::default(),
            Event::Touch | Event::ControllerA if self.mode == Mode::Running => {
                self.displayed = self.elapsed(now);
                self.mode = Mode::Stopped;
                self.redraw_at = 0;
                Effect {
                    redraw: true,
                    cue: Some(if self.displayed == 1000 {
                        Cue::Exact
                    } else {
                        Cue::Miss
                    }),
                    result: Some((self.displayed, event)),
                    exit: false,
                }
            }
            Event::Touch | Event::ControllerA => {
                *self = Self {
                    mode: Mode::Running,
                    displayed: 0,
                    started_at: now,
                    redraw_at: now,
                };
                Effect {
                    redraw: true,
                    cue: Some(Cue::Start),
                    ..Effect::default()
                }
            }
        }
    }
}

fn back(x: i32, y: i32) -> bool {
    (16..=167).contains(&x) && (16..=79).contains(&y)
}

fn input_events(reports: &[controls::ControlReport], touches: &[input::TouchReport]) -> Vec<Event> {
    if touches
        .iter()
        .any(|touch| touch.pressed && back(touch.x, touch.y))
    {
        vec![Event::Back]
    } else if reports
        .iter()
        .any(|report| report.kind == controls::GAMEPAD_REPORT && report.value & 0x004 != 0)
    {
        vec![Event::ControllerA]
    } else {
        touches
            .iter()
            .filter(|touch| touch.pressed)
            .map(|_| Event::Touch)
            .collect()
    }
}

fn source_rect(x: i32, y: i32, width: u32, height: u32, color: u32) {
    canvas::fill_rect(16 + 2 * x, 16 + 2 * y, 2 * width, 2 * height, color);
}

fn source_text(x: i32, y: i32, text: &str, scale: u32, color: u32) {
    for (index, character) in text.bytes().enumerate() {
        canvas::draw_glyph(
            16 + 2 * x + index as i32 * 12 * scale as i32,
            16 + 2 * y,
            character,
            2 * scale,
            color,
        );
    }
}

fn centered_text(y: i32, text: &str, scale: u32, color: u32) {
    let width = (text.len() as i32 * 6 - 1) * scale as i32;
    source_text(((624 - width) / 2).max(0), y, text, scale, color);
}

fn digit(x: i32, character: u8, active: u32) {
    let mask = [
        0x3f_u8, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f,
    ][usize::from(character - b'0')];
    for (index, (dx, dy, width, height)) in [
        (11, 0, 54, 11),
        (65, 11, 11, 53),
        (65, 64, 11, 53),
        (11, 117, 54, 11),
        (0, 64, 11, 53),
        (0, 11, 11, 53),
        (11, 59, 54, 11),
    ]
    .into_iter()
    .enumerate()
    {
        source_rect(
            x + dx,
            43 + dy,
            width,
            height,
            if mask & (1 << index) != 0 {
                active
            } else {
                0x1c1c1c
            },
        );
    }
}

fn render(state: State) {
    let cream = 0xffedc2;
    let success = 0x62d38c;
    let exact = state.mode == Mode::Stopped && state.displayed == 1000;
    let color = if exact { success } else { 0xff7138 };
    let shown = format_time(state.displayed);
    canvas::clear(0);
    source_rect(0, 0, 624, 224, 0x100d0c);
    source_rect(6, 5, 70, 25, 0x29211e);
    source_text(15, 11, "BACK", 2, cream);
    centered_text(9, "STOP AT 10.00", 2, cream);
    for (x, index) in [(129, 0), (219, 1), (329, 3), (419, 4)] {
        digit(x, shown.as_bytes()[index], color);
    }
    source_rect(303, 149, 14, 14, color);
    if state.mode == Mode::Stopped {
        let result = if exact {
            "EXACT".to_owned()
        } else if state.displayed < 1000 {
            format!("{} EARLY", format_time(1000 - state.displayed))
        } else {
            format!("{} LATE", format_time(state.displayed - 1000))
        };
        centered_text(178, &result, 1, if exact { success } else { 0xaa8f7c });
    }
    centered_text(
        198,
        match state.mode {
            Mode::Ready => "TAP OR A TO START",
            Mode::Running => "TAP OR A TO STOP",
            Mode::Stopped => "TAP OR A FOR ANOTHER TRY",
        },
        2,
        cream,
    );
}

fn volume(text: Option<&OsStr>) -> Option<i32> {
    match text {
        None => Some(42),
        Some(text) => {
            let text = text.to_str()?;
            if text.is_empty() {
                return None;
            }
            text.bytes().try_fold(0_i32, |value, digit| {
                digit.is_ascii_digit().then_some(())?;
                let value = value * 10 + i32::from(digit - b'0');
                (value <= 100).then_some(value)
            })
        }
    }
}

struct Session {
    widget: bool,
    audio_owned: bool,
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.audio_owned {
            audio::stop();
        }
        controls::close();
        input::close_touch();
        if self.widget {
            wayland::close();
        } else {
            fbdev::close();
        }
    }
}

pub fn run() -> Result<(), String> {
    process::install_signal_handlers()?;
    let widget = std::env::var_os("RETRO_DECK_PRESENTATION").is_some_and(|value| value == "widget")
        && std::env::var_os("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty());
    let mut session = Session {
        widget,
        audio_owned: false,
    };
    if !widget {
        fbdev::open()?;
    }
    input::open_touch()?;
    let gamepads = match controls::scan_gamepads() {
        Ok(count) => count,
        Err(error) => {
            eprintln!("ten-seconds-deck: controller input unavailable: {error}");
            0
        }
    };
    eprintln!(
        "ten-seconds-deck: {gamepads} THEGamepad controller(s) ready; physical A starts and stops the timer"
    );
    let volume =
        volume(std::env::var_os("RETRO_DECK_VOLUME_PERCENT").as_deref()).unwrap_or_else(|| {
            eprintln!(
                "ten-seconds-deck: volume must be an integer from 0 through 100; game cues disabled"
            );
            0
        });
    let mut state = State::default();
    let mut dirty = true;
    while !process::shutdown_requested() {
        session.audio_owned = audio::active();
        dirty |= state
            .apply(Event::Tick, polling::monotonic_nanoseconds())
            .redraw;
        if dirty {
            render(state);
            if widget && wayland::size().is_none() {
                wayland::open_game_widget()?;
            }
            canvas::with_pixels(|pixels| {
                if widget {
                    wayland::present_rgba(pixels)
                } else {
                    fbdev::present_rgba(pixels)
                }
            })?;
            dirty = false;
        }
        if widget {
            wayland::dispatch(0)?;
            while wayland::next_touch().is_some() {}
            if wayland::shutdown_requested() {
                break;
            }
        }
        let poll = polling::dispatch(false, 8)?;
        let reports: Vec<_> = (0..poll.control_count)
            .map(|_| {
                controls::next_report().ok_or_else(|| "controller queue ended early".to_owned())
            })
            .collect::<Result<_, _>>()?;
        let touches: Vec<_> = (0..poll.touch_count)
            .map(|_| input::next_touch().ok_or_else(|| "touch queue ended early".to_owned()))
            .collect::<Result<_, _>>()?;
        if poll.touch_lost {
            return Err("10 Seconds touchscreen disconnected".to_owned());
        }
        if poll.shutdown {
            break;
        }
        for event in input_events(&reports, &touches) {
            let effect = state.apply(event, polling::monotonic_nanoseconds());
            if effect.exit {
                return Ok(());
            }
            if let Some((displayed, event)) = effect.result {
                let source = if event == Event::ControllerA {
                    "controller-a"
                } else {
                    "touch"
                };
                eprintln!(
                    "ten-seconds-deck: result={} input={source}",
                    format_time(displayed)
                );
            }
            if let Some(cue) = effect.cue.filter(|_| volume > 0) {
                match audio::play_tone_sequence(cue.tones(), volume) {
                    Ok(audio::PlayOutcome::Started) => session.audio_owned = true,
                    Ok(audio::PlayOutcome::Busy) => {}
                    Err(error) => eprintln!("ten-seconds-deck: cue unavailable: {error}"),
                }
            }
            dirty |= effect.redraw;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_stop_retry_and_centi_second_boundaries() {
        for input in [Event::Touch, Event::ControllerA] {
            for (elapsed, cue) in [
                (999, Cue::Miss),
                (1000, Cue::Exact),
                (1003, Cue::Miss),
                (10000, Cue::Miss),
            ] {
                let mut state = State::default();
                assert_eq!(state.apply(input, 100).cue, Some(Cue::Start));
                let effect = state.apply(input, 100 + elapsed * CENTISECOND_NS);
                assert_eq!(state.displayed, elapsed.min(u64::from(LIMIT)) as u16);
                assert_eq!(effect.cue, Some(cue));
                assert_eq!(effect.result, Some((state.displayed, input)));
                assert_eq!(state.mode, Mode::Stopped);
                assert_eq!(
                    state.apply(input, 200 + elapsed * CENTISECOND_NS).cue,
                    Some(Cue::Start)
                );
                assert_eq!(state.displayed, 0);
            }
        }
        assert_eq!(format_time(0), "00.00");
        assert_eq!(format_time(123), "01.23");
        assert_eq!(format_time(10000), "99.99");
    }

    #[test]
    fn redraw_clock_and_backwards_clock() {
        let mut state = State::default();
        assert_eq!(state.apply(Event::Tick, 1), Effect::default());
        state.apply(Event::Touch, 100);
        assert!(state.apply(Event::Tick, 100).redraw);
        let deadline = state.redraw_at;
        assert!(!state.apply(Event::Tick, deadline - 1).redraw);
        assert!(state.apply(Event::Tick, deadline).redraw);
        assert_eq!(state.apply(Event::Touch, 0).result, Some((0, Event::Touch)));
        let before = state;
        assert!(state.apply(Event::Back, 0).exit);
        assert_eq!(state, before);
    }

    #[test]
    fn back_then_controller_then_touch_priority() {
        let a = controls::ControlReport {
            kind: controls::GAMEPAD_REPORT,
            value: 4,
            flags: 0,
        };
        let touch = input::TouchReport {
            x: 300,
            y: 200,
            pressed: true,
            down: true,
            released: false,
        };
        assert_eq!(input_events(&[a, a], &[touch]), [Event::ControllerA]);
        assert_eq!(
            input_events(&[], &[touch, touch]),
            [Event::Touch, Event::Touch]
        );
        for (x, y) in [(16, 16), (167, 79)] {
            assert_eq!(
                input_events(&[a], &[input::TouchReport { x, y, ..touch }]),
                [Event::Back]
            );
        }
        for (x, y) in [(15, 16), (168, 79), (16, 15), (167, 80)] {
            assert_eq!(
                input_events(&[], &[input::TouchReport { x, y, ..touch }]),
                [Event::Touch]
            );
        }
        assert!(
            input_events(
                &[],
                &[input::TouchReport {
                    pressed: false,
                    ..touch
                }]
            )
            .is_empty()
        );
    }

    #[test]
    fn inherited_volume_and_cue_sequences() {
        assert_eq!(volume(None), Some(42));
        for (text, expected) in [
            ("0", Some(0)),
            ("042", Some(42)),
            ("100", Some(100)),
            ("", None),
            ("101", None),
            ("-1", None),
            (" 42", None),
            ("42\n", None),
        ] {
            assert_eq!(volume(Some(OsStr::new(text))), expected);
        }
        for (cue, expected) in [
            (Cue::Start, vec![(523, 28), (784, 38)]),
            (Cue::Exact, vec![(784, 35), (1047, 40), (1319, 55)]),
            (Cue::Miss, vec![(659, 35), (440, 55)]),
        ] {
            assert_eq!(
                cue.tones()
                    .iter()
                    .map(|tone| (tone.frequency, tone.duration_ms))
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn frames_match_lisp_reference() {
        for (mode, displayed, expected) in [
            (Mode::Ready, 0, 0x562ba370a3b4da05),
            (Mode::Running, 123, 0x802225e33aea53b5),
            (Mode::Stopped, 1000, 0x171e4a78d40d458d),
            (Mode::Stopped, 1003, 0x0171d3938f410bd5),
            (Mode::Stopped, 9999, 0xd8877f2ba172c415),
        ] {
            render(State {
                mode,
                displayed,
                ..State::default()
            });
            assert_eq!(canvas::rgb565_hash(), expected, "{mode:?}/{displayed}");
        }
    }
}
