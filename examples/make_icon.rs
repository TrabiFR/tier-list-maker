//! Renders `assets/app-icon.svg` into the icon files used for packaging,
//! plus the side image of the Windows installer (`installer/welcome.bmp`).
//!
//! ```text
//! cargo run --example make_icon                 # assets/app-icon.png + assets/app-icon.ico
//! cargo run --example make_icon -- <dir>.iconset  # also writes a macOS iconset
//! ```

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::ExtendedColorType;
use std::path::{Path, PathBuf};

fn render(tree: &resvg::usvg::Tree, size: u32) -> Vec<u8> {
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("valid size");
    let scale = size as f32 / tree.size().width();
    resvg::render(tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap.encode_png().expect("png encoding")
}

/// 164x314 sidebar for the installer's welcome/finish pages: dark gradient, app icon, tier bars.
fn write_installer_bitmap(root: &Path, icon: &resvg::usvg::Tree) -> Result<(), Box<dyn std::error::Error>> {
    let colors = ["#FF7F7F", "#FFBF7F", "#FFDF7F", "#FFFF7F", "#BFFF7F"];
    let bars: String = colors
        .iter()
        .enumerate()
        .map(|(i, c)| {
            format!(
                r#"<rect x="26" y="{}" width="{}" height="11" rx="3" fill="{c}"/>"#,
                214 + i * 17,
                112 - i * 20
            )
        })
        .collect();
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="164" height="314" viewBox="0 0 164 314">
  <defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="#2c2c33"/><stop offset="1" stop-color="#121215"/>
  </linearGradient></defs>
  <rect width="164" height="314" fill="url(#g)"/>{bars}
</svg>"##
    );
    let tree = resvg::usvg::Tree::from_data(svg.as_bytes(), &resvg::usvg::Options::default())?;
    let mut side = image::load_from_memory(&render_size(&tree, 164, 314))?.to_rgba8();
    let logo = image::load_from_memory(&render(icon, 136))?.to_rgba8();
    image::imageops::overlay(&mut side, &logo, 14, 48);
    let rgb = image::DynamicImage::ImageRgba8(side).to_rgb8();
    let dir = root.join("installer");
    std::fs::create_dir_all(&dir)?;
    rgb.save(dir.join("welcome.bmp"))?;
    Ok(())
}

fn render_size(tree: &resvg::usvg::Tree, w: u32, h: u32) -> Vec<u8> {
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h).expect("valid size");
    let scale = w as f32 / tree.size().width();
    resvg::render(tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap.encode_png().expect("png encoding")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let svg = std::fs::read(root.join("assets/app-icon.svg"))?;
    let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default())?;

    std::fs::write(root.join("assets/app-icon.png"), render(&tree, 512))?;

    let frames = [16u32, 24, 32, 48, 64, 128, 256]
        .iter()
        .map(|&s| {
            let png = image::load_from_memory(&render(&tree, s))?.to_rgba8();
            IcoFrame::as_png(png.as_raw(), s, s, ExtendedColorType::Rgba8)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let ico = std::fs::File::create(root.join("assets/app-icon.ico"))?;
    IcoEncoder::new(ico).encode_images(&frames)?;

    write_installer_bitmap(root, &tree)?;

    if let Some(dir) = std::env::args().nth(1).map(PathBuf::from) {
        std::fs::create_dir_all(&dir)?;
        for size in [16u32, 32, 128, 256, 512] {
            std::fs::write(dir.join(format!("icon_{size}x{size}.png")), render(&tree, size))?;
            std::fs::write(dir.join(format!("icon_{size}x{size}@2x.png")), render(&tree, size * 2))?;
        }
        println!("Wrote iconset to {}", dir.display());
    }
    println!("Wrote assets/app-icon.png, assets/app-icon.ico and installer/welcome.bmp");
    Ok(())
}
