use crate::font;

const GLYPH_WIDTH: u32 = 5;
const GLYPH_HEIGHT: u32 = 7;
const GLYPH_ADVANCE: u32 = 6;

#[derive(Clone, Debug)]
pub(crate) struct TextMask {
    width: u32,
    height: u32,
    scale: u32,
    // Cumulative ink per column, at the eight glyph-row boundaries.
    columns: Vec<[u8; GLYPH_HEIGHT as usize + 1]>,
}

pub(crate) struct Projection {
    scroll: f64,
    camera_distance: f64,
    maximum_depth: f64,
    horizon_y: i32,
    clip_top: i32,
    fade_invisible_y: i32,
    fade_opaque_y: i32,
    bottom_y: i32,
    color: u16,
}

impl TextMask {
    pub(crate) fn new(text: &[u8], scale: u32, canvas_size: (u32, u32)) -> Result<Self, String> {
        if text.is_empty() || scale == 0 {
            return Err("projected text and scale must be nonempty".to_owned());
        }
        let characters =
            u32::try_from(text.len()).map_err(|_| "projected text is too long".to_owned())?;
        let width = characters
            .checked_mul(GLYPH_ADVANCE)
            .and_then(|value| value.checked_sub(1))
            .and_then(|value| value.checked_mul(scale))
            .ok_or_else(|| "projected text dimensions overflow".to_owned())?;
        let height = GLYPH_HEIGHT
            .checked_mul(scale)
            .ok_or_else(|| "projected text dimensions overflow".to_owned())?;
        if width > canvas_size.0 || height > canvas_size.1 {
            return Err("projected text exceeds the native canvas".to_owned());
        }
        let mut columns = vec![[0; GLYPH_HEIGHT as usize + 1]; (width / scale) as usize];
        for (index, character) in text.iter().enumerate() {
            let rows = font::rows(*character);
            for column in 0..GLYPH_WIDTH {
                let prefix = &mut columns[index * GLYPH_ADVANCE as usize + column as usize];
                for (row, mask) in rows.iter().enumerate() {
                    prefix[row + 1] = prefix[row] + ((mask >> (GLYPH_WIDTH - 1 - column)) & 1);
                }
            }
        }
        Ok(Self {
            width,
            height,
            scale,
            columns,
        })
    }

    pub(crate) fn draw(
        &self,
        canvas: &mut [u8],
        canvas_size: (u32, u32),
        source_y: i32,
        projection: &Projection,
    ) -> Result<(), String> {
        let (canvas_width, canvas_height) = canvas_size;
        if canvas.len() != canvas_width as usize * canvas_height as usize * 4 {
            return Err("native canvas dimensions are invalid".to_owned());
        }
        let source_top = f64::from(source_y).max(projection.scroll - projection.maximum_depth);
        let source_bottom = (f64::from(source_y) + f64::from(self.height)).min(projection.scroll);
        if source_top >= source_bottom {
            return Ok(());
        }

        let top_y = projection.screen_y(source_top);
        let bottom_y = projection.screen_y(source_bottom);
        let first_y = projection.clip_top.max(top_y.floor() as i32);
        let last_y = (projection.bottom_y - 1).min(bottom_y.ceil() as i32 - 1);
        let projection_height = f64::from(projection.bottom_y - projection.horizon_y);
        let glyph_units = 256.0 / f64::from(self.scale);
        let glyph_position = |y: i32| {
            let scale = f64::from(y - projection.horizon_y) / projection_height;
            let depth = projection.camera_distance * (1.0 / scale - 1.0);
            ((projection.scroll - depth - f64::from(source_y)) * glyph_units).round() as i32
        };
        let limit = GLYPH_HEIGHT as i32 * 256;
        let clipped_top = ((source_top - f64::from(source_y)) * glyph_units).round() as i32;
        let clipped_bottom = ((source_bottom - f64::from(source_y)) * glyph_units).round() as i32;
        let mut end = glyph_position(first_y);
        for y in first_y..=last_y {
            let start = end;
            end = glyph_position(y + 1);
            let alpha = projection.alpha(y);
            if alpha == 0 || start == end {
                continue;
            }
            // Average the mask across a screen pixel's vertical footprint.
            // The ink can then move between rows without snapping an entire
            // stroke to the next row at once or vanishing near the horizon.
            let top = GlyphEdge::new(start.max(clipped_top).clamp(0, limit));
            let bottom = GlyphEdge::new(end.min(clipped_bottom).clamp(0, limit));
            let span = end - start;
            let alpha_per_ink = ((alpha << 16) + span / 2) / span;

            let scale = (f64::from(y - projection.horizon_y) + 0.5) / projection_height;
            let center = f64::from(canvas_width) * 0.5;
            let left = center - f64::from(self.width) * 0.5 * scale;
            let right = center + f64::from(self.width) * 0.5 * scale;
            let first_x = 0.max((left - 0.5).ceil() as i32);
            let last_x = (canvas_width as i32 - 1).min((right - 0.5).floor() as i32);
            // Walk source columns in 16.16 fixed point, with one division per row.
            let inverse_scale = 1.0 / (scale * f64::from(self.scale));
            let step = (inverse_scale * 65536.0) as i64;
            let mut source_fx = (((f64::from(first_x) + 0.5 - center) * inverse_scale
                + self.columns.len() as f64 * 0.5)
                * 65536.0) as i64;
            for x in first_x..=last_x {
                let source_column = (source_fx >> 16) as usize;
                source_fx += step;
                let Some(column) = self.columns.get(source_column) else {
                    continue;
                };
                let ink = bottom.integral(column) - top.integral(column);
                let alpha = ((ink * alpha_per_ink + 32768) >> 16).min(alpha);
                if alpha <= 0 {
                    continue;
                }
                let offset = (y as usize * canvas_width as usize + x as usize) * 4;
                let color = if alpha == 256 {
                    projection.color
                } else {
                    blend_rgb565(
                        projection.color,
                        rgba_rgb565(&canvas[offset..offset + 4]),
                        alpha,
                    )
                };
                write_rgb565(&mut canvas[offset..offset + 4], color);
            }
        }
        Ok(())
    }
}

