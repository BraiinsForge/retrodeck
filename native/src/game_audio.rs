//! Emulator audio: mono OSS output with the C++ frontend's queue, gain, and
//! block-local linear resampler. The OSS write blocks only the writer thread.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::{Condvar, Mutex, OnceLock};

const AUDIO_DEVICE: &std::ffi::CStr = c"/dev/dsp";
const QUEUE_FRAMES: usize = 16384;
// Start the queue with a silent cushion so a late emulation window drains
// slack instead of underrunning the device audibly.
const CUSHION_FRAMES: usize = 1024;
const WRITE_CHUNK_FRAMES: usize = 2048;

const SNDCTL_DSP_SETFMT: libc::c_ulong = 0xc0045005;
const SNDCTL_DSP_CHANNELS: libc::c_ulong = 0xc0045006;
const SNDCTL_DSP_SPEED: libc::c_ulong = 0xc0045002;
const SNDCTL_DSP_SETFRAGMENT: libc::c_ulong = 0xc004500a;
const SNDCTL_DSP_SETTRIGGER: libc::c_ulong = 0x40045010;
const SNDCTL_DSP_GETOSPACE: libc::c_ulong = 0x8010500c;
const AFMT_S16_LE: libc::c_int = 0x10;
const PCM_ENABLE_OUTPUT: libc::c_int = 2;

#[repr(C)]
struct AudioBufferInfo {
    fragments: libc::c_int,
    fragstotal: libc::c_int,
    fragsize: libc::c_int,
    bytes: libc::c_int,
}

struct Shared {
    queue: Vec<i16>,
    head: usize,
    size: usize,
    dropped: u64,
    stopping: bool,
    worker_failed: bool,
}

struct Player {
    shared: &'static Mutex<Shared>,
    condition: &'static Condvar,
    worker: Option<std::thread::JoinHandle<()>>,
    source_rate: u32,
    output_rate: u32,
    volume_percent: i32,
    rate_remainder: u64,
    resampled: Vec<i16>,
    mono: Vec<i16>,
}

static SHARED: OnceLock<&'static Mutex<Shared>> = OnceLock::new();
static CONDITION: OnceLock<&'static Condvar> = OnceLock::new();
static PLAYER: Mutex<Option<Player>> = Mutex::new(None);

fn shared() -> &'static Mutex<Shared> {
    SHARED.get_or_init(|| {
        Box::leak(Box::new(Mutex::new(Shared {
            queue: Vec::new(),
            head: 0,
            size: 0,
            dropped: 0,
            stopping: false,
            worker_failed: false,
        })))
    })
}

fn condition() -> &'static Condvar {
    CONDITION.get_or_init(|| Box::leak(Box::new(Condvar::new())))
}

pub fn volume_percent() -> Result<i32, String> {
    let error = || "volume must be an integer from 0 through 100".to_owned();
    match std::env::var_os("RETRO_DECK_VOLUME_PERCENT") {
        None => Ok(42),
        Some(value) => {
            let text = value.to_str().ok_or_else(error)?;
            if text.is_empty() {
                return Err(error());
            }
            let mut volume: i32 = 0;
            for character in text.chars() {
                let digit = character.to_digit(10).ok_or_else(error)?;
                volume = volume * 10 + digit as i32;
                if volume > 100 {
                    return Err(error());
                }
            }
            Ok(volume)
        }
    }
}

fn output_rate(source_rate: u32, negotiated_rate: u32) -> u32 {
    // The OSS bridge reports 48 kHz but measurably consumes ~47,328
    // application frames per second; correct here, not at the device.
    if source_rate == 48000 && negotiated_rate == 48000 {
        47328
    } else {
        negotiated_rate
    }
}

fn scale_sample(sample: i32, percent: i32) -> i16 {
    (sample * percent / 100).clamp(-32768, 32767) as i16
}

fn configure(fd: &OwnedFd, source_rate: u32) -> Result<u32, String> {
    let raw = fd.as_raw_fd();
    // Eight 1 KiB fragments; smaller rings audibly underran during
    // framebuffer updates. The result is deliberately ignored.
    let mut fragment: libc::c_int = (8 << 16) | 10;
    unsafe { libc::ioctl(raw, SNDCTL_DSP_SETFRAGMENT, &mut fragment) };
    let failure = || format!("cannot configure /dev/dsp: {}", std::io::Error::last_os_error());
    let mut format = AFMT_S16_LE;
    if unsafe { libc::ioctl(raw, SNDCTL_DSP_SETFMT, &mut format) } != 0 || format != AFMT_S16_LE {
        return Err(failure());
    }
    let mut channels: libc::c_int = 1;
    if unsafe { libc::ioctl(raw, SNDCTL_DSP_CHANNELS, &mut channels) } != 0 || channels != 1 {
        return Err(failure());
    }
    // The 32 kHz hardware stream consumes one application frame per
    // hardware frame while the bridge reports 32768.
    let mut rate: libc::c_int = if source_rate == 32768 { 32000 } else { source_rate as libc::c_int };
    if unsafe { libc::ioctl(raw, SNDCTL_DSP_SPEED, &mut rate) } != 0 || rate <= 0 {
        return Err(failure());
    }
    Ok(rate as u32)
}

