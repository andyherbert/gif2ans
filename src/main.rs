mod converter;
use clap::Parser;
use converter::{convert_image, get_cga_color, Block, Font};
use image::DynamicImage;
use std::error;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

static SAUCE_BYTES: &[u8; 129] = include_bytes!("./sauce.bin");

fn convert_blocks_to_ans(
    blocks: &Vec<Block>,
    font: &Font,
    columns: u32,
    truecolor: bool,
) -> Vec<u8> {
    let mut ans: Vec<u8> = Vec::new();
    for block in blocks {
        ans.append("\x1b[0;".as_bytes().to_vec().as_mut());
        if let Some(bg) = block.cga_bg {
            let bg_string = if bg >= 8 {
                format!("5;{};", 40 + bg % 8)
            } else {
                format!("{};", 40 + bg)
            };
            ans.append(bg_string.as_bytes().to_vec().as_mut());
        }
        let fg_string = if block.cga_fg >= 8 {
            format!("1;{}m", 30 + block.cga_fg % 8)
        } else {
            format!("{}m", 30 + block.cga_fg)
        };
        ans.append(fg_string.as_bytes().to_vec().as_mut());
        if truecolor {
            if let Some(bg) = block.bg {
                let bg_string = format!("\x1b[0;{};{};{}t", bg[0], bg[1], bg[2]);
                ans.append(bg_string.as_bytes().to_vec().as_mut());
            }
            let fg = block.fg;
            let fg_string = format!("\x1b[1;{};{};{}t", fg[0], fg[1], fg[2]);
            ans.append(fg_string.as_bytes().to_vec().as_mut());
        }
        ans.push(block.codepoint);
    }
    let mut sauce = SAUCE_BYTES.to_vec();
    sauce[91..95].copy_from_slice((ans.len() as u32).to_le_bytes().as_ref());
    sauce[97..99].copy_from_slice((columns as u16).to_le_bytes().as_ref());
    let rows = (blocks.len() as u32) / columns;
    sauce[99..=100].copy_from_slice((rows as u16).to_le_bytes().as_ref());
    let font_string = font.to_string();
    sauce[107..(107 + font_string.len())].copy_from_slice(font_string.as_bytes());
    ans.append(&mut sauce);
    ans
}

fn convert_blocks_to_image(
    blocks: &Vec<Block>,
    font: &Font,
    columns: u32,
    truecolor: bool,
) -> DynamicImage {
    let rows = (blocks.len() as u32) / columns;
    let mut image = DynamicImage::new_rgba8(columns * font.width, rows * font.height);
    for block in blocks {
        let (fg, bg) = if truecolor {
            (block.fg, block.bg)
        } else {
            (get_cga_color(block.cga_fg), block.cga_bg.map(get_cga_color))
        };
        let codepoint = font.render_codepoint(block.codepoint, fg, bg);
        font.draw_codepoint(
            &mut image,
            &codepoint,
            block.column * codepoint.width,
            block.row * codepoint.height,
        );
    }
    image
}

#[derive(Parser)]
#[clap(version, about, long_about = None)]
struct Cli {
    /// Use 8x8 font
    #[clap(long, action, value_name = "Defaults to 8x16")]
    vga50: bool,
    /// Number of columns
    #[clap(long, value_name = "1 to 65535", default_value = "80")]
    columns: u16,
    /// Generates an PNG image file
    #[clap(long, action, value_name = "Output an image file")]
    image: bool,
    /// Use 24-bit color
    #[clap(
        long,
        action,
        default_value = "false",
        value_name = "Defaults to CGA colors"
    )]
    truecolor: bool,
    #[clap(value_name = "INPUT")]
    input: PathBuf,
    #[clap(value_name = "OUTPUT")]
    output: PathBuf,
}

fn convert(cli: Cli) -> Result<(), Box<dyn error::Error>> {
    let path = Path::new(&cli.input);
    let image = image::open(path)?;
    let font = if cli.vga50 {
        Font::vga50()
    } else {
        Font::ibm_vga()
    };
    let blocks = convert_image(&image, &font, cli.columns as u32);
    let bytes = convert_blocks_to_ans(&blocks, &font, cli.columns as u32, cli.truecolor);
    let mut out_path = PathBuf::from(&cli.output);
    let file = File::create(&out_path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(&bytes)?;
    writer.flush()?;
    println!("Wrote {:?}", out_path);
    if cli.image {
        let image = convert_blocks_to_image(&blocks, &font, cli.columns as u32, cli.truecolor);
        out_path.set_extension("ans.png");
        image.save(&out_path)?;
        println!("Wrote {:?}", out_path);
    }
    Ok(())
}

fn main() {
    if let Err(error) = convert(Cli::parse()) {
        eprintln!("Error: {}", error);
    }
}
