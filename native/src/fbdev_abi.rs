//! Linux framebuffer ioctl ABI shared by the dashboard and emulator targets.

use std::ffi::{c_int, c_ulong};

pub(super) const FBIOGET_VSCREENINFO: c_ulong = 0x4600;
pub(super) const FBIOGET_FSCREENINFO: c_ulong = 0x4602;
pub(super) const FBIOBLANK: c_ulong = 0x4611;
pub(super) const FB_BLANK_UNBLANK: c_int = 0;
pub(super) const FB_TYPE_PACKED_PIXELS: u32 = 0;
pub(super) const FB_VISUAL_TRUECOLOR: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct FbBitfield {
    pub(super) offset: u32,
    pub(super) length: u32,
    pub(super) msb_right: u32,
}

#[repr(C)]
#[derive(Default)]
pub(super) struct FbVariableScreenInfo {
    pub(super) xres: u32,
    pub(super) yres: u32,
    pub(super) xres_virtual: u32,
    pub(super) yres_virtual: u32,
    pub(super) xoffset: u32,
    pub(super) yoffset: u32,
    pub(super) bits_per_pixel: u32,
    pub(super) grayscale: u32,
    pub(super) red: FbBitfield,
    pub(super) green: FbBitfield,
    pub(super) blue: FbBitfield,
    pub(super) transparency: FbBitfield,
    pub(super) nonstandard: u32,
    pub(super) activate: u32,
    pub(super) height_mm: u32,
    pub(super) width_mm: u32,
    pub(super) accel_flags: u32,
    pub(super) pixclock: u32,
    pub(super) left_margin: u32,
    pub(super) right_margin: u32,
    pub(super) upper_margin: u32,
    pub(super) lower_margin: u32,
    pub(super) hsync_length: u32,
    pub(super) vsync_length: u32,
    pub(super) sync: u32,
    pub(super) vmode: u32,
    pub(super) rotate: u32,
    pub(super) colorspace: u32,
    pub(super) reserved: [u32; 4],
}

#[repr(C)]
#[derive(Default)]
pub(super) struct FbFixedScreenInfo {
    pub(super) id: [u8; 16],
    pub(super) smem_start: c_ulong,
    pub(super) smem_len: u32,
    pub(super) kind: u32,
    pub(super) type_aux: u32,
    pub(super) visual: u32,
    pub(super) xpanstep: u16,
    pub(super) ypanstep: u16,
    pub(super) ywrapstep: u16,
    pub(super) line_length: u32,
    pub(super) mmio_start: c_ulong,
    pub(super) mmio_len: u32,
    pub(super) accel: u32,
    pub(super) capabilities: u16,
    pub(super) reserved: [u16; 2],
}

const _: [(); 160] = [(); std::mem::size_of::<FbVariableScreenInfo>()];
#[cfg(target_pointer_width = "64")]
const _: [(); 80] = [(); std::mem::size_of::<FbFixedScreenInfo>()];
#[cfg(target_pointer_width = "32")]
const _: [(); 68] = [(); std::mem::size_of::<FbFixedScreenInfo>()];
