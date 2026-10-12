//! Standalone chiptune player, using the shared GME/Vorbis and OSS mechanisms.
mod render;

use crate::{
    audio, canvas, chiptune, controls, fbdev, input, polling, process, regular_file, state_file,
    wayland,
};
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

const BLOCK_NS: u64 = 16_666_666;
const RENDER_NS: u64 = 50_000_000;
const MAXIMUM_FILES: usize = 1024;
const EXTENSIONS: &[&[u8]] = &[
    b"ay", b"gbs", b"gym", b"hes", b"kss", b"nsf", b"nsfe", b"ogg", b"sap", b"spc", b"vgm", b"vgz",
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Mode {
    #[default]
    LoopAll,
    LoopOne,
    Shuffle,
}
impl Mode {
    fn next(self) -> Self {
        match self {
            Self::LoopAll => Self::LoopOne,
            Self::LoopOne => Self::Shuffle,
            Self::Shuffle => Self::LoopAll,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Back,
    PreviousFile,
    NextFile,
    PreviousTrack,
    NextTrack,
    Pause,
    Mode,
    VolumeUp,
    VolumeDown,
}

struct Metadata {
    title: String,
    subtitle: String,
    system: String,
    length_ms: i32,
    track_index: i32,
    track_count: i32,
}

impl Metadata {
    fn compose(path: &Path, metadata: chiptune::Metadata) -> Self {
        let title = if metadata.title.is_empty() {
            base_name(path)
        } else {
            String::from_utf8_lossy(&metadata.title).into_owned()
        };
        let artist = String::from_utf8_lossy(&metadata.artist);
        let author = String::from_utf8_lossy(&metadata.author);
        let subtitle = if !artist.is_empty() && !author.is_empty() {
            format!("{artist} - {author}")
        } else {
            format!("{artist}{author}")
        };
        Self {
            title,
            subtitle,
            system: if is_ogg(path) {
                "OGG VORBIS".to_owned()
            } else {
                String::from_utf8_lossy(&metadata.system).into_owned()
            },
            length_ms: metadata.length_ms,
            track_index: metadata.track_index,
            track_count: metadata.track_count,
        }
    }
}

struct PlayerState {
    files: Vec<PathBuf>,
    file_index: usize,
    player: Option<chiptune::Player>,
    metadata: Option<Metadata>,
    mode: Mode,
    random: u32,
    paused: bool,
    visual: Vec<i16>,
    position: i32,
    pending: Option<Vec<u8>>,
    status: String,
}

impl PlayerState {
    fn new(files: Vec<PathBuf>, random: u32, status: String) -> Self {
        Self {
            files,
            file_index: 0,
            player: None,
            metadata: None,
            mode: Mode::LoopAll,
            random,
            paused: false,
            visual: Vec::new(),
            position: 0,
            pending: None,
            status,
        }
    }

    fn note_metadata(&mut self, metadata: chiptune::Metadata) {
        self.metadata = Some(Metadata::compose(&self.files[self.file_index], metadata));
        self.paused = false;
        self.visual.clear();
        self.position = 0;
        self.pending = None;
    }

    fn open_index(&mut self, index: usize) -> bool {
        match chiptune::Player::open(&self.files[index]) {
            Ok(player) => {
                let metadata = player.metadata();
                self.file_index = index;
                self.player = Some(player);
                self.note_metadata(metadata);
                true
            }
            Err(error) => {
                eprintln!(
                    "chiptune-deck: cannot open {}: {error}",
                    self.files[index].display()
                );
                false
            }
        }
    }

    fn open_any(&mut self, indices: impl IntoIterator<Item = usize>) -> bool {
        indices.into_iter().any(|index| self.open_index(index))
    }

    fn navigate(&mut self, direction: i32) -> bool {
        if self.files.is_empty() {
            return false;
        }
        self.open_any(file_candidates(
            self.files.len(),
            self.file_index,
            direction,
        ))
    }

    fn start_track(&mut self, track: i32) -> bool {
        if let Some(Ok(metadata)) = self.player.as_mut().map(|player| player.start_track(track)) {
            self.note_metadata(metadata);
            true
        } else {
            false
        }
    }

    fn switch_track(&mut self, direction: i32) -> bool {
        let Some(metadata) = &self.metadata else {
            return false;
        };
        if is_ogg(&self.files[self.file_index]) {
            return false;
        }
        self.start_track((metadata.track_index + direction).rem_euclid(metadata.track_count))
    }

    fn advance(&mut self) {
        let Some(metadata) = &self.metadata else {
            return;
        };
        let opened = match self.mode {
            Mode::LoopOne => self
                .player
                .as_mut()
                .is_some_and(|player| player.rewind().is_ok()),
            Mode::LoopAll if metadata.track_index + 1 < metadata.track_count => {
                self.start_track(metadata.track_index + 1)
            }
            Mode::LoopAll => self.navigate(1),
            Mode::Shuffle => {
                self.random = next_random(self.random);
                let candidates = shuffle_candidates(self.files.len(), self.file_index, self.random);
                let opened = self.open_any(candidates) || self.open_index(self.file_index);
                if opened && self.metadata.as_ref().unwrap().track_count > 1 {
                    self.random = next_random(self.random);
                    self.start_track(
                        (self.random % self.metadata.as_ref().unwrap().track_count as u32) as i32,
                    )
                } else {
                    opened
                }
            }
        };
        if opened {
            if self.mode == Mode::LoopOne {
                self.position = 0;
            }
        } else {
            self.fail_playback("cannot advance to the next track");
        }
    }

    fn fail_playback(&mut self, reason: &str) {
        self.player = None;
        self.metadata = None;
        self.pending = None;
        self.visual.clear();
        self.status = format!("PLAYBACK ERROR: {reason}");
    }
}

fn display_text(input: &str, maximum: usize) -> String {
    let mut clean: String = input
        .chars()
        .filter_map(|character| match character {
            'a'..='z' => Some(character.to_ascii_uppercase()),
            'A'..='Z' | '0'..='9' | ' ' | '.' | ':' | '-' | '+' | '/' => Some(character),
            '_' | '\t' => Some(' '),
            _ => None,
        })
        .collect();
    if clean.len() > maximum {
        clean.truncate(if maximum > 3 { maximum - 3 } else { maximum });
        if maximum > 3 {
            clean.push_str("...");
        }
    }
    clean
}

fn base_name(path: &Path) -> String {
    let name = path.file_name().unwrap_or_default().as_bytes();
    let stem = &name[..name
        .iter()
        .rposition(|byte| *byte == b'.')
        .unwrap_or(name.len())];
    String::from_utf8_lossy(stem).replace(['_', '-'], " ")
}

fn format_time(milliseconds: i32) -> String {
    let seconds = milliseconds.max(0) / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn is_ogg(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.as_bytes().eq_ignore_ascii_case(b"ogg"))
}

fn file_candidates(count: usize, current: usize, direction: i32) -> Vec<usize> {
    (1..=count)
        .map(|attempt| {
            if direction < 0 {
                (current + count - attempt % count) % count
            } else {
                (current + attempt) % count
            }
        })
        .collect()
}

fn next_random(mut value: u32) -> u32 {
    if value == 0 {
        value = 0x6d2b79f5;
    }
    value ^= value << 13;
    value ^= value >> 17;
    value ^ (value << 5)
}

fn shuffle_candidates(count: usize, current: usize, random: u32) -> Vec<usize> {
    let offset = if count > 1 {
        1 + random as usize % (count - 1)
    } else {
        0
    };
    (0..count)
        .map(|attempt| (current + offset + attempt) % count)
        .filter(|candidate| count == 1 || *candidate != current)
        .collect()
}

fn scan_files(directory: &Path) -> Vec<PathBuf> {
    fn walk(directory: &Path, depth: usize, files: &mut Vec<PathBuf>) {
        if depth > 4 || files.len() >= MAXIMUM_FILES {
            return;
        }
        let mut entries = regular_file::list_directory(directory).unwrap_or_else(|error| {
            eprintln!("chiptune-deck: {error}");
            Vec::new()
        });
        entries.retain(|entry| !entry.name.is_empty() && entry.name[0] != b'.');
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        for entry in entries {
            if files.len() == MAXIMUM_FILES {
                break;
            }
            let path = directory.join(OsString::from_vec(entry.name));
            if entry.kind == regular_file::ENTRY_DIRECTORY {
                walk(&path, depth + 1, files);
            } else if entry.kind == regular_file::ENTRY_FILE
                && (1..=16 * 1024 * 1024).contains(&entry.size)
                && path.extension().is_some_and(|extension| {
                    EXTENSIONS
                        .iter()
                        .any(|accepted| extension.as_bytes().eq_ignore_ascii_case(accepted))
                })
            {
                files.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(directory, 0, &mut files);
    files.sort();
    files
}

fn volume(text: Option<&OsStr>) -> Option<i32> {
    match text {
        None => Some(42),
        Some(text) if !text.is_empty() => text.as_bytes().iter().try_fold(0_i32, |value, byte| {
            byte.is_ascii_digit().then_some(())?;
            let value = value * 10 + i32::from(byte - b'0');
            (value <= 100).then_some(value)
        }),
        _ => None,
    }
}

fn touch_command(x: i32, y: i32) -> Option<Command> {
    let (x, y) = ((x - 16) / 2, (y - 16) / 2);
    for (command, left, top, width, height) in [
        (Command::Back, 554, 3, 62, 34),
        (Command::PreviousFile, 215, 177, 92, 34),
        (Command::Pause, 317, 177, 92, 34),
        (Command::NextFile, 419, 177, 92, 34),
        (Command::Mode, 113, 177, 92, 34),
    ] {
        if x >= left && x < left + width && y >= top && y < top + height {
            return Some(command);
        }
    }
    None
}

fn report_commands(report: controls::ControlReport) -> Vec<Command> {
    if report.kind != controls::GAMEPAD_REPORT {
        return Vec::new();
    }
    let mut commands = Vec::new();
    for (mask, command) in [
        (1, Command::Back),
        (2, Command::Back),
        (4, Command::Pause),
        (8, Command::Pause),
        (16, Command::PreviousTrack),
        (32, Command::NextTrack),
        (128, Command::Mode),
        (256, Command::PreviousFile),
        (512, Command::NextFile),
        (1024, Command::VolumeUp),
        (2048, Command::VolumeDown),
    ] {
        if report.value & mask != 0 && !commands.contains(&command) {
            commands.push(command);
        }
    }
    commands
}

struct Session {
    widget: bool,
    io_owned: bool,
    audio_owned: bool,
    volume: i32,
    volume_state: Option<PathBuf>,
    next_block: u64,
    next_render: u64,
    dirty: bool,
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.audio_owned {
            if let Err(error) = audio::close_chiptune_pcm() {
                eprintln!("chiptune-deck: {error}");
            }
        }
        if !self.io_owned {
            return;
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

impl Session {
    fn set_volume(&mut self, requested: i32) {
        match audio::open_chiptune_pcm(requested) {
            Ok(()) => {
                self.volume = requested;
                self.audio_owned = true;
                self.dirty = true;
                if let Some(path) = &self.volume_state {
                    if let Err(error) = state_file::write(path, format!("{requested}\n").as_bytes())
                    {
                        eprintln!("chiptune-deck: cannot save volume state: {error}");
                    }
                }
            }
            Err(error) => eprintln!("chiptune-deck: cannot change volume: {error}"),
        }
    }

    fn flush(&self, state: &mut PlayerState, pcm: Vec<u8>) -> bool {
        if self.audio_owned && self.volume > 0 {
            match audio::write_chiptune_pcm(&pcm) {
                Ok(audio::PcmWriteOutcome::Queued) => {}
                Ok(audio::PcmWriteOutcome::Busy) => {
                    state.pending = Some(pcm);
                    return false;
                }
                Err(error) => {
                    eprintln!("chiptune-deck: {error}");
                    state.pending = Some(pcm);
                    return false;
                }
            }
        }
        state.pending = None;
        true
    }

    fn generate(&mut self, state: &mut PlayerState, now: u64) -> bool {
        if state.metadata.is_none() || state.paused {
            return false;
        }
        if let Some(pcm) = state.pending.take() {
            return self.flush(state, pcm);
        }
        if !(self.audio_owned && self.volume > 0) && now < self.next_block {
            return false;
        }
        let player = state.player.as_mut().expect("metadata has an open player");
        let block = match player.decode() {
            Ok(block) => block,
            Err(error) => {
                eprintln!("chiptune-deck: {error}");
                state.fail_playback("native playback step failed");
                self.dirty = true;
                return false;
            }
        };
        state.visual = player.samples().to_vec();
        state.position = player.position_ms().max(0);
        let pcm = state
            .visual
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        self.flush(state, pcm);
        if block.ended {
            state.advance();
        }
        self.next_block = self.next_block.saturating_add(BLOCK_NS).max(now);
        true
    }

    fn apply(&mut self, state: &mut PlayerState, commands: &[Command]) -> bool {
        if commands.contains(&Command::PreviousFile) && state.navigate(-1) {
            self.dirty = true;
        }
        if commands.contains(&Command::NextFile) && state.navigate(1) {
            self.dirty = true;
        }
        if commands.contains(&Command::PreviousTrack) && state.switch_track(-1) {
            self.dirty = true;
        }
        if commands.contains(&Command::NextTrack) && state.switch_track(1) {
            self.dirty = true;
        }
        if commands.contains(&Command::Pause) {
            state.paused = !state.paused;
            self.dirty = true;
        }
        if commands.contains(&Command::Mode) {
            state.mode = state.mode.next();
            self.dirty = true;
        }
        if commands.contains(&Command::VolumeUp) || commands.contains(&Command::VolumeDown) {
            let requested = if commands.contains(&Command::VolumeUp) {
                (self.volume + 5).min(100)
            } else {
                (self.volume - 5).max(0)
            };
            if requested != self.volume {
                self.set_volume(requested);
            }
        }
        commands.contains(&Command::Back)
    }

    fn present(&mut self) -> Result<(), String> {
        if self.widget && wayland::size().is_none() {
            wayland::open_game_widget()?;
        }
        canvas::with_pixels(|pixels| {
            if self.widget {
                wayland::present_rgba(pixels)
            } else {
                fbdev::present_rgba(pixels)
            }
        })
    }
}

pub fn run(directory: &Path) -> Result<(), String> {
    process::install_signal_handlers()?;
    let widget = std::env::var_os("RETRO_DECK_PRESENTATION").is_some_and(|value| value == "widget")
        && std::env::var_os("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty());
    let volume =
        volume(std::env::var_os("RETRO_DECK_VOLUME_PERCENT").as_deref()).unwrap_or_else(|| {
            eprintln!("chiptune-deck: volume must be an integer from 0 through 100; playing muted");
            0
        });
    let mut session = Session {
        widget,
        io_owned: true,
        audio_owned: false,
        volume,
        volume_state: std::env::var_os("RETRO_DECK_VOLUME_STATE")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from),
        next_block: 0,
        next_render: 0,
        dirty: true,
    };
    if !widget {
        fbdev::open()?;
    }
    input::open_touch()?;
    let gamepads = controls::scan_gamepads().unwrap_or_else(|error| {
        eprintln!("chiptune-deck: controller input unavailable: {error}");
        0
    });
    eprintln!("chiptune-deck: {gamepads} THEGamepad controller(s) ready");
    let files = scan_files(directory);
    eprintln!(
        "chiptune-deck: found {} supported file(s) in {}",
        files.len(),
        directory.display()
    );
    match audio::open_chiptune_pcm(volume) {
        Ok(()) => session.audio_owned = true,
        Err(error) => eprintln!("chiptune-deck: cannot open audio; continuing muted: {error}"),
    }
    let now = polling::monotonic_nanoseconds();
    let mut state = PlayerState::new(
        files,
        (0x9e3779b9_u64 ^ now) as u32,
        format!("ADD MUSIC TO {}", directory.display()),
    );
    state.random ^= state.files.len() as u32;
    if !state.files.is_empty() && !state.open_any(0..state.files.len()) {
        state.status = "CANNOT PLAY FILES".to_owned();
    }
    session.next_block = polling::monotonic_nanoseconds();
    session.next_render = session.next_block;
    while !process::shutdown_requested() {
        let now = polling::monotonic_nanoseconds();
        let decoded = session.generate(&mut state, now);
        if session.dirty {
            render::full(&state, session.volume);
            session.present()?;
            session.dirty = false;
            session.next_render = now.saturating_add(RENDER_NS);
        } else if decoded && state.metadata.is_some() && now >= session.next_render {
            render::playback(&state);
            session.present()?;
            session.next_render = now.saturating_add(RENDER_NS);
        }
        if widget {
            wayland::dispatch(0)?;
            while wayland::next_touch().is_some() {}
            if wayland::shutdown_requested() {
                break;
            }
        }
        let timeout = if decoded && state.pending.is_none() {
            0
        } else {
            8
        };
        let poll = polling::dispatch(false, timeout)?;
        let mut commands = Vec::new();
        for _ in 0..poll.control_count {
            let report = controls::next_report()
                .ok_or_else(|| "Chiptune control queue ended early".to_owned())?;
            for command in report_commands(report) {
                if !commands.contains(&command) {
                    commands.push(command);
                }
            }
        }
        for _ in 0..poll.touch_count {
            let touch =
                input::next_touch().ok_or_else(|| "Chiptune touch queue ended early".to_owned())?;
            if let Some(command) = touch
                .pressed
                .then(|| touch_command(touch.x, touch.y))
                .flatten()
            {
                if !commands.contains(&command) {
                    commands.push(command);
                }
            }
        }
        if poll.touch_lost {
            return Err("Chiptune touchscreen disconnected".to_owned());
        }
        if poll.shutdown || session.apply(&mut state, &commands) {
            break;
        }
    }
    if session.audio_owned {
        audio::close_chiptune_pcm()?;
        session.audio_owned = false;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_and_shuffle_match_player_policy() {
        assert_eq!(display_text("\tFoo_bar-+/:. !ä", 100), " FOO BAR-+/:. ");
        assert_eq!(display_text("ABCDEFGHI", 7), "ABCD...");
        assert_eq!(format_time(59_999), "0:59");
        assert_eq!(format_time(-1), "0:00");
        assert_eq!(next_random(1), 270369);
        assert_eq!(file_candidates(3, 0, -1), [2, 1, 0]);
        assert_eq!(file_candidates(3, 2, 1), [0, 1, 2]);
        assert_eq!(shuffle_candidates(3, 1, 0), [2, 0]);
        assert_eq!(Mode::LoopAll.next().next().next(), Mode::LoopAll);
    }

    #[test]
    fn input_edges_touch_boundaries_and_volume() {
        let report = controls::ControlReport {
            kind: controls::GAMEPAD_REPORT,
            value: 4095,
            flags: 0,
        };
        assert_eq!(report_commands(report).len(), 9);
        assert_eq!(touch_command(1124, 22), Some(Command::Back));
        assert_eq!(touch_command(1248, 22), None);
        assert_eq!(touch_command(650, 370), Some(Command::Pause));
        assert_eq!(volume(None), Some(42));
        assert_eq!(volume(Some(OsStr::new("000100"))), Some(100));
        for value in ["", "101", "-1", "1 ", "99999999999999999"] {
            assert_eq!(volume(Some(OsStr::new(value))), None);
        }
    }

    #[test]
    fn scan_limits_hidden_files_and_symlinks() {
        let directory = crate::test_support::fixture_directory("chiptune-scan");
        std::fs::write(directory.join("b.NSF"), b"x").unwrap();
        std::fs::write(directory.join("a.ogg"), b"x").unwrap();
        std::fs::write(directory.join(".hidden.spc"), b"x").unwrap();
        std::fs::write(directory.join("empty.gbs"), b"").unwrap();
        std::fs::write(directory.join("other.mp3"), b"x").unwrap();
        let oversized = std::fs::File::create(directory.join("large.vgm")).unwrap();
        oversized.set_len(16 * 1024 * 1024 + 1).unwrap();
        std::os::unix::fs::symlink("a.ogg", directory.join("link.ogg")).unwrap();
        let mut nested = directory.clone();
        for depth in 1..=5 {
            nested.push("nested");
            std::fs::create_dir(&nested).unwrap();
            std::fs::write(nested.join(format!("depth-{depth}.spc")), b"x").unwrap();
        }
        let files = scan_files(&directory);
        assert_eq!(files.len(), 6);
        assert_eq!(files[0], directory.join("a.ogg"));
        assert_eq!(files[1], directory.join("b.NSF"));
        assert!(files.last().unwrap().ends_with("depth-4.spc"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn real_ogg_navigation_and_muted_pacing() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../chiptunes/crazy.ogg");
        let mut state = PlayerState::new(vec![path], 1, String::new());
        assert!(state.open_index(0));
        assert_eq!(state.metadata.as_ref().unwrap().title, "crazy");
        assert_eq!(state.metadata.as_ref().unwrap().length_ms, 50851);
        assert!(!state.switch_track(1));
        let mut session = Session {
            widget: false,
            io_owned: false,
            audio_owned: false,
            volume: 0,
            volume_state: None,
            next_block: 100,
            next_render: 0,
            dirty: false,
        };
        assert!(!session.generate(&mut state, 99));
        assert!(session.generate(&mut state, 100));
        assert_eq!(session.next_block, 100 + BLOCK_NS);
        assert!(state.position > 0 && state.pending.is_none());
        assert!(!session.generate(&mut state, 101));
        state.mode = Mode::LoopOne;
        state.advance();
        assert_eq!(state.position, 0);
        assert!(session.apply(&mut state, &[Command::Back, Command::Pause, Command::Mode]));
        assert!(state.paused);
        assert_eq!(state.mode, Mode::Shuffle);
    }
}