fn prefill(fd: &OwnedFd) -> Result<bool, String> {
    let raw = fd.as_raw_fd();
    let mut trigger: libc::c_int = 0;
    let trigger_pending = unsafe { libc::ioctl(raw, SNDCTL_DSP_SETTRIGGER, &mut trigger) } == 0;
    let mut space = AudioBufferInfo {
        fragments: 0,
        fragstotal: 0,
        fragsize: 0,
        bytes: 0,
    };
    if unsafe { libc::ioctl(raw, SNDCTL_DSP_GETOSPACE, &mut space) } == 0
        && space.bytes > 0
        && space.bytes <= 1024 * 1024
        && space.bytes % 2 == 0
    {
        // Starting an empty ring on the first small emulator callback
        // caused a repeatable startup XRUN.
        let silence = vec![0_u8; space.bytes as usize];
        write_all(fd, &silence).map_err(|_| "cannot prefill /dev/dsp".to_owned())?;
    }
    Ok(trigger_pending)
}

fn write_all(fd: &OwnedFd, mut bytes: &[u8]) -> Result<(), ()> {
    while !bytes.is_empty() {
        let written = unsafe {
            libc::write(fd.as_raw_fd(), bytes.as_ptr().cast(), bytes.len())
        };
        if written > 0 {
            bytes = &bytes[written as usize..];
        } else if written < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
        {
            continue;
        } else {
            return Err(());
        }
    }
    Ok(())
}

fn pcm_bytes<'a>(samples: &[i16], bytes: &'a mut [u8]) -> &'a [u8] {
    debug_assert!(bytes.len() >= samples.len() * 2);
    for (index, &sample) in samples.iter().enumerate() {
        let [low, high] = sample.to_le_bytes();
        bytes[index * 2] = low;
        bytes[index * 2 + 1] = high;
    }
    &bytes[..samples.len() * 2]
}

fn writer_thread(fd: OwnedFd, trigger_pending: bool) {
    let shared = shared();
    let condition = condition();
    let mut started = !trigger_pending;
    let mut output = vec![0_i16; WRITE_CHUNK_FRAMES];
    let mut bytes = vec![0_u8; WRITE_CHUNK_FRAMES * 2];
    loop {
        let count;
        {
            let mut guard = shared.lock().expect("audio queue lock");
            while !guard.stopping && guard.size == 0 {
                guard = condition.wait(guard).expect("audio queue wait");
            }
            if guard.stopping {
                return;
            }
            count = guard.size.min(WRITE_CHUNK_FRAMES);
            let capacity = guard.queue.len();
            for index in 0..count {
                output[index] = guard.queue[(guard.head + index) % capacity];
            }
            guard.head = (guard.head + count) % capacity;
            guard.size -= count;
        }
        if !started {
            let mut enable = PCM_ENABLE_OUTPUT;
            if unsafe { libc::ioctl(fd.as_raw_fd(), SNDCTL_DSP_SETTRIGGER, &mut enable) } != 0 {
                shared.lock().expect("audio queue lock").worker_failed = true;
                return;
            }
            started = true;
        }
        let bytes = pcm_bytes(&output[..count], &mut bytes);
        if write_all(&fd, bytes).is_err() {
            let mut guard = shared.lock().expect("audio queue lock");
            guard.worker_failed = true;
            guard.size = 0;
            return;
        }
    }
}

