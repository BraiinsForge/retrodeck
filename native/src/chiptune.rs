use std::ffi::{CStr, c_char, c_int, c_long, c_void};
use std::fs::OpenOptions;
use std::mem::size_of;
use std::os::fd::IntoRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::ptr::NonNull;
use std::sync::{Mutex, MutexGuard};

pub const FRAMES_PER_TICK: usize = 735;
const MAXIMUM_FILE_SIZE: u64 = 16 * 1024 * 1024;
// The pinned libvorbisfile 1.3.7 OggVorbis_File is 944 bytes on the host and
// 720 bytes on ARM. calloc supplies its required alignment; keep oversized
// private storage here rather than adding a first-party C shim.
const VORBIS_STATE_BYTES: usize = 8192;

static PLAYER: Mutex<Option<Player>> = Mutex::new(None);

#[repr(C)]
struct VorbisInfo {
    version: c_int,
    channels: c_int,
    rate: c_long,
}

#[repr(C)]
struct GmeInfo {
    length: c_int,
    intro_length: c_int,
    loop_length: c_int,
    play_length: c_int,
    fade_length: c_int,
    reserved_integers: [c_int; 11],
    system: *const c_char,
    game: *const c_char,
    song: *const c_char,
    author: *const c_char,
    copyright: *const c_char,
    comment: *const c_char,
    dumper: *const c_char,
    reserved_strings: [*const c_char; 9],
}

#[link(name = "gme")]
#[link(name = "stdc++")]
unsafe extern "C" {
    fn gme_open_data(
        data: *const c_void,
        size: c_long,
        out: *mut *mut c_void,
        sample_rate: c_int,
    ) -> *const c_char;
    fn gme_track_count(emulator: *const c_void) -> c_int;
    fn gme_start_track(emulator: *mut c_void, index: c_int) -> *const c_char;
    fn gme_track_info(
        emulator: *const c_void,
        out: *mut *mut GmeInfo,
        track: c_int,
    ) -> *const c_char;
    fn gme_free_info(info: *mut GmeInfo);
    fn gme_play(emulator: *mut c_void, count: c_int, out: *mut i16) -> *const c_char;
    fn gme_track_ended(emulator: *const c_void) -> c_int;
    fn gme_tell(emulator: *const c_void) -> c_int;
    fn gme_delete(emulator: *mut c_void);
}

#[link(name = "vorbisfile")]
#[link(name = "vorbis")]
#[link(name = "ogg")]
unsafe extern "C" {
    fn ov_open(
        file: *mut libc::FILE,
        state: *mut c_void,
        initial: *const c_char,
        initial_bytes: c_long,
    ) -> c_int;
    fn ov_clear(state: *mut c_void) -> c_int;
    fn ov_info(state: *mut c_void, link: c_int) -> *mut VorbisInfo;
    fn ov_comment(state: *mut c_void, link: c_int) -> *mut c_void;
    fn ov_time_total(state: *mut c_void, link: c_int) -> f64;
    fn ov_time_tell(state: *mut c_void) -> f64;
    fn ov_time_seek(state: *mut c_void, position: f64) -> c_int;
    fn ov_read(
        state: *mut c_void,
        buffer: *mut c_char,
        length: c_int,
        big_endian: c_int,
        word_size: c_int,
        signed: c_int,
        bitstream: *mut c_int,
    ) -> c_long;
    fn vorbis_comment_query(comment: *mut c_void, tag: *const c_char, count: c_int) -> *mut c_char;
}

pub struct OggPlayer {
    state: NonNull<c_void>,
    channels: usize,
    length_ms: i32,
    title: Vec<u8>,
    artist: Vec<u8>,
    samples: Vec<i16>,
}

// PLAYER serializes every access to the movable decoder handle.
unsafe impl Send for OggPlayer {}

