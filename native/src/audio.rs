use std::ffi::{c_char, c_int, c_ulong, c_void};
use std::io;
use std::mem;
use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

const AUDIO_DEVICE: &[u8] = b"/dev/dsp\0";
const O_WRONLY: c_int = 1;
const O_CLOEXEC: c_int = 0o2000000;
const SIG_HUP: c_int = 1;
const SIG_INT: c_int = 2;
const SIG_TERM: c_int = 15;
const SIG_DFL: usize = 0;
const WNOHANG: c_int = 1;
const ECHILD: c_int = 10;

const AFMT_S16_LE: c_int = 0x10;
const SNDCTL_DSP_SYNC: c_ulong = 0x5001;
const SNDCTL_DSP_SPEED: c_ulong = 0xc0045002;
const SNDCTL_DSP_SETFMT: c_ulong = 0xc0045005;
const SNDCTL_DSP_CHANNELS: c_ulong = 0xc0045006;
const SNDCTL_DSP_SETFRAGMENT: c_ulong = 0xc004500a;
const SNDCTL_DSP_GETOSPACE: c_ulong = 0x8010500c;
const SNDCTL_DSP_SETTRIGGER: c_ulong = 0x40045010;
const PCM_ENABLE_OUTPUT: c_int = 0x2;
const CHIPTUNE_PCM_BYTES: usize = 735 * 2 * mem::size_of::<i16>();
// Six blocks (~100 ms) plus the ~93 ms prefilled kernel buffer absorb the
// worst decode-loop stalls while keeping pause/volume/track controls snappy;
// the Lisp pump keeps this queue full and is paced by its backpressure.
const CHIPTUNE_QUEUE_BLOCKS: usize = 6;

#[repr(C)]
#[derive(Default)]
struct AudioBufferInfo {
    fragments: c_int,
    fragment_total: c_int,
    fragment_size: c_int,
    bytes: c_int,
}

#[derive(Clone, Copy)]
pub struct Tone {
    pub frequency: c_int,
    pub duration_ms: c_int,
}

pub enum PlayOutcome {
    Started,
    Busy,
}

pub enum PcmWriteOutcome {
    Queued,
    Busy,
}

struct Player {
    child_pid: c_int,
}

struct PcmPlayer {
    sender: Option<SyncSender<Vec<u8>>>,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}

static PLAYER: Mutex<Player> = Mutex::new(Player { child_pid: -1 });
static PCM_PLAYER: Mutex<Option<PcmPlayer>> = Mutex::new(None);

unsafe extern "C" {
    fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn write(fd: c_int, data: *const c_void, size: usize) -> isize;
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    fn fork() -> c_int;
    fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
    fn kill(pid: c_int, signal: c_int) -> c_int;
    fn signal(signal: c_int, handler: usize) -> usize;
    fn _exit(status: c_int) -> !;
}

pub fn play_tones(
    first_frequency: c_int,
    first_duration_ms: c_int,
    second_frequency: c_int,
    second_duration_ms: c_int,
    volume_percent: c_int,
) -> Result<PlayOutcome, String> {
    let (tones, count) = tone_sequence(
        first_frequency,
        first_duration_ms,
        second_frequency,
        second_duration_ms,
    )?;
    play_tone_sequence(&tones[..count], volume_percent)
}

pub fn play_tone_sequence(tones: &[Tone], volume_percent: c_int) -> Result<PlayOutcome, String> {
    validate(tones, 44100, volume_percent)?;
    let mut player = player();
    player.reap_finished();
    if player.child_pid > 0 {
        return Ok(PlayOutcome::Busy);
    }

    let child = unsafe { fork() };
    if child < 0 {
        return Err(os_error("cannot start menu sound worker"));
    }
    if child == 0 {
        unsafe {
            signal(SIG_TERM, SIG_DFL);
            signal(SIG_INT, SIG_DFL);
            signal(SIG_HUP, SIG_DFL);
        }
        let result = play_blocking(tones, volume_percent);
        if let Err(error) = result.as_ref() {
            eprintln!("retrodeck: {error}");
        }
        unsafe { _exit(if result.is_ok() { 0 } else { 1 }) };
    }

    player.child_pid = child;
    Ok(PlayOutcome::Started)
}

