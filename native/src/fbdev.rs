//! Read-only scanout capture for the fbterm Wayland mirror.

use crate::canvas;
use crate::fbdev_abi::{FbFixedScreenInfo, FbVariableScreenInfo, Geometry, query_geometry};
use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use std::ffi::c_void;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::ptr;

const LOGICAL_WIDTH: usize = canvas::WIDTH as usize;
const LOGICAL_HEIGHT: usize = canvas::HEIGHT as usize;
const PHYSICAL_WIDTH: usize = 600;
const PHYSICAL_HEIGHT: usize = 1280;

fn validate_console_geometry(
    variable: &FbVariableScreenInfo,
    fixed: &FbFixedScreenInfo,
) -> Result<Geometry, String> {
    let rows = if variable.yres_virtual == 0 {
        variable.yres
    } else {
        variable.yres_virtual
    } as usize;
    let stride = fixed.line_length as usize;
    let required = stride
        .checked_mul(rows)
        .ok_or_else(|| "unsupported console framebuffer geometry".to_owned())?;
    let map_size = fixed.smem_len as usize;
    if variable.xres != PHYSICAL_WIDTH as u32
        || variable.yres != PHYSICAL_HEIGHT as u32
        || variable.bits_per_pixel != 16
        || rows < PHYSICAL_HEIGHT
        || stride < PHYSICAL_WIDTH * 2
        || !stride.is_multiple_of(2)
        || map_size < required
    {
        return Err("unsupported console framebuffer geometry".to_owned());
    }
    Ok(Geometry { stride, map_size })
}

struct Mapping {
    pointer: *mut c_void,
    size: usize,
}

impl Mapping {
    fn new(file: &File, size: usize, context: &str) -> Result<Self, String> {
        let pointer = unsafe {
            mmap(
                ptr::null_mut(),
                size,
                ProtFlags::READ,
                MapFlags::SHARED,
                file,
                0,
            )
        }
        .map_err(|error| format!("{context}: {error}"))?;
        Ok(Self { pointer, size })
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        unsafe {
            let _ = munmap(self.pointer, self.size);
        }
    }
}
fn read_scanout_into_canvas(frame: &[u16], row_words: usize, canvas: &mut [u16]) -> bool {
    let Some(required) = row_words.checked_mul(PHYSICAL_HEIGHT) else {
        return false;
    };
    if frame.len() < required
        || row_words < PHYSICAL_WIDTH
        || canvas.len() != LOGICAL_WIDTH * LOGICAL_HEIGHT
    {
        return false;
    }
    for logical_x in 0..LOGICAL_WIDTH {
        let physical_row = PHYSICAL_HEIGHT - 1 - logical_x;
        let source = &frame[physical_row * row_words..][..LOGICAL_HEIGHT];
        for logical_y in 0..LOGICAL_HEIGHT {
            canvas[logical_y * LOGICAL_WIDTH + logical_x] = source[logical_y];
        }
    }
    true
}

pub(crate) fn read_console_scanout() -> Result<Vec<u16>, String> {
    let file = File::open("/dev/fb0")
        .map_err(|error| format!("cannot open console framebuffer: {error}"))?;
    let (variable, fixed) = query_geometry(file.as_raw_fd())
        .map_err(|error| format!("cannot query console framebuffer: {error}"))?;
    let geometry = validate_console_geometry(&variable, &fixed)?;
    let mapping = Mapping::new(&file, geometry.map_size, "cannot map console framebuffer")?;
    let frame =
        unsafe { std::slice::from_raw_parts(mapping.pointer.cast::<u16>(), geometry.map_size / 2) };
    let mut canvas = vec![0; LOGICAL_WIDTH * LOGICAL_HEIGHT];
    if !read_scanout_into_canvas(frame, geometry.stride / 2, &mut canvas) {
        return Err("cannot rotate console framebuffer".to_owned());
    }
    Ok(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_padded_console_stride_and_rejects_short_mapping() {
        let variable = FbVariableScreenInfo {
            xres: PHYSICAL_WIDTH as u32,
            yres: PHYSICAL_HEIGHT as u32,
            bits_per_pixel: 16,
            ..Default::default()
        };
        let mut fixed = FbFixedScreenInfo {
            line_length: 1216,
            smem_len: 1216 * PHYSICAL_HEIGHT as u32,
            ..Default::default()
        };
        let geometry = validate_console_geometry(&variable, &fixed).unwrap();
        assert_eq!(geometry.stride, 1216);
        assert_eq!(geometry.map_size, 1216 * PHYSICAL_HEIGHT);
        fixed.smem_len -= 1;
        assert!(validate_console_geometry(&variable, &fixed).is_err());
    }

    #[test]
    fn rotates_console_capture_without_reading_row_padding() {
        let row_words = PHYSICAL_WIDTH + 3;
        let mut frame = vec![0; row_words * PHYSICAL_HEIGHT];
        frame[(PHYSICAL_HEIGHT - 1) * row_words] = 0x1234;
        frame[LOGICAL_HEIGHT - 1] = 0xabcd;
        for row in frame.chunks_exact_mut(row_words) {
            row[PHYSICAL_WIDTH..].fill(0xdead);
        }
        let mut canvas = vec![0; LOGICAL_WIDTH * LOGICAL_HEIGHT];
        assert!(read_scanout_into_canvas(&frame, row_words, &mut canvas));
        assert_eq!(canvas[0], 0x1234);
        assert_eq!(canvas[canvas.len() - 1], 0xabcd);
        assert!(!canvas.contains(&0xdead));
        assert!(!read_scanout_into_canvas(
            &frame[..frame.len() - 1],
            row_words,
            &mut canvas
        ));
    }
}