pub struct DecodeBlock {
    pub ended: bool,
    pub frames: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Metadata {
    pub title: Vec<u8>,
    pub artist: Vec<u8>,
    pub author: Vec<u8>,
    pub system: Vec<u8>,
    pub length_ms: i32,
    pub track_count: i32,
    pub track_index: i32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub samples: Vec<i16>,
    pub ended: bool,
    pub frames: usize,
    pub position_ms: i32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Probe {
    pub tracks: usize,
    pub samples: usize,
    pub peak: i32,
}

impl OggPlayer {
    pub fn open(path: &Path) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(path)
            .map_err(|error| format!("cannot open chiptune: {error}"))?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("cannot inspect chiptune: {error}"))?;
        if !metadata.is_file() || !(1..=MAXIMUM_FILE_SIZE).contains(&metadata.len()) {
            return Err("chiptune must be a nonempty regular file up to 16 MiB".to_owned());
        }

        let descriptor = file.into_raw_fd();
        let stream = unsafe { libc::fdopen(descriptor, b"rb\0".as_ptr().cast()) };
        if stream.is_null() {
            unsafe { libc::close(descriptor) };
            return Err(format!(
                "cannot open chiptune stream: {}",
                std::io::Error::last_os_error()
            ));
        }
        let state = unsafe { libc::calloc(1, VORBIS_STATE_BYTES) };
        let Some(state) = NonNull::new(state) else {
            unsafe { libc::fclose(stream) };
            return Err("cannot allocate Ogg Vorbis state".to_owned());
        };
        if unsafe { ov_open(stream, state.as_ptr(), std::ptr::null(), 0) } != 0 {
            unsafe {
                libc::fclose(stream);
                libc::free(state.as_ptr());
            }
            return Err("cannot decode Ogg Vorbis file".to_owned());
        }

        let info = unsafe { ov_info(state.as_ptr(), -1).as_ref() };
        let Some(info) = info else {
            close_state(state);
            return Err("Ogg Vorbis stream has no format information".to_owned());
        };
        if info.rate != 44100 || !(info.channels == 1 || info.channels == 2) {
            close_state(state);
            return Err("Ogg Vorbis file must be 44.1 kHz mono or stereo".to_owned());
        }
        let duration = unsafe { ov_time_total(state.as_ptr(), -1) };
        let comments = unsafe { ov_comment(state.as_ptr(), -1) };
        Ok(Self {
            state,
            channels: info.channels as usize,
            length_ms: if duration >= 0.0 {
                (duration * 1000.0) as i32
            } else {
                -1
            },
            title: comment(comments, b"TITLE\0"),
            artist: comment(comments, b"ARTIST\0"),
            samples: vec![0; FRAMES_PER_TICK * 2],
        })
    }

    pub fn decode(&mut self) -> Result<DecodeBlock, String> {
        self.samples.fill(0);
        let mut mono = [0_i16; FRAMES_PER_TICK];
        let mut frames = 0;
        while frames < FRAMES_PER_TICK {
            let remaining = FRAMES_PER_TICK - frames;
            let destination = if self.channels == 2 {
                self.samples[frames * 2..].as_mut_ptr()
            } else {
                mono[frames..].as_mut_ptr()
            };
            let frame_bytes = self.channels * size_of::<i16>();
            let mut bitstream = 0;
            let amount = unsafe {
                ov_read(
                    self.state.as_ptr(),
                    destination.cast(),
                    (remaining * frame_bytes) as c_int,
                    0,
                    2,
                    1,
                    &mut bitstream,
                )
            };
            if amount < 0 {
                return Err("Ogg Vorbis stream is damaged".to_owned());
            }
            if amount == 0 {
                return Ok(DecodeBlock {
                    ended: true,
                    frames,
                });
            }
            if amount as usize % frame_bytes != 0 {
                return Err("Ogg Vorbis decoder returned a partial frame".to_owned());
            }
            let decoded = amount as usize / frame_bytes;
            if self.channels == 1 {
                for (index, sample) in mono[frames..frames + decoded].iter().enumerate() {
                    self.samples[(frames + index) * 2] = *sample;
                    self.samples[(frames + index) * 2 + 1] = *sample;
                }
            }
            frames += decoded;
        }
        Ok(DecodeBlock {
            ended: false,
            frames,
        })
    }

    pub fn rewind(&mut self) -> Result<(), String> {
        if unsafe { ov_time_seek(self.state.as_ptr(), 0.0) } == 0 {
            Ok(())
        } else {
            Err("cannot restart Ogg Vorbis track".to_owned())
        }
    }

    pub fn position_ms(&self) -> i32 {
        let seconds = unsafe { ov_time_tell(self.state.as_ptr()) };
        if seconds >= 0.0 {
            (seconds * 1000.0) as i32
        } else {
            0
        }
    }

    pub fn length_ms(&self) -> i32 {
        self.length_ms
    }

    pub fn title(&self) -> &[u8] {
        &self.title
    }

    pub fn artist(&self) -> &[u8] {
        &self.artist
    }

    pub fn samples(&self) -> &[i16] {
        &self.samples
    }
}

impl Drop for OggPlayer {
    fn drop(&mut self) {
        close_state(self.state);
    }
}