pub fn active() -> bool {
    let mut player = player();
    player.reap_finished();
    player.child_pid > 0
}

pub fn stop() {
    player().stop();
}

pub fn finish() {
    player().finish();
}

pub fn open_chiptune_pcm(volume_percent: c_int) -> Result<(), String> {
    close_chiptune_pcm()?;
    if !(0..=100).contains(&volume_percent) {
        return Err("chiptune volume must be between 0 and 100".to_owned());
    }
    if volume_percent == 0 {
        *pcm_player() = Some(PcmPlayer {
            sender: None,
            worker: None,
            stop: Arc::new(AtomicBool::new(false)),
        });
        return Ok(());
    }

    let fd = unsafe { open(AUDIO_DEVICE.as_ptr().cast(), O_WRONLY | O_CLOEXEC) };
    if fd < 0 {
        return Err(os_error("cannot open /dev/dsp for chiptune audio"));
    }
    let trigger_pending = match configure_chiptune_pcm(fd) {
        Ok(trigger_pending) => trigger_pending,
        Err(error) => {
            unsafe { close(fd) };
            return Err(error);
        }
    };
    let (sender, receiver) = sync_channel(CHIPTUNE_QUEUE_BLOCKS);
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let worker = match thread::Builder::new()
        .name("retrodeck-chiptune-pcm".to_owned())
        .spawn(move || {
            chiptune_pcm_worker(fd, receiver, volume_percent, trigger_pending, worker_stop)
        }) {
        Ok(worker) => worker,
        Err(error) => {
            unsafe { close(fd) };
            return Err(format!("cannot start chiptune audio worker: {error}"));
        }
    };
    *pcm_player() = Some(PcmPlayer {
        sender: Some(sender),
        worker: Some(worker),
        stop,
    });
    Ok(())
}

pub fn write_chiptune_pcm(pcm: &[u8]) -> Result<PcmWriteOutcome, String> {
    if pcm.len() != CHIPTUNE_PCM_BYTES {
        return Err(format!(
            "chiptune PCM must contain {CHIPTUNE_PCM_BYTES} bytes"
        ));
    }
    let player = pcm_player();
    let Some(sender) = player.as_ref().and_then(|player| player.sender.as_ref()) else {
        return Ok(PcmWriteOutcome::Busy);
    };
    match sender.try_send(pcm.to_vec()) {
        Ok(()) => Ok(PcmWriteOutcome::Queued),
        Err(TrySendError::Full(_)) => Ok(PcmWriteOutcome::Busy),
        Err(TrySendError::Disconnected(_)) => {
            Err("chiptune audio worker is unavailable".to_owned())
        }
    }
}

pub fn close_chiptune_pcm() -> Result<(), String> {
    let Some(mut player) = pcm_player().take() else {
        return Ok(());
    };
    player.stop.store(true, Ordering::Release);
    drop(player.sender.take());
    if let Some(worker) = player.worker.take() {
        worker
            .join()
            .map_err(|_| "chiptune audio worker failed".to_owned())?;
    }
    Ok(())
}

impl Player {
    fn reap_finished(&mut self) {
        if self.child_pid <= 0 {
            return;
        }
        loop {
            let mut status = 0;
            let result = unsafe { waitpid(self.child_pid, &mut status, WNOHANG) };
            if result == 0 {
                return;
            }
            if result == self.child_pid {
                if !status_succeeded(status) {
                    eprintln!("retrodeck: menu sound worker failed");
                }
                self.child_pid = -1;
                return;
            }
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if error.raw_os_error() != Some(ECHILD) {
                eprintln!("retrodeck: cannot reap menu sound worker: {error}");
            }
            self.child_pid = -1;
            return;
        }
    }

    fn stop(&mut self) {
        if self.child_pid > 0 {
            unsafe { kill(self.child_pid, SIG_TERM) };
            self.wait(false);
        }
    }

