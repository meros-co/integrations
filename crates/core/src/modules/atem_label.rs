//! Multiviewer labels drawn from text, for switchers that take labels as
//! bitmaps (those before the ATEM Mini; later ones draw their own).
//!
//! No Blackmagic document describes the label bitmap. Its layout is
//! atem-connection's (Sofie `src/lib/multiviewLabel.ts`, MIT): one byte per
//! pixel in the switcher's own palette, a label per multiviewer resolution
//! the switcher can run (4K, then 1080, then 720 lines) stacked top to
//! bottom in one buffer as wide as the first, each a rounded box with a
//! two-pixel border, centered, holding the text. The palette's mapping from
//! 8-bit coverage, the box sizes and the corner pixels below are that
//! file's data, which describe the switcher's encoding (its MIT notice is in
//! `third-party/LICENSE-atem-connection.txt`). The text is drawn
//! with Roboto Regular 2.137 (Google, Apache License 2.0, in
//! `assets/fonts/`), the font atem-connection uses, rasterized with swash.

use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format, Vector};
use swash::FontRef;

/// Roboto Regular 2.137 (Apache License 2.0; `assets/fonts/LICENSE-Roboto.txt`).
const FONT: &[u8] = include_bytes!("../../assets/fonts/Roboto-Regular.ttf");

/// The switcher's palette value for each 8-bit coverage of a text pixel
/// (Sofie `multiviewLabel.ts:13-27`). Low coverage maps to the box's
/// background, so faint edges never cut through it.
const PALETTE: [u8; 256] = [
    14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, //
    14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, //
    14, 14, 14, 14, 14, 14, 14, 14, 29, 85, 132, 17, 30, 30, 96, 136, //
    136, 186, 101, 101, 13, 19, 19, 72, 217, 217, 124, 130, 123, 47, 28, 51, //
    32, 192, 166, 56, 167, 193, 189, 172, 198, 198, 41, 41, 154, 81, 109, 109, //
    199, 60, 108, 37, 12, 12, 15, 153, 142, 142, 68, 35, 35, 173, 141, 197, //
    197, 144, 94, 94, 116, 42, 214, 214, 40, 46, 46, 184, 181, 127, 127, 175, //
    31, 31, 150, 213, 73, 95, 86, 191, 191, 117, 135, 135, 208, 22, 78, 78, //
    107, 179, 179, 210, 119, 58, 58, 67, 209, 209, 171, 110, 89, 89, 103, 133, //
    43, 202, 137, 66, 66, 39, 151, 129, 71, 200, 200, 26, 111, 128, 16, 16, //
    23, 23, 196, 201, 36, 106, 207, 205, 205, 69, 139, 62, 105, 90, 203, 203, //
    216, 54, 99, 99, 100, 88, 88, 146, 145, 74, 74, 80, 204, 204, 180, 84, //
    138, 212, 215, 24, 24, 63, 187, 187, 118, 148, 183, 183, 57, 102, 102, 206, //
    44, 152, 152, 190, 134, 134, 59, 182, 87, 87, 48, 174, 125, 104, 140, 140, //
    188, 92, 131, 178, 64, 70, 70, 76, 65, 79, 121, 113, 176, 176, 177, 211, //
    75, 75, 120, 82, 82, 93, 98, 122, 122, 53, 115, 115, 45, 55, 34, 34, //
];

/// The box's background: the palette value of no coverage.
const BACKGROUND: u8 = PALETTE[0];

/// One resolution's label (Sofie `multiviewLabel.ts:55-121`).
struct Size {
    width: usize,
    height: usize,
    x_pad: usize,
    pad_bottom: usize,
    pad_top: usize,
    /// Pixels per em.
    font: f32,
    border: u8,
    /// The top left corner, row by row; the other corners mirror it.
    corner: &'static [&'static [u8]],
}

const UHD: Size = Size {
    width: 640,
    height: 100,
    x_pad: 16,
    pad_bottom: 16,
    pad_top: 4,
    font: 46.0,
    border: 0x05,
    corner: &[
        &[0, 0, 0, 0, 0, 0, 223, 2, 162, 220, 20],
        &[0, 0, 0, 0, 223, 195, 20, 5, 5, 5, 5],
        &[0, 0, 0, 7, 3, 5, 5, 110, 141, 124, 29],
        &[0, 0, 7, 220, 5, 200, 97, 14, 14, 14, 14],
        &[0, 223, 3, 5, 209, 29, 14, 14, 14, 14, 14],
        &[0, 219, 5, 200, 29, 14, 14, 14, 14, 14, 14],
        &[223, 20, 5, 97, 14, 14, 14, 14, 14, 14, 14],
        &[2, 5, 110, 14, 14, 14, 14, 14, 14, 14, 14],
        &[162, 5, 141, 14, 14, 14, 14, 14, 14, 14, 14],
        &[220, 5, 124, 14, 14, 14, 14, 14, 14, 14, 14],
        &[20, 5, 29, 14, 14, 14, 14, 14, 14, 14, 14],
    ],
};