pub struct GmePlayer {
    emulator: NonNull<c_void>,
    info: *mut GmeInfo,
    track_index: i32,
    track_count: i32,
    samples: Vec<i16>,
}

// PLAYER serializes every access to the movable emulator handle.
unsafe impl Send for GmePlayer {}

impl GmePlayer {
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = read_chiptune_bytes(path)?;
        let mut emulator = std::ptr::null_mut();
        let result = unsafe {
            gme_open_data(
                bytes.as_ptr().cast(),
                bytes.len() as c_long,
                &mut emulator,
                44100,
            )
        };
        if !result.is_null() || emulator.is_null() {
            return Err(gme_error(result, "cannot create emulator"));
        }
        let mut player = Self {
            emulator: NonNull::new(emulator).unwrap(),
            info: std::ptr::null_mut(),
            track_index: 0,
            track_count: unsafe { gme_track_count(emulator) }.max(1),
            samples: vec![0; FRAMES_PER_TICK * 2],
        };
        player.start_track(0)?;
        Ok(player)
    }

    pub fn start_track(&mut self, track: i32) -> Result<(), String> {
        if !(0..self.track_count).contains(&track) {
            return Err(format!("chiptune has no track {track}"));
        }
        let result = unsafe { gme_start_track(self.emulator.as_ptr(), track) };
        if !result.is_null() {
            return Err(gme_error(result, "cannot start track"));
        }
        self.free_info();
        let mut info = std::ptr::null_mut();
        if !unsafe { gme_track_info(self.emulator.as_ptr(), &mut info, track) }.is_null() {
            info = std::ptr::null_mut();
        }
        self.info = info;
        self.track_index = track;
        Ok(())
    }

    pub fn decode(&mut self) -> Result<DecodeBlock, String> {
        self.samples.fill(0);
        let result = unsafe {
            gme_play(
                self.emulator.as_ptr(),
                self.samples.len() as c_int,
                self.samples.as_mut_ptr(),
            )
        };
        if !result.is_null() {
            return Err(gme_error(result, "cannot play track"));
        }
        Ok(DecodeBlock {
            ended: unsafe { gme_track_ended(self.emulator.as_ptr()) } != 0,
            frames: FRAMES_PER_TICK,
        })
    }

    pub fn position_ms(&self) -> i32 {
        unsafe { gme_tell(self.emulator.as_ptr()) }
    }

    pub fn metadata(&self) -> Metadata {
        let field = |value: *const c_char| {
            if value.is_null() {
                Vec::new()
            } else {
                unsafe { CStr::from_ptr(value) }.to_bytes().to_vec()
            }
        };
        let info = unsafe { self.info.as_ref() };
        Metadata {
            title: info.map_or_else(Vec::new, |info| field(info.song)),
            artist: info.map_or_else(Vec::new, |info| field(info.game)),
            author: info.map_or_else(Vec::new, |info| field(info.author)),
            system: info.map_or_else(Vec::new, |info| field(info.system)),
            length_ms: info.map_or(-1, |info| info.play_length),
            track_count: self.track_count,
            track_index: self.track_index,
        }
    }

    fn free_info(&mut self) {
        if !self.info.is_null() {
            unsafe { gme_free_info(self.info) };
            self.info = std::ptr::null_mut();
        }
    }
}

impl Drop for GmePlayer {
    fn drop(&mut self) {
        self.free_info();
        unsafe { gme_delete(self.emulator.as_ptr()) };
    }
}

pub enum Player {
    Ogg(OggPlayer),
    Gme(GmePlayer),
}

impl Player {
    pub fn open(path: &Path) -> Result<Self, String> {
        if has_ogg_extension(path) {
            Ok(Self::Ogg(OggPlayer::open(path)?))
        } else {
            Ok(Self::Gme(GmePlayer::open(path)?))
        }
    }

    pub fn decode(&mut self) -> Result<DecodeBlock, String> {
        match self {
            Self::Ogg(player) => player.decode(),
            Self::Gme(player) => player.decode(),
        }
    }

    pub fn rewind(&mut self) -> Result<(), String> {
        match self {
            Self::Ogg(player) => player.rewind(),
            Self::Gme(player) => player.start_track(player.track_index),
        }
    }

    pub fn start_track(&mut self, track: i32) -> Result<Metadata, String> {
        match self {
            Self::Ogg(_) => Err("Ogg Vorbis chiptunes have one track".to_owned()),
            Self::Gme(player) => {
                player.start_track(track)?;
                Ok(player.metadata())
            }
        }
    }

