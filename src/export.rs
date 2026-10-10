//! Renders a tier list to a PNG, using the same layout rules as the editor.

use crate::jobs::decode_file;
use crate::layout::{flow, item_size};
use crate::model::{Rgb, TierList};
use crate::storage;
use ab_glyph::{point, Font, FontArc, GlyphId, PxScale, ScaleFont};
use anyhow::Result;
use eframe::egui::{Rect, Vec2};
use image::{imageops, Rgba, RgbaImage};
use std::collections::HashMap;
use std::path::Path;

const BORDER: f32 = 2.0;
const TITLE_BG: Rgb = [17, 17, 19];

pub fn export_png(list: &TierList, path: &Path, width: f32, font: &FontArc) -> Result<()> {
    let img = render(list, &storage::list_dir(&list.id), width, font);
    img.save_with_format(path, image::ImageFormat::Png)?;
    Ok(())
}

struct Row {
    y: f32,
    h: f32,
    rects: Vec<Rect>,
}

/// `dir` is the list's folder (containing `images/` and `thumbs/`).
pub fn render(list: &TierList, dir: &Path, width: f32, font: &FontArc) -> RgbaImage {
    let s = &list.settings;
    let k = s.export_scale.clamp(1.0, 4.0);
    let label_w = s.label_width;
    let width = width.max(label_w + s.item_size + 4.0 * BORDER + 2.0 * s.gap);
    let content_w = width - label_w - 3.0 * BORDER;
    let label_px = s.label_font_size * k;

    let show_title = s.export_title && !list.name.trim().is_empty();
    let title_px = 30.0 * k;
    let title_lines = wrap_text(font, title_px, list.name.trim(), (width - 48.0) * k);
    let title_h = if show_title {
        text_block_height(font, title_px, title_lines.len()) / k + 36.0
    } else {
        0.0
    };

    // Layout pass.
    let mut rows = Vec::with_capacity(list.tiers.len());
    let mut y = title_h + BORDER;
    for tier in &list.tiers {
        let sizes: Vec<Vec2> = tier
            .items
            .iter()
            .map(|id| item_size(s, list.items.get(id)))
            .collect();
        let (rects, flow_h) = flow(content_w, &sizes, s.gap);
        let lines = wrap_text(font, label_px, &tier.label, (label_w - 12.0) * k);
        let text_h = text_block_height(font, label_px, lines.len()) / k + 16.0;
        let h = flow_h.max(s.item_size + 2.0 * s.gap).max(text_h);
        rows.push(Row { y, h, rects });
        y += h + BORDER;
    }
    let height = y.max(title_h + 2.0 * BORDER);

    let px = |v: f32| (v * k).round() as i64;
    let mut img = RgbaImage::from_pixel(
        px(width).max(1) as u32,
        px(height).max(1) as u32,
        rgba(s.border_color),
    );

    if show_title {
        fill(&mut img, 0, 0, px(width), px(title_h), TITLE_BG);
        draw_text_block(
            &mut img,
            font,
            title_px,
            &title_lines,
            (px(width) as f32 / 2.0, px(title_h) as f32 / 2.0),
            [240, 240, 240],
        );
    }

    let mut cache: HashMap<&str, Option<image::DynamicImage>> = HashMap::new();
    for (tier, row) in list.tiers.iter().zip(&rows) {
        let (y0, y1) = (px(row.y), px(row.y + row.h));
        let (lx0, lx1) = (px(BORDER), px(BORDER + label_w));
        fill(&mut img, lx0, y0, lx1, y1, tier.color);
        let lines = wrap_text(font, label_px, &tier.label, (label_w - 12.0) * k);
        draw_text_block(
            &mut img,
            font,
            label_px,
            &lines,
            ((lx0 + lx1) as f32 / 2.0, (y0 + y1) as f32 / 2.0),
            tier.text_color(),
        );

        let cx = 2.0 * BORDER + label_w;
        fill(&mut img, px(cx), y0, px(cx + content_w), y1, s.row_color);

        for (id, rect) in tier.items.iter().zip(&row.rects) {
            let (x0, ry0) = (px(cx + rect.min.x), px(row.y + rect.min.y));
            let (x1, ry1) = (px(cx + rect.max.x), px(row.y + rect.max.y));
            let (w, h) = ((x1 - x0).max(1) as u32, (ry1 - ry0).max(1) as u32);
            let source = cache.entry(id.as_str()).or_insert_with(|| {
                let item = list.items.get(id)?;
                decode_file(&dir.join("images").join(&item.file))
                    .or_else(|_| decode_file(&dir.join("thumbs").join(&item.thumb)))
                    .ok()
            });
            match source {
                Some(src) => {
                    let tile = src
                        .resize_to_fill(w, h, imageops::FilterType::Lanczos3)
                        .to_rgba8();
                    imageops::overlay(&mut img, &tile, x0, ry0);
                }
                None => fill(&mut img, x0, ry0, x1, ry1, [60, 60, 60]),
            }
            if s.show_names {
                if let Some(item) = list.items.get(id) {
                    let name_px = (11.0 * k).max(10.0);
                    let strip = (name_px * 1.6) as i64;
                    darken(&mut img, x0, ry1 - strip, x1, ry1, 0.65);
                    let line = truncate_to_width(font, name_px, &item.name, (w as f32) - 6.0);
                    draw_text_block(
                        &mut img,
                        font,
                        name_px,
                        &[line],
                        ((x0 + x1) as f32 / 2.0, (ry1 - strip / 2) as f32),
                        [245, 245, 245],
                    );
                }
            }
        }
    }
    img
}

fn rgba(c: Rgb) -> Rgba<u8> {
    Rgba([c[0], c[1], c[2], 255])
}