const HD1080: Size = Size {
    width: 320,
    height: 50,
    x_pad: 10,
    pad_bottom: 10,
    pad_top: 4,
    font: 24.0,
    border: 0x05,
    corner: &[
        &[0, 0, 1, 229, 230, 20],
        &[0, 7, 158, 5, 5, 5],
        &[1, 158, 5, 23, 37, 101],
        &[229, 5, 23, 29, 14, 14],
        &[230, 5, 37, 14, 14, 14],
        &[20, 5, 101, 14, 14, 14],
    ],
};

const HD720: Size = Size {
    width: 320,
    height: 40,
    x_pad: 6,
    pad_bottom: 6,
    pad_top: 2,
    font: 17.0,
    border: 170,
    corner: &[
        &[0, 0, 160, 169],
        &[0, 165, 165, 169],
        &[160, 165, 56, 14],
        &[169, 169, 14, 14],
    ],
};

/// Which multiviewer resolutions the switcher can run, so which labels the
/// buffer holds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Resolutions {
    pub(super) uhd: bool,
    pub(super) hd1080: bool,
    pub(super) hd720: bool,
}

impl Resolutions {
    /// From the video modes a multiviewer runs in (by wire value, as in
    /// `atem_settings::VIDEO_MODES`). SD and 8K multiviewers take no label
    /// (atem-connection draws none for them).
    pub(super) fn add_mode(&mut self, mode: u8) {
        match mode {
            4 | 5 => self.hd720 = true,
            6..=13 | 26 | 27 => self.hd1080 = true,
            14..=19 => self.uhd = true,
            _ => {}
        }
    }

    fn sizes(self) -> Vec<&'static Size> {
        [
            (self.uhd, &UHD),
            (self.hd1080, &HD1080),
            (self.hd720, &HD720),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, s)| s)
        .collect()
    }

    pub(super) fn any(self) -> bool {
        self.uhd || self.hd1080 || self.hd720
    }

    /// The label buffer's width and height.
    pub(super) fn dimensions(self) -> (usize, usize) {
        let sizes = self.sizes();
        (
            sizes.first().map(|s| s.width).unwrap_or(0),
            sizes.iter().map(|s| s.height).sum(),
        )
    }
}

/// One character's coverage, placed relative to the pen.
struct Glyph {
    left: i32,
    top: i32,
    width: usize,
    height: usize,
    advance: f32,
    data: Vec<u8>,
}

/// The label bitmap for `text`: empty text gives a blank (all 0) label,
/// which clears it. Text too wide for the box is cut at the last character
/// that fits.
pub(super) fn render(text: &str, resolutions: Resolutions) -> Result<Vec<u8>, String> {
    let sizes = resolutions.sizes();
    let (width, height) = resolutions.dimensions();
    if sizes.is_empty() {
        return Err("the switcher reported no multiviewer resolution that takes a label".into());
    }
    let mut buffer = vec![0u8; width * height];
    if text.is_empty() {
        return Ok(buffer);
    }
    let font = FontRef::from_index(FONT, 0).ok_or("the embedded font does not load")?;
    let mut context = ScaleContext::new();
    let mut y_offset = 0;
    for size in sizes {
        draw(&mut context, font, &mut buffer, width, y_offset, size, text);
        y_offset += size.height;
    }
    debug_assert!(buffer.iter().all(|b| *b < 254));
    Ok(buffer)
}

fn glyph(context: &mut ScaleContext, font: FontRef, px: f32, c: char, at_x: f32) -> Glyph {
    let id = font.charmap().map(c);
    let advance = font.glyph_metrics(&[]).scale(px).advance_width(id);
    let mut scaler = context.builder(font).size(px).hint(false).build();
    let image = Render::new(&[Source::Outline])
        .format(Format::Alpha)
        .offset(Vector::new(at_x.fract(), 0.0))
        .render(&mut scaler, id);
    match image {
        Some(i) => Glyph {
            left: i.placement.left,
            top: i.placement.top,
            width: i.placement.width as usize,
            height: i.placement.height as usize,
            advance,
            data: i.data,
        },
        None => Glyph {
            left: 0,
            top: 0,
            width: 0,
            height: 0,
            advance,
            data: Vec::new(),
        },
    }
}