    pub fn position_ms(&self) -> i32 {
        match self {
            Self::Ogg(player) => player.position_ms(),
            Self::Gme(player) => player.position_ms(),
        }
    }

    pub fn samples(&self) -> &[i16] {
        match self {
            Self::Ogg(player) => player.samples(),
            Self::Gme(player) => &player.samples,
        }
    }

    pub fn metadata(&self) -> Metadata {
        match self {
            Self::Ogg(player) => Metadata {
                title: player.title().to_vec(),
                artist: player.artist().to_vec(),
                author: Vec::new(),
                system: Vec::new(),
                length_ms: player.length_ms(),
                track_count: 1,
                track_index: 0,
            },
            Self::Gme(player) => player.metadata(),
        }
    }
}

fn has_ogg_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ogg"))
}

fn gme_error(result: *const c_char, fallback: &str) -> String {
    if result.is_null() {
        fallback.to_owned()
    } else {
        unsafe { CStr::from_ptr(result) }
            .to_string_lossy()
            .into_owned()
    }
}

fn read_chiptune_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| format!("cannot open chiptune: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect chiptune: {error}"))?;
    if !metadata.is_file() || !(1..=MAXIMUM_FILE_SIZE).contains(&metadata.len()) {
        return Err("chiptune must be a nonempty regular file up to 16 MiB".to_owned());
    }
    let mut bytes = Vec::new();
    let mut file = file;
    std::io::Read::read_to_end(&mut file, &mut bytes)
        .map_err(|error| format!("cannot read chiptune: {error}"))?;
    Ok(bytes)
}

pub fn open(path: &Path) -> Result<Metadata, String> {
    *lock_player()? = None;
    let player = Player::open(path)?;
    let metadata = player.metadata();
    *lock_player()? = Some(player);
    Ok(metadata)
}

pub fn step() -> Result<Snapshot, String> {
    let mut guard = lock_player()?;
    let player = guard
        .as_mut()
        .ok_or_else(|| "no chiptune is open".to_owned())?;
    let block = player.decode()?;
    Ok(Snapshot {
        samples: player.samples().to_vec(),
        ended: block.ended,
        frames: block.frames,
        position_ms: player.position_ms(),
    })
}

pub fn rewind() -> Result<(), String> {
    lock_player()?
        .as_mut()
        .ok_or_else(|| "no chiptune is open".to_owned())?
        .rewind()
}

pub fn start_track(track: i32) -> Result<Metadata, String> {
    lock_player()?
        .as_mut()
        .ok_or_else(|| "no chiptune is open".to_owned())?
        .start_track(track)
}

pub fn close() -> Result<(), String> {
    *lock_player()? = None;
    Ok(())
}

fn lock_player() -> Result<MutexGuard<'static, Option<Player>>, String> {
    PLAYER
        .lock()
        .map_err(|_| "chiptune lock is poisoned".to_owned())
}

pub fn probe(path: &Path) -> Result<Probe, String> {
    match Player::open(path)? {
        Player::Ogg(mut player) => {
            let mut peak = 0;
            for _ in 0..60 {
                let block = player.decode()?;
                peak = peak.max(
                    player
                        .samples()
                        .iter()
                        .map(|sample| i32::from(*sample).abs())
                        .max()
                        .unwrap_or(0),
                );
                if block.ended {
                    player.rewind()?;
                }
            }
            Ok(Probe {
                tracks: 1,
                samples: 60 * FRAMES_PER_TICK * 2,
                peak,
            })
        }
        Player::Gme(player) => {
            let mut samples = vec![0_i16; 44100 * 2];
            let result = unsafe {
                gme_play(
                    player.emulator.as_ptr(),
                    samples.len() as c_int,
                    samples.as_mut_ptr(),
                )
            };
            if !result.is_null() {
                return Err(gme_error(result, "cannot play track"));
            }
            Ok(Probe {
                tracks: player.track_count as usize,
                samples: samples.len(),
                peak: samples
                    .iter()
                    .map(|sample| i32::from(*sample).abs())
                    .max()
                    .unwrap_or(0),
            })
        }
    }
}

fn comment(comments: *mut c_void, tag: &[u8]) -> Vec<u8> {
    if comments.is_null() {
        return Vec::new();
    }
    let value = unsafe { vorbis_comment_query(comments, tag.as_ptr().cast(), 0) };
    if value.is_null() {
        Vec::new()
    } else {
        unsafe { CStr::from_ptr(value) }.to_bytes().to_vec()
    }
}

