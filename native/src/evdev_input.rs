//! Shared Linux evdev transport. Consumers keep their own edge/held-state policy.

use evdev::raw_stream::RawDevice;
use evdev::{AbsoluteAxisCode, KeyCode};
use rustix::fs::{Mode, OFlags, open};
use std::io::{self, ErrorKind};
use std::path::Path;

pub(crate) const GAMEPAD_VENDOR: u16 = 0x1c59;
pub(crate) const GAMEPAD_PRODUCT: u16 = 0x0026;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct AxisInfo {
    pub(crate) minimum: i32,
    pub(crate) maximum: i32,
    pub(crate) value: i32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct GamepadSnapshot {
    pub(crate) x: AxisInfo,
    pub(crate) y: AxisInfo,
    pub(crate) raw_buttons: u32,
}

pub(crate) fn open_device(path: &Path) -> Option<RawDevice> {
    let fd = open(
        path,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .ok()?;
    RawDevice::from_fd(fd).ok()
}

pub(crate) fn gamepad_axes(device: &RawDevice) -> io::Result<(AxisInfo, AxisInfo)> {
    let mut x = None;
    let mut y = None;
    for (code, info) in device.get_absinfo()? {
        let axis = AxisInfo {
            minimum: info.minimum(),
            maximum: info.maximum(),
            value: info.value(),
        };
        match code {
            AbsoluteAxisCode::ABS_X => x = Some(axis),
            AbsoluteAxisCode::ABS_Y => y = Some(axis),
            _ => {}
        }
    }
    let x = x.ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "ABS_X is unavailable"))?;
    let y = y.ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "ABS_Y is unavailable"))?;
    Ok((x, y))
}

pub(crate) fn gamepad_snapshot(device: &RawDevice) -> io::Result<GamepadSnapshot> {
    let (x, y) = gamepad_axes(device)?;
    let keys = device.get_key_state()?;
    let mut raw_buttons = 0;
    for index in 0..8 {
        if keys.contains(KeyCode(KeyCode::BTN_TRIGGER.0 + index)) {
            raw_buttons |= 1 << index;
        }
    }
    Ok(GamepadSnapshot { x, y, raw_buttons })
}
