use image::{imageops::FilterType, DynamicImage, GenericImage, GenericImageView, Rgba};
use imagequant::RGBA;
use std::fmt::Display;

static BLACK: [u8; 4] = [0, 0, 0, 255];

struct Match {
    pub codepoint: u8,
    pub fg: u8,
    pub bg: u8,
}

pub struct Codepoint {
    pub width: u32,
    pub height: u32,
    bytes: Vec<[u8; 4]>,
}

enum FontType {
    IBMVGAType,
    VGA50Type,
}

pub struct Font {
    pub width: u32,
    pub height: u32,
    size: u32,
    bitmask: Vec<u8>,
    font_type: FontType,
}

impl Font {
    fn with_bytes(bytes: Vec<u8>, font_type: FontType) -> Self {
        let width = 8;
        let height = (bytes.len() / 256) as u32;
        let size = width * height;
        let bitmask = bytes
            .iter()
            .flat_map(|byte| {
                (0..8)
                    .rev()
                    .map(move |i| if byte & (1 << i) != 0 { 1 } else { 0 })
            })
            .collect();
        Self {
            width,
            height,
            size,
            bitmask,
            font_type,
        }
    }

    pub fn ibm_vga() -> Self {
        let bytes = include_bytes!("../fonts/CP437.F16").to_vec();
        Self::with_bytes(bytes, FontType::IBMVGAType)
    }

    pub fn vga50() -> Self {
        let bytes = include_bytes!("../fonts/CP437.F08").to_vec();
        Self::with_bytes(bytes, FontType::VGA50Type)
    }

    fn bits_for_codepoint(&self, codepoint: u8) -> impl Iterator<Item = &u8> {
        let start = codepoint as u32 * self.size;
        let end = start + self.size;
        self.bitmask[start as usize..end as usize].iter()
    }

    pub fn render_codepoint(&self, codepoint: u8, fg: [u8; 4], bg: Option<[u8; 4]>) -> Codepoint {
        let bytes = self
            .bits_for_codepoint(codepoint)
            .map(|bit| if *bit == 1 { fg } else { bg.unwrap_or(BLACK) })
            .collect();
        Codepoint {
            width: self.width,
            height: self.height,
            bytes,
        }
    }

    pub fn draw_codepoint(&self, img: &mut DynamicImage, codepoint: &Codepoint, x: u32, y: u32) {
        for (i, byte) in codepoint.bytes.iter().enumerate() {
            let x = x + (i as u32 % codepoint.width);
            let y = y + (i as u32 / codepoint.width);
            let pixel = Rgba::from(*byte);
            img.put_pixel(x, y, pixel);
        }
    }

    fn find_closest_bitmask(&self, other: &[u8]) -> Match {
        let mut best = Match {
            codepoint: 0,
            fg: 0,
            bg: 0,
        };
        let mut best_count = 0;
        for codepoint in 0..=255 {
            if codepoint == 9
                || codepoint == 10
                || codepoint == 13
                || codepoint == 26
                || codepoint == 27
            {
                continue;
            }
            let count: u32 = self
                .bits_for_codepoint(codepoint)
                .zip(other.iter())
                .map(|(a, b)| if *a == *b { 1 } else { 0 })
                .sum();
            if count > best_count {
                best.codepoint = codepoint;
                best.fg = 1;
                best.bg = 0;
                best_count = count;
            }
            let inverse_count: u32 = self
                .bits_for_codepoint(codepoint)
                .zip(other.iter())
                .map(|(a, b)| if *a == *b { 0 } else { 1 })
                .sum();
            if inverse_count > best_count {
                best.codepoint = codepoint;
                best.fg = 0;
                best.bg = 1;
                best_count = count;
            }
        }
        best
    }
}

impl Display for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.font_type)
    }
}

impl Display for FontType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontType::IBMVGAType => write!(f, "IBM VGA"),
            FontType::VGA50Type => write!(f, "IBM VGA50"),
        }
    }
}

pub struct TextSection {
    pub pixels: Vec<u8>,
    pub palette: Vec<[u8; 4]>,
    pub width: u32,
    pub char_height: u32,
    pub columns: u32,
    pub rows: u32,
    pub row: u32,
    pub column: u32,
}

pub struct TextSectionIterator {
    image: DynamicImage,
    width: u32,
    height: u32,
    columns: u32,
    rows: u32,
    row: u32,
    column: u32,
}

impl Iterator for TextSectionIterator {
    type Item = TextSection;

    fn next(&mut self) -> Option<Self::Item> {
        if self.row >= self.rows {
            return None;
        }
        let x = self.column * self.width;
        let y = self.row * self.height;
        let image = self.image.crop_imm(x, y, self.width, self.height);
        let pixels: Vec<RGBA> = image
            .pixels()
            .map(|(_, _, pixel)| RGBA {
                r: pixel[0],
                g: pixel[1],
                b: pixel[2],
                a: 255,
            })
            .collect();
        let mut liq = imagequant::new();
        liq.set_speed(1).expect("liq speed");
        liq.set_max_colors(2).expect("liq mac colors");
        let mut img = liq
            .new_image(&pixels[..], self.width as usize, self.height as usize, 0.0)
            .expect("liq image");
        let mut res = liq.quantize(&mut img).expect("quantize");
        let (palette, pixels) = res.remapped(&mut img).expect("remapped");
        let palette = palette
            .iter()
            .map(|rgb| [rgb.r, rgb.g, rgb.b, 255])
            .collect();
        let current_column = self.column;
        let current_row = self.row;
        self.column += 1;
        if self.column >= self.columns {
            self.column = 0;
            self.row += 1;
        }
        Some(TextSection {
            palette,
            pixels,
            width: self.width,
            char_height: self.height,
            columns: self.columns,
            rows: self.rows,
            row: current_row,
            column: current_column,
        })
    }
}

pub trait AsTextSections {
    fn calculate_rows(&self, columns: u32, width: u32, height: u32) -> u32;
    fn as_text_sections(&self, columns: u32, width: u32, height: u32) -> TextSectionIterator;
}

impl AsTextSections for DynamicImage {
    fn calculate_rows(&self, columns: u32, width: u32, height: u32) -> u32 {
        let (img_width, img_height) = self.dimensions();
        let target_width = width * columns;
        let scale = target_width as f32 / img_width as f32;
        (scale * img_height as f32 / height as f32).ceil() as u32
    }

    fn as_text_sections(&self, columns: u32, width: u32, height: u32) -> TextSectionIterator {
        let rows = self.calculate_rows(columns, width, height);
        let image = self.resize_exact(width * columns, height * rows, FilterType::Lanczos3);
        TextSectionIterator {
            image,
            width,
            height,
            columns,
            rows,
            row: 0,
            column: 0,
        }
    }
}

pub struct Block {
    pub fg: [u8; 4],
    pub bg: Option<[u8; 4]>,
    pub codepoint: u8,
    pub column: u32,
    pub row: u32,
}

pub fn convert_image(image: &DynamicImage, font: &Font, columns: u32) -> Vec<Block> {
    image
        .as_text_sections(columns, font.width, font.height)
        .map(|section| {
            if section.palette.len() == 1 {
                Block {
                    fg: section.palette[0],
                    bg: None,
                    codepoint: 219,
                    column: section.column,
                    row: section.row,
                }
            } else {
                let best = font.find_closest_bitmask(&section.pixels);
                Block {
                    fg: section.palette[best.fg as usize],
                    bg: Some(section.palette[best.bg as usize]),
                    codepoint: best.codepoint,
                    column: section.column,
                    row: section.row,
                }
            }
        })
        .collect()
}
