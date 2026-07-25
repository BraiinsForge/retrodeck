use std::ffi::{CStr, c_char, c_int, c_long, c_void};
use std::fs::OpenOptions;
use std::mem::size_of;
use std::os::fd::IntoRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::ptr::NonNull;

pub const FRAMES_PER_TICK: usize = 735;
const MAXIMUM_FILE_SIZE: u64 = 16 * 1024 * 1024;
// The pinned libvorbisfile 1.3.7 OggVorbis_File is 944 bytes on the host and
// 720 bytes on ARM. calloc supplies its required alignment; keep oversized
// private storage here rather than adding a first-party C shim.
const VORBIS_STATE_BYTES: usize = 8192;

#[repr(C)]
struct VorbisInfo {
    version: c_int,
    channels: c_int,
    rate: c_long,
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

pub struct DecodeBlock {
    pub ended: bool,
    pub frames: usize,
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

pub fn probe(path: &Path) -> Result<Probe, String> {
    let mut player = OggPlayer::open(path)?;
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
}