pub fn open(source_rate: u32, volume_percent: i32) -> Result<(), String> {
    close();
    if source_rate == 0 || !(0..=100).contains(&volume_percent) {
        return Err("invalid audio rate or volume".to_owned());
    }
    if volume_percent == 0 {
        return Ok(());
    }
    let raw = unsafe { libc::open(AUDIO_DEVICE.as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC) };
    if raw < 0 {
        return Err(format!(
            "cannot open /dev/dsp: {}",
            std::io::Error::last_os_error()
        ));
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let negotiated = configure(&fd, source_rate)?;
    let trigger_pending = prefill(&fd)?;
    {
        let mut guard = shared().lock().expect("audio queue lock");
        guard.queue = vec![0; QUEUE_FRAMES];
        guard.head = 0;
        guard.size = CUSHION_FRAMES;
        guard.dropped = 0;
        guard.stopping = false;
        guard.worker_failed = false;
    }
    let worker = std::thread::Builder::new()
        .name("retro-deck-audio".to_owned())
        .spawn(move || writer_thread(fd, trigger_pending))
        .map_err(|error| format!("cannot start audio writer: {error}"))?;
    *PLAYER.lock().expect("audio player lock") = Some(Player {
        shared: shared(),
        condition: condition(),
        worker: Some(worker),
        source_rate,
        output_rate: output_rate(source_rate, negotiated),
        volume_percent,
        rate_remainder: 0,
        resampled: Vec::new(),
        mono: Vec::new(),
    });
    Ok(())
}

fn enqueue(player: &Player, samples: &[i16]) {
    let mut guard = player.shared.lock().expect("audio queue lock");
    if guard.queue.is_empty() || guard.stopping || guard.worker_failed {
        return;
    }
    let capacity = guard.queue.len();
    let mut source = samples;
    if source.len() > capacity {
        guard.dropped += (source.len() - capacity) as u64;
        source = &source[source.len() - capacity..];
    }
    let free = capacity - guard.size;
    if source.len() > free {
        let shortfall = source.len() - free;
        guard.head = (guard.head + shortfall) % capacity;
        guard.size -= shortfall;
        guard.dropped += shortfall as u64;
    }
    let tail = (guard.head + guard.size) % capacity;
    for (index, &sample) in source.iter().enumerate() {
        let slot = (tail + index) % capacity;
        guard.queue[slot] = sample;
    }
    guard.size += source.len();
    drop(guard);
    player.condition.notify_one();
}

pub fn write_stereo(samples: &[i16]) {
    let mut guard = PLAYER.lock().expect("audio player lock");
    let Some(player) = guard.as_mut() else {
        return;
    };
    let frames = samples.len() / 2;
    if frames == 0 {
        return;
    }
    player.mono.clear();
    for frame in 0..frames {
        let mixed = (i32::from(samples[frame * 2]) + i32::from(samples[frame * 2 + 1])) / 2;
        player.mono.push(scale_sample(mixed, player.volume_percent));
    }
    if player.source_rate == player.output_rate {
        let mono = std::mem::take(&mut player.mono);
        enqueue(player, &mono);
        player.mono = mono;
        return;
    }
    // Block-local linear resampling with a persistent fractional carry.
    let scaled = frames as u64 * u64::from(player.output_rate) + player.rate_remainder;
    let output_frames = (scaled / u64::from(player.source_rate)) as usize;
    player.rate_remainder = scaled % u64::from(player.source_rate);
    if output_frames == 0 {
        return;
    }
    player.resampled.clear();
    if frames == 1 || output_frames == 1 {
        player
            .resampled
            .resize(output_frames, player.mono[0]);
    } else {
        let step = ((frames as u64 - 1) << 32) / (output_frames as u64 - 1);
        let mut position: u64 = 0;
        for _ in 0..output_frames {
            let first = (position >> 32) as usize;
            let fraction = ((position >> 16) & 0xffff) as i64;
            let value = if first >= frames - 1 {
                player.mono[frames - 1]
            } else {
                let start = i64::from(player.mono[first]);
                let delta = i64::from(player.mono[first + 1]) - start;
                (start + ((delta * fraction) >> 16)) as i16
            };
            player.resampled.push(value);
            position += step;
        }
    }
    let resampled = std::mem::take(&mut player.resampled);
    enqueue(player, &resampled);
    player.resampled = resampled;
}

pub fn queued_frames() -> usize {
    shared().lock().map(|guard| guard.size).unwrap_or(0)
}

pub fn dropped_frames() -> u64 {
    shared().lock().map(|guard| guard.dropped).unwrap_or(0)
}

pub fn close() {
    let player = PLAYER.lock().expect("audio player lock").take();
    if let Some(mut player) = player {
        {
            let mut guard = player.shared.lock().expect("audio queue lock");
            guard.stopping = true;
        }
        player.condition.notify_all();
        if let Some(worker) = player.worker.take() {
            let _ = worker.join();
        }
        let mut guard = player.shared.lock().expect("audio queue lock");
        guard.queue = Vec::new();
        guard.head = 0;
        guard.size = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_cpp_output_rate_policy() {
        assert_eq!(output_rate(48000, 48000), 47328);
        assert_eq!(output_rate(32768, 32000), 32000);
        assert_eq!(output_rate(44100, 44100), 44100);
        assert_eq!(output_rate(48000, 44100), 44100);
    }

    #[test]
    fn encodes_pcm_without_changing_sample_order() {
        let samples = [-32768_i16, -1, 0, 1, 32767];
        let mut bytes = [0_u8; 10];
        assert_eq!(
            pcm_bytes(&samples, &mut bytes),
            [0, 128, 255, 255, 0, 0, 1, 0, 255, 127]
        );
    }

    #[test]
    fn scales_samples_with_truncation_and_clamping() {
        assert_eq!(scale_sample(-32768, 100), -32768);
        assert_eq!(scale_sample(32767, 100), 32767);
        assert_eq!(scale_sample(-99, 50), -49);
        assert_eq!(scale_sample(99, 50), 49);
        assert_eq!(scale_sample(40000, 100), 32767);
    }

    #[test]
    fn parses_the_volume_environment_contract() {
        // The environment cannot be mutated safely in-process here; the
        // parsing rules are pinned through the Lisp policy tests instead.
        assert!(volume_percent().is_ok());
    }
}