fn draw(
    context: &mut ScaleContext,
    font: FontRef,
    buffer: &mut [u8],
    buffer_width: usize,
    y_offset: usize,
    size: &Size,
    text: &str,
) {
    let px = size.font;
    // Keep the characters that fit, measuring each by its ink.
    let max = (size.width - size.x_pad * 2) as f32;
    let mut kept = String::new();
    let (mut advance, mut text_width) = (0.0f32, 0.0f32);
    for c in text.chars() {
        let g = glyph(context, font, px, c, 0.0);
        let ink = (g.left.max(0) as usize + g.width) as f32;
        if advance + ink > max {
            break;
        }
        text_width = advance + ink;
        advance += g.advance;
        kept.push(c);
    }

    let box_width = (text_width + (size.x_pad * 2) as f32).floor() as usize;
    let box_height = (px + (size.pad_top + size.pad_bottom) as f32).floor() as usize;
    let buffer_x = (buffer_width - size.width) / 2;
    let box_y = ((size.height as f32 - box_height as f32) / 2.5)
        .floor()
        .max(0.0) as usize
        + y_offset;
    let box_x = size.width.saturating_sub(box_width) / 2 + buffer_x;
    let mut line = |y: usize, x: usize, len: usize, value: u8| {
        let at = (box_y + y) * buffer_width + box_x + x;
        if let Some(row) = buffer.get_mut(at..at + len) {
            row.fill(value);
        }
    };

    // The box: a two-pixel border round the background.
    for y in 0..box_height {
        if y < 2 || y + 2 >= box_height {
            line(y, 0, box_width, size.border);
        } else {
            line(y, 0, 2, size.border);
            line(y, box_width - 2, 2, size.border);
            line(y, 2, box_width - 4, BACKGROUND);
        }
    }
    // Rounded corners, mirrored to the right and to the bottom.
    for (i, row) in size.corner.iter().enumerate() {
        let n = row.len();
        for y in [i, box_height - 1 - i] {
            let at = (box_y + y) * buffer_width + box_x;
            if let Some(left) = buffer.get_mut(at..at + n) {
                left.copy_from_slice(row);
            }
            let right_at = at + box_width - n;
            if let Some(right) = buffer.get_mut(right_at..right_at + n) {
                for (dst, src) in right.iter_mut().zip(row.iter().rev()) {
                    *dst = *src;
                }
            }
        }
    }

    // The text, on a baseline a font height below the box's top padding.
    let baseline = (box_y + size.pad_top) as f32 + px;
    let limit = (box_x + size.width + size.x_pad) as f32;
    let mut pen = (box_x + size.x_pad) as f32;
    for c in kept.chars() {
        let g = glyph(context, font, px, c, pen);
        let end = pen + g.advance;
        if end >= limit {
            break;
        }
        let left = pen.floor() as i64 + g.left as i64;
        let top = (baseline - g.top as f32).floor() as i64;
        for y in 0..g.height {
            for x in 0..g.width {
                let (bx, by) = (left + x as i64, top + y as i64);
                if bx < 0 || by < 0 || bx as usize >= buffer_width {
                    continue;
                }
                if let Some(px) = buffer.get_mut(by as usize * buffer_width + bx as usize) {
                    *px = PALETTE[g.data[y * g.width + x] as usize];
                }
            }
        }
        pen = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Resolutions {
        Resolutions {
            uhd: true,
            hd1080: true,
            hd720: true,
        }
    }

    #[test]
    fn the_buffer_stacks_a_label_per_resolution() {
        assert_eq!(all().dimensions(), (640, 190));
        let only_hd = Resolutions {
            hd1080: true,
            hd720: true,
            ..Default::default()
        };
        assert_eq!(only_hd.dimensions(), (320, 90));
        let mut r = Resolutions::default();
        for mode in [0, 4, 12, 20] {
            r.add_mode(mode);
        }
        assert_eq!(r, only_hd, "SD and 8K multiviewers take no label");
        assert!(render("x", Resolutions::default()).is_err());
    }

    #[test]
    fn a_label_is_a_bordered_box_holding_the_text_in_the_palette() {
        let r = all();
        let label = render("CAM 1", r).unwrap();
        assert_eq!(label.len(), 640 * 190);
        assert!(
            label.iter().all(|b| *b < 254),
            "no byte the switcher refuses"
        );
        // Every resolution's part holds a box and some text.
        for (y0, h, border) in [(0, 100, 0x05u8), (100, 50, 0x05), (150, 40, 170)] {
            let part = &label[y0 * 640..(y0 + h) * 640];
            assert!(part.contains(&border), "border at {y0}");
            assert!(part.contains(&BACKGROUND), "background at {y0}");
            let text = part
                .iter()
                .filter(|b| ![0, border, BACKGROUND].contains(*b))
                .count();
            assert!(text > 20, "text at {y0}: {text} pixels");
        }
        // Centered: the 1080 box's middle row is symmetric in extent.
        let row = &label[(100 + 25) * 640..(100 + 26) * 640];
        let first = row.iter().position(|b| *b != 0).unwrap();
        let last = row.iter().rposition(|b| *b != 0).unwrap();
        assert!(
            (first as i64 - (639 - last) as i64).abs() <= 2,
            "{first} {last}"
        );

        // Empty text clears the label.
        assert!(render("", r).unwrap().iter().all(|b| *b == 0));
        // A longer text is wider, up to the box's limit.
        let short = render("A", r).unwrap();
        let long = render(&"W".repeat(200), r).unwrap();
        let extent = |l: &[u8]| l[25 * 640..26 * 640].iter().filter(|b| **b != 0).count();
        assert!(extent(&long) > extent(&short));
        assert!(extent(&long) <= 640);
    }
}