struct GlyphEdge {
    row: usize,
    fraction: i32,
}

impl GlyphEdge {
    fn new(fixed: i32) -> Self {
        // The final boundary is the far edge of the last glyph row.
        let row = ((fixed >> 8) as usize).min(GLYPH_HEIGHT as usize - 1);
        Self {
            row,
            fraction: fixed - (row as i32 * 256),
        }
    }

    fn integral(&self, prefix: &[u8; GLYPH_HEIGHT as usize + 1]) -> i32 {
        let start = prefix[self.row];
        i32::from(start) * 256 + i32::from(prefix[self.row + 1] - start) * self.fraction
    }
}
impl Projection {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        elapsed_ms: i64,
        speed_numerator: u32,
        speed_denominator: u32,
        cycle: u32,
        camera_distance: u32,
        maximum_depth: u32,
        horizon_y: i32,
        clip_top: i32,
        fade_invisible_y: i32,
        fade_opaque_y: i32,
        bottom_y: i32,
        color: u32,
        canvas_height: u32,
    ) -> Result<Self, String> {
        if speed_denominator == 0 || cycle == 0 || camera_distance == 0 || maximum_depth == 0 {
            return Err("projection scale and depth parameters must be positive".to_owned());
        }
        if !(0 <= horizon_y
            && horizon_y < clip_top
            && clip_top <= fade_invisible_y
            && fade_invisible_y < fade_opaque_y
            && fade_opaque_y <= bottom_y
            && bottom_y <= canvas_height as i32)
        {
            return Err("projection vertical geometry is invalid".to_owned());
        }
        let speed = f64::from(speed_numerator) / f64::from(speed_denominator);
        let scroll = (elapsed_ms.max(0) as f64 * speed) % f64::from(cycle);
        Ok(Self {
            scroll,
            camera_distance: f64::from(camera_distance),
            maximum_depth: f64::from(maximum_depth),
            horizon_y,
            clip_top,
            fade_invisible_y,
            fade_opaque_y,
            bottom_y,
            color: color_rgb565(color),
        })
    }

    fn screen_y(&self, source_y: f64) -> f64 {
        let depth = self.scroll - source_y;
        let scale = self.camera_distance / (self.camera_distance + depth);
        f64::from(self.horizon_y) + f64::from(self.bottom_y - self.horizon_y) * scale
    }

    fn alpha(&self, screen_y: i32) -> i32 {
        if screen_y <= self.fade_invisible_y {
            0
        } else if screen_y >= self.fade_opaque_y {
            256
        } else {
            (screen_y - self.fade_invisible_y) * 256 / (self.fade_opaque_y - self.fade_invisible_y)
        }
    }
}

fn color_rgb565(color: u32) -> u16 {
    rgb565((color >> 16) as u8, (color >> 8) as u8, color as u8)
}

fn rgb565(red: u8, green: u8, blue: u8) -> u16 {
    ((u16::from(red & 0xf8)) << 8) | ((u16::from(green & 0xfc)) << 3) | u16::from(blue >> 3)
}

fn rgba_rgb565(rgba: &[u8]) -> u16 {
    rgb565(rgba[0], rgba[1], rgba[2])
}