    fn finish(&mut self) {
        if self.child_pid > 0 {
            self.wait(true);
        }
    }

    fn wait(&mut self, report_failure: bool) {
        loop {
            let mut status = 0;
            let result = unsafe { waitpid(self.child_pid, &mut status, 0) };
            if result == self.child_pid {
                if report_failure && !status_succeeded(status) {
                    eprintln!("retrodeck: menu sound worker failed");
                }
                break;
            }
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if error.raw_os_error() != Some(ECHILD) {
                eprintln!("retrodeck: cannot finish menu sound worker: {error}");
            }
            break;
        }
        self.child_pid = -1;
    }
}

fn player() -> MutexGuard<'static, Player> {
    PLAYER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn pcm_player() -> MutexGuard<'static, Option<PcmPlayer>> {
    PCM_PLAYER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn configure_chiptune_pcm(fd: c_int) -> Result<bool, String> {
    let mut fragment: c_int = (8 << 16) | 10;
    let mut format = AFMT_S16_LE;
    let mut channels: c_int = 1;
    let mut rate: c_int = 44100;
    unsafe {
        ioctl(fd, SNDCTL_DSP_SETFRAGMENT, &mut fragment as *mut c_int);
    }
    let configured = unsafe {
        ioctl(fd, SNDCTL_DSP_SETFMT, &mut format as *mut c_int) == 0
            && format == AFMT_S16_LE
            && ioctl(fd, SNDCTL_DSP_CHANNELS, &mut channels as *mut c_int) == 0
            && channels == 1
            && ioctl(fd, SNDCTL_DSP_SPEED, &mut rate as *mut c_int) == 0
            && rate == 44100
    };
    if !configured {
        return Err(os_error("cannot configure /dev/dsp for chiptune audio"));
    }

    let mut trigger = 0;
    let trigger_pending =
        unsafe { ioctl(fd, SNDCTL_DSP_SETTRIGGER, &mut trigger as *mut c_int) == 0 };
    let mut space = AudioBufferInfo::default();
    if unsafe { ioctl(fd, SNDCTL_DSP_GETOSPACE, &mut space as *mut AudioBufferInfo) } == 0
        && space.bytes > 0
        && space.bytes <= 1024 * 1024
        && space.bytes % 2 == 0
    {
        write_all(fd, &vec![0; space.bytes as usize])
            .map_err(|error| format!("cannot prefill /dev/dsp: {error}"))?;
    }
    Ok(trigger_pending)
}