fn fill(img: &mut RgbaImage, x0: i64, y0: i64, x1: i64, y1: i64, color: Rgb) {
    let (w, h) = (img.width() as i64, img.height() as i64);
    for y in y0.max(0)..y1.min(h) {
        for x in x0.max(0)..x1.min(w) {
            img.put_pixel(x as u32, y as u32, rgba(color));
        }
    }
}

fn darken(img: &mut RgbaImage, x0: i64, y0: i64, x1: i64, y1: i64, amount: f32) {
    let (w, h) = (img.width() as i64, img.height() as i64);
    for y in y0.max(0)..y1.min(h) {
        for x in x0.max(0)..x1.min(w) {
            let p = img.get_pixel_mut(x as u32, y as u32);
            for c in 0..3 {
                p[c] = (p[c] as f32 * (1.0 - amount)) as u8;
            }
        }
    }
}

fn measure(font: &FontArc, px: f32, text: &str) -> f32 {
    let sf = font.as_scaled(PxScale::from(px));
    let mut width = 0.0;
    let mut prev: Option<GlyphId> = None;
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        if let Some(p) = prev {
            width += sf.kern(p, id);
        }
        width += sf.h_advance(id);
        prev = Some(id);
    }
    width
}

fn line_height(font: &FontArc, px: f32) -> f32 {
    let sf = font.as_scaled(PxScale::from(px));
    sf.ascent() - sf.descent() + sf.line_gap()
}

fn text_block_height(font: &FontArc, px: f32, lines: usize) -> f32 {
    line_height(font, px) * lines.max(1) as f32
}

/// Word-wraps `text` (respecting explicit newlines), breaking overly long words.
fn wrap_text(font: &FontArc, px: f32, text: &str, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if measure(font, px, &candidate) <= max_w {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            // The word alone may still be too wide: split it by characters.
            for ch in word.chars() {
                let mut next = line.clone();
                next.push(ch);
                if !line.is_empty() && measure(font, px, &next) > max_w {
                    lines.push(std::mem::take(&mut line));
                    line.push(ch);
                } else {
                    line = next;
                }
            }
        }
        lines.push(line);
    }
    lines
}

fn truncate_to_width(font: &FontArc, px: f32, text: &str, max_w: f32) -> String {
    if measure(font, px, text) <= max_w {
        return text.to_owned();
    }
    let mut out = String::new();
    for ch in text.chars() {
        let candidate = format!("{out}{ch}…");
        if measure(font, px, &candidate) > max_w {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

/// Draws lines centered horizontally and vertically around `center` (in pixels).
fn draw_text_block(
    img: &mut RgbaImage,
    font: &FontArc,
    px: f32,
    lines: &[String],
    center: (f32, f32),
    color: Rgb,
) {
    let sf = font.as_scaled(PxScale::from(px));
    let lh = line_height(font, px);
    let top = center.1 - lh * lines.len() as f32 / 2.0;
    for (i, line) in lines.iter().enumerate() {
        let x = center.0 - measure(font, px, line) / 2.0;
        let baseline = top + i as f32 * lh + sf.ascent();
        draw_line(img, font, px, line, x, baseline, color);
    }
}

fn draw_line(img: &mut RgbaImage, font: &FontArc, px: f32, text: &str, x: f32, baseline: f32, color: Rgb) {
    let scale = PxScale::from(px);
    let sf = font.as_scaled(scale);
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mut caret = x;
    let mut prev: Option<GlyphId> = None;
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        if let Some(p) = prev {
            caret += sf.kern(p, id);
        }
        let glyph = id.with_scale_and_position(scale, point(caret, baseline));
        caret += sf.h_advance(id);
        prev = Some(id);
        let Some(outlined) = font.outline_glyph(glyph) else { continue };
        let bounds = outlined.px_bounds();
        outlined.draw(|gx, gy, coverage| {
            let px = bounds.min.x as i32 + gx as i32;
            let py = bounds.min.y as i32 + gy as i32;
            if px < 0 || py < 0 || px >= w || py >= h {
                return;
            }
            let a = coverage.clamp(0.0, 1.0);
            let p = img.get_pixel_mut(px as u32, py as u32);
            for c in 0..3 {
                p[c] = (p[c] as f32 * (1.0 - a) + color[c] as f32 * a).round() as u8;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{new_id, Item, Template, Zone};

    /// Renders a demo list; the PNG path is printed so it can be inspected by eye.
    #[test]
    fn renders_demo_list() {
        let dir = std::env::temp_dir().join(format!("tierlist-export-test-{}", new_id()));
        std::fs::create_dir_all(dir.join("images")).unwrap();
        let mut list = TierList::new("My favourite colours", Template::Classic);
        list.tiers[1].label = "Really good stuff".into();
        for i in 0..14u32 {
            let (w, h) = (160 + i * 25, 160);
            let img = RgbaImage::from_fn(w, h, |x, y| {
                Rgba([(x * 255 / w) as u8, (y * 255 / h) as u8, (i * 18) as u8, 255])
            });
            let id = new_id();
            img.save(dir.join("images").join(format!("{id}.png"))).unwrap();
            list.add_item(Item {
                id: id.clone(),
                name: format!("Image {i}"),
                file: format!("{id}.png"),
                thumb: format!("{id}.png"),
                aspect: w as f32 / h as f32,
            });
            list.move_item(&id, Zone::Tier((i % 4) as usize), usize::MAX);
        }
        list.settings.show_names = true;
        let font = crate::theme::export_font(crate::theme::load_system_font().as_ref());
        let img = render(&list, &dir, 700.0, &font);
        assert_eq!(img.width(), 1400);
        let out = std::env::temp_dir().join("tierlist-export-demo.png");
        img.save(&out).unwrap();
        println!("demo export written to {}", out.display());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