fn blend_rgb565(foreground: u16, background: u16, alpha: i32) -> u16 {
    let inverse = 256 - alpha;
    let red = (i32::from((foreground >> 11) & 0x1f) * alpha
        + i32::from((background >> 11) & 0x1f) * inverse
        + 128)
        >> 8;
    let green = (i32::from((foreground >> 5) & 0x3f) * alpha
        + i32::from((background >> 5) & 0x3f) * inverse
        + 128)
        >> 8;
    let blue =
        (i32::from(foreground & 0x1f) * alpha + i32::from(background & 0x1f) * inverse + 128) >> 8;
    ((red << 11) | (green << 5) | blue) as u16
}

fn write_rgb565(rgba: &mut [u8], color: u16) {
    let red = ((color >> 11) & 0x1f) as u8;
    let green = ((color >> 5) & 0x3f) as u8;
    let blue = (color & 0x1f) as u8;
    rgba.copy_from_slice(&[
        (red << 3) | (red >> 2),
        (green << 2) | (green >> 4),
        (blue << 3) | (blue >> 2),
        255,
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fnv1a;

    fn hash_rgb565(canvas: &[u8]) -> u64 {
        fnv1a(
            canvas
                .chunks_exact(4)
                .flat_map(|rgba| rgba_rgb565(rgba).to_le_bytes()),
        )
    }

    fn draw_starfield(canvas: &mut [u8], size: (u32, u32), color: u16) {
        for index in 0..96_u32 {
            if index % 7 == 0 {
                continue;
            }
            let x = (index * 193 + 47) % size.0;
            let y = (index * 83 + 29) % size.1;
            let side = if index % 11 == 0 { 2 } else { 1 };
            for row in y..(y + side).min(size.1) {
                for column in x..(x + side).min(size.0) {
                    let offset = (row as usize * size.0 as usize + column as usize) * 4;
                    write_rgb565(&mut canvas[offset..offset + 4], color);
                }
            }
        }
    }

    fn draw_close(canvas: &mut [u8], size: (u32, u32), color: u16) {
        for offset in (-12..=12).step_by(4) {
            for (left, top) in [(1240 + offset, 40 + offset), (1240 + offset, 40 - offset)] {
                for y in top..top + 4 {
                    for x in left..left + 4 {
                        let index = (y as usize * size.0 as usize + x as usize) * 4;
                        write_rgb565(&mut canvas[index..index + 4], color);
                    }
                }
            }
        }
    }

    #[test]
    // The fixture covers entry, the main crawl, and the fade near the horizon.
    fn projected_line_fixture() {
        let size = (1280, 480);
        let mask = TextMask::new(b"HHHHHHHHHH", 4, size).unwrap();
        assert_eq!((mask.width, mask.height), (236, 28));
        for (elapsed_ms, expected) in [
            (1000, 0xe49c_0649_dc4e_5833),
            (3000, 0xfd57_2778_0caf_d751),
            (50000, 0x3167_4bdc_7f3d_fc63),
            (55000, 0x2342_72c5_86ba_5c5b),
        ] {
            let mut canvas = [0, 0, 0, 255].repeat(size.0 as usize * size.1 as usize);
            draw_starfield(&mut canvas, size, color_rgb565(0x949494));
            let projection = Projection::new(
                elapsed_ms, 1, 20, 4044, 420, 4000, 56, 72, 104, 210, 480, 0xffffaf, size.1,
            )
            .unwrap();
            mask.draw(&mut canvas, size, 0, &projection).unwrap();
            draw_close(&mut canvas, size, color_rgb565(0x949494));
            assert_eq!(hash_rgb565(&canvas), expected);
        }
    }
    #[test]
    fn projected_ink_moves_at_subpixel_intervals() {
        let size = (1280, 480);
        let mask = TextMask::new(b"I", 4, size).unwrap();
        let center_of_ink = |elapsed_ms| {
            let mut canvas = [0, 0, 0, 255].repeat(size.0 as usize * size.1 as usize);
            let projection = Projection::new(
                elapsed_ms, 1, 20, 4044, 420, 4000, 56, 72, 104, 210, 480, 0xffffff, size.1,
            )
            .unwrap();
            mask.draw(&mut canvas, size, 0, &projection).unwrap();
            let mut weighted = 0;
            let mut sum = 0;
            for (y, row) in canvas.chunks_exact(size.0 as usize * 4).enumerate() {
                let ink = u32::from(row[size.0 as usize * 2]);
                weighted += y as u32 * ink;
                sum += ink;
            }
            assert!(sum > 0);
            f64::from(weighted) / f64::from(sum)
        };
        let centers: Vec<_> = (0..8)
            .map(|frame| center_of_ink(15_000 + frame * 16))
            .collect();
        for samples in centers.windows(2) {
            let distance = samples[0] - samples[1];
            assert!(distance > 0.0 && distance < 0.5, "{samples:?}");
        }
    }
}
