//! OSS wire ABI and signed-16 mono setup shared by menu and emulator audio.
use libc::{c_int, c_ulong};
use std::io;

pub(crate) const AFMT_S16_LE: c_int = 0x10;
pub(crate) const SNDCTL_DSP_SYNC: c_ulong = 0x5001;
pub(crate) const SNDCTL_DSP_SPEED: c_ulong = 0xc0045002;
pub(crate) const SNDCTL_DSP_SETFMT: c_ulong = 0xc0045005;
pub(crate) const SNDCTL_DSP_CHANNELS: c_ulong = 0xc0045006;
pub(crate) const SNDCTL_DSP_SETFRAGMENT: c_ulong = 0xc004500a;
pub(crate) const SNDCTL_DSP_GETOSPACE: c_ulong = 0x8010500c;
pub(crate) const SNDCTL_DSP_SETTRIGGER: c_ulong = 0x40045010;
pub(crate) const PCM_ENABLE_OUTPUT: c_int = 2;

#[repr(C)]
#[derive(Default)]
pub(crate) struct AudioBufferInfo {
    pub(crate) fragments: c_int,
    pub(crate) fragment_total: c_int,
    pub(crate) fragment_size: c_int,
    pub(crate) bytes: c_int,
}

pub(crate) fn configure_mono(
    fd: c_int,
    fragment: c_int,
    requested_rate: c_int,
) -> io::Result<c_int> {
    let mut fragment = fragment;
    let mut format = AFMT_S16_LE;
    let mut channels = 1;
    let mut rate = requested_rate;
    // The bridge may ignore fragment hints. Format and channel changes are not optional.
    unsafe { libc::ioctl(fd, SNDCTL_DSP_SETFRAGMENT, &mut fragment) };
    for (request, value, expected) in [
        (SNDCTL_DSP_SETFMT, &mut format, Some(AFMT_S16_LE)),
        (SNDCTL_DSP_CHANNELS, &mut channels, Some(1)),
        (SNDCTL_DSP_SPEED, &mut rate, None),
    ] {
        if unsafe { libc::ioctl(fd, request, value as *mut c_int) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if expected.is_some_and(|expected| *value != expected) || *value <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "unsupported OSS PCM format",
            ));
        }
    }
    Ok(rate)
}