fn chiptune_pcm_worker(
    fd: c_int,
    receiver: Receiver<Vec<u8>>,
    volume_percent: c_int,
    mut trigger_pending: bool,
    stop: Arc<AtomicBool>,
) {
    let result = (|| {
        for pcm in receiver {
            if stop.load(Ordering::Acquire) {
                break;
            }
            if trigger_pending {
                let mut trigger = PCM_ENABLE_OUTPUT;
                if unsafe { ioctl(fd, SNDCTL_DSP_SETTRIGGER, &mut trigger as *mut c_int) } != 0 {
                    return Err(os_error("cannot start chiptune audio playback"));
                }
                trigger_pending = false;
            }
            let mono = render_chiptune_pcm_mono(&pcm, volume_percent)?;
            let bytes = unsafe {
                slice::from_raw_parts(
                    mono.as_ptr().cast::<u8>(),
                    mono.len() * mem::size_of::<i16>(),
                )
            };
            write_all(fd, bytes)
                .map_err(|error| format!("cannot write chiptune audio: {error}"))?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("retrodeck: {error}");
    }
    if unsafe { close(fd) } != 0 {
        eprintln!(
            "retrodeck: cannot close chiptune audio: {}",
            io::Error::last_os_error()
        );
    }
}

fn render_chiptune_pcm_mono(pcm: &[u8], volume_percent: c_int) -> Result<Vec<i16>, String> {
    if pcm.len() != CHIPTUNE_PCM_BYTES || !(1..=100).contains(&volume_percent) {
        return Err("invalid chiptune PCM block or volume".to_owned());
    }
    Ok(pcm
        .chunks_exact(4)
        .map(|frame| {
            let left = i16::from_le_bytes([frame[0], frame[1]]) as i32;
            let right = i16::from_le_bytes([frame[2], frame[3]]) as i32;
            (((left + right) / 2) * volume_percent / 100) as i16
        })
        .collect())
}

fn tone_sequence(
    first_frequency: c_int,
    first_duration_ms: c_int,
    second_frequency: c_int,
    second_duration_ms: c_int,
) -> Result<([Tone; 2], usize), String> {
    let tones = [
        Tone {
            frequency: first_frequency,
            duration_ms: first_duration_ms,
        },
        Tone {
            frequency: second_frequency,
            duration_ms: second_duration_ms,
        },
    ];
    let count = if second_frequency == 0 && second_duration_ms == 0 {
        1
    } else if second_frequency > 0 && second_duration_ms > 0 {
        2
    } else {
        return Err("the optional second tone needs frequency and duration".to_owned());
    };
    Ok((tones, count))
}

fn validate(tones: &[Tone], rate: c_int, volume_percent: c_int) -> Result<(), String> {
    if !(1..=3).contains(&tones.len()) {
        return Err("menu sounds need one through three tones".to_owned());
    }
    if !(1..=100).contains(&volume_percent) {
        return Err("menu sound volume must be between 1 and 100".to_owned());
    }
    if rate <= 0 {
        return Err("menu sound sample rate must be positive".to_owned());
    }
    if tones
        .iter()
        .any(|tone| tone.frequency <= 0 || tone.duration_ms <= 0)
    {
        return Err("menu tones need positive frequency and duration".to_owned());
    }
    Ok(())
}

fn render_tones(tones: &[Tone], rate: c_int, volume_percent: c_int) -> Result<Vec<i16>, String> {
    validate(tones, rate, volume_percent)?;
    let rate = rate as usize;
    let amplitude = (5000 * volume_percent / 100).max(256);
    let ramp_samples = (rate / 200).max(1);
    let mut samples = Vec::new();

    for tone in tones {
        let note_samples = rate
            .checked_mul(tone.duration_ms as usize)
            .ok_or_else(|| "menu tone duration is too large".to_owned())?
            / 1000;
        let note_samples = note_samples.max(1);
        samples
            .try_reserve(note_samples)
            .map_err(|_| "cannot allocate menu tone samples".to_owned())?;
        let period = ((rate as c_int) / tone.frequency).max(2) as usize;
        for index in 0..note_samples {
            let mut sample = if index % period < period / 2 {
                amplitude
            } else {
                -amplitude
            };
            let remaining = note_samples - index;
            let envelope = ramp_samples.min((index + 1).min(remaining));
            sample = (sample as i64 * envelope as i64 / ramp_samples as i64) as c_int;
            samples.push(sample as i16);
        }
    }
    Ok(samples)
}

fn play_blocking(tones: &[Tone], volume_percent: c_int) -> Result<(), String> {
    let fd = unsafe { open(AUDIO_DEVICE.as_ptr().cast(), O_WRONLY | O_CLOEXEC) };
    if fd < 0 {
        return Err(os_error("cannot open /dev/dsp for menu sound"));
    }

    let result = (|| {
        let mut fragment: c_int = (4 << 16) | 9;
        let mut format = AFMT_S16_LE;
        let mut channels: c_int = 1;
        let mut rate: c_int = 44100;
        unsafe {
            ioctl(fd, SNDCTL_DSP_SETFRAGMENT, &mut fragment as *mut c_int);
        }
        let configured = unsafe {
            ioctl(fd, SNDCTL_DSP_SETFMT, &mut format as *mut c_int) == 0
                && format == AFMT_S16_LE
                && ioctl(fd, SNDCTL_DSP_CHANNELS, &mut channels as *mut c_int) == 0
                && channels == 1
                && ioctl(fd, SNDCTL_DSP_SPEED, &mut rate as *mut c_int) == 0
                && rate > 0
        };
        if !configured {
            return Err(os_error("cannot configure menu sound"));
        }

        let samples = render_tones(tones, rate, volume_percent)?;
        let bytes = unsafe {
            slice::from_raw_parts(
                samples.as_ptr().cast::<u8>(),
                samples.len() * mem::size_of::<i16>(),
            )
        };
        write_all(fd, bytes).map_err(|error| format!("cannot play menu sound: {error}"))?;
        unsafe { ioctl(fd, SNDCTL_DSP_SYNC, 0usize) };
        Ok(())
    })();

    let close_result = unsafe { close(fd) };
    if result.is_ok() && close_result != 0 {
        return Err(os_error("cannot play menu sound"));
    }
    result
}

fn write_all(fd: c_int, mut bytes: &[u8]) -> io::Result<()> {
    while !bytes.is_empty() {
        let written = unsafe { write(fd, bytes.as_ptr().cast(), bytes.len()) };
        if written > 0 {
            bytes = &bytes[written as usize..];
        } else if written == 0 {
            return Err(io::ErrorKind::WriteZero.into());
        } else {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
    Ok(())
}

fn os_error(action: &str) -> String {
    format!("{action}: {}", io::Error::last_os_error())
}

fn status_succeeded(status: c_int) -> bool {
    status & 0x7f == 0 && (status >> 8) & 0xff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fnv1a;

    fn digest(samples: &[i16]) -> u64 {
        fnv1a(samples.iter().flat_map(|sample| sample.to_le_bytes()))
    }

    fn tone(frequency: i32, duration_ms: i32) -> Tone {
        Tone {
            frequency,
            duration_ms,
        }
    }

    #[test]
    fn matches_the_legacy_menu_waveforms() {
        let cases = [
            (&[(660, 60), (880, 60)][..], 5292, 0xecf6ba7ff22d0dc2),
            (&[(523, 35)][..], 1543, 0xdfdd2a5aba3f4a04),
            (&[(659, 35)][..], 1543, 0xab6adca9dc7484b9),
            (&[(659, 25), (880, 30)][..], 2425, 0x633b4308002d1688),
            (&[(659, 25), (440, 30)][..], 2425, 0xfe15242926ff4036),
            (
                &[(784, 35), (1047, 40), (1319, 55)][..],
                5732,
                0xb572c4d4420310d4,
            ),
        ];
        for (notes, expected_length, expected_digest) in cases {
            let tones = notes
                .iter()
                .map(|&(frequency, duration_ms)| tone(frequency, duration_ms))
                .collect::<Vec<_>>();
            let samples = render_tones(&tones, 44100, 42).unwrap();
            assert_eq!(
                (samples.len(), digest(&samples)),
                (expected_length, expected_digest)
            );
        }
    }

    #[test]
    fn rejects_invalid_tone_requests() {
        assert!(tone_sequence(659, 25, 0, 30).is_err());
        for (tones, volume) in [
            (&[tone(659, 25); 4][..], 42),
            (&[tone(0, 25)][..], 42),
            (&[tone(659, 25)][..], 0),
        ] {
            assert!(render_tones(tones, 44100, volume).is_err());
        }
    }

    #[test]
    fn mixes_and_scales_exact_chiptune_blocks() {
        let mut pcm = vec![0; CHIPTUNE_PCM_BYTES];
        for (index, (left, right)) in [
            (-32768_i16, -32768_i16),
            (32767, 32767),
            (-32768, 32767),
            (1000, -2000),
        ]
        .into_iter()
        .enumerate()
        {
            let offset = index * 4;
            pcm[offset..offset + 2].copy_from_slice(&left.to_le_bytes());
            pcm[offset + 2..offset + 4].copy_from_slice(&right.to_le_bytes());
        }
        let mono = render_chiptune_pcm_mono(&pcm, 42).unwrap();
        assert_eq!(mono.len(), 735);
        assert_eq!(&mono[..4], &[-13762, 13762, 0, -210]);
        assert!(mono[4..].iter().all(|sample| *sample == 0));
        assert!(render_chiptune_pcm_mono(&pcm[..4], 42).is_err());
    }
}