fn close_state(state: NonNull<c_void>) {
    unsafe {
        ov_clear(state.as_ptr());
        libc::free(state.as_ptr());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../chiptunes/crazy.ogg")
    }

    fn nsf_fixture() -> std::path::PathBuf {
        let mut header = vec![0_u8; 128];
        header[..5].copy_from_slice(b"NESM\x1a");
        header[5] = 1;
        header[6] = 2;
        header[7] = 1;
        header[8..10].copy_from_slice(&0x8000_u16.to_le_bytes());
        header[10..12].copy_from_slice(&0x8000_u16.to_le_bytes());
        header[12..14].copy_from_slice(&0x8001_u16.to_le_bytes());
        header[14..21].copy_from_slice(b"SILENCE");
        header[46..55].copy_from_slice(b"RETRODECK");
        header[110..112].copy_from_slice(&16666_u16.to_le_bytes());
        header[120..122].copy_from_slice(&20000_u16.to_le_bytes());
        header.extend_from_slice(&[0x60, 0x60]);
        let path = crate::test_support::fixture_directory("chiptune-nsf").join("silence.nsf");
        std::fs::write(&path, header).unwrap();
        path
    }

    #[test]
    fn decodes_gme_tracks_through_the_maintained_library() {
        let path = nsf_fixture();
        let mut player = match Player::open(&path).unwrap() {
            Player::Gme(player) => Player::Gme(player),
            Player::Ogg(_) => panic!("NSF must use the game-music-emu backend"),
        };
        let metadata = player.metadata();
        assert_eq!(
            (metadata.artist, metadata.author, metadata.system),
            (
                b"SILENCE".to_vec(),
                b"RETRODECK".to_vec(),
                b"Nintendo NES".to_vec()
            )
        );
        assert_eq!(
            (metadata.length_ms, metadata.track_count, metadata.track_index),
            (150000, 2, 0)
        );
        let block = player.decode().unwrap();
        assert_eq!((block.frames, player.samples().len()), (FRAMES_PER_TICK, 1470));
        assert!(player.position_ms() > 0);
        assert_eq!(player.start_track(1).unwrap().track_index, 1);
        assert!(player.start_track(2).is_err());
        player.rewind().unwrap();
        assert_eq!(player.position_ms(), 0);
    }

    #[test]
    fn matches_cpp_gme_probe() {
        assert_eq!(
            probe(&nsf_fixture()).unwrap(),
            Probe {
                tracks: 2,
                samples: 88200,
                peak: 0,
            }
        );
    }

    #[test]
    fn routes_only_ogg_extensions_to_vorbis() {
        assert!(has_ogg_extension(Path::new("/tmp/song.OGG")));
        assert!(!has_ogg_extension(Path::new("/tmp/song.nsf")));
        assert!(open(Path::new("/tmp/retrodeck-missing.spc")).is_err());
    }

    #[test]
    fn matches_cpp_ogg_probe() {
        assert_eq!(
            probe(&fixture()).unwrap(),
            Probe {
                tracks: 1,
                samples: 88200,
                peak: 4432,
            }
        );
    }

    #[test]
    fn decodes_exact_blocks_and_rewinds() {
        let mut player = OggPlayer::open(&fixture()).unwrap();
        assert!(player.length_ms() >= 50000);
        assert!(player.title().is_empty());
        assert!(player.artist().is_empty());
        for _ in 0..4 {
            let block = player.decode().unwrap();
            assert_eq!((block.ended, block.frames), (false, FRAMES_PER_TICK));
        }
        assert!(player.position_ms() > 0);
        player.rewind().unwrap();
        assert_eq!(player.position_ms(), 0);
    }

    #[test]
    fn owns_one_decoder_through_the_global_boundary() {
        close().unwrap();
        let metadata = open(&fixture()).unwrap();
        assert_eq!((metadata.title, metadata.artist), (Vec::new(), Vec::new()));
        assert!(metadata.length_ms >= 50000);
        let snapshot = step().unwrap();
        assert_eq!((snapshot.ended, snapshot.frames), (false, FRAMES_PER_TICK));
        assert_eq!(snapshot.samples.len(), FRAMES_PER_TICK * 2);
        assert!(snapshot.position_ms > 0);
        rewind().unwrap();
        assert_eq!(step().unwrap().position_ms, snapshot.position_ms);
        assert!(open(Path::new("/no/such/retrodeck-chiptune.ogg")).is_err());
        assert!(step().is_err());
        close().unwrap();
    }
}
