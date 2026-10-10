//! Geometry shared by the on-screen editor and the PNG exporter, so exports look like the app.

use crate::model::{ImageFit, Item, Settings, TierList, Zone};
use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};

pub fn item_size(settings: &Settings, item: Option<&Item>) -> Vec2 {
    let h = settings.item_size;
    let w = match settings.fit {
        ImageFit::Square => h,
        ImageFit::Original => h * item.map_or(1.0, |i| i.aspect).clamp(0.25, 4.0),
    };
    vec2(w, h)
}

/// Lays items out left-to-right, wrapping into rows. Rects are relative to the zone's top-left.
/// Returns the rects and the total height (0 when empty).
pub fn flow(width: f32, sizes: &[Vec2], gap: f32) -> (Vec<Rect>, f32) {
    let mut rects = Vec::with_capacity(sizes.len());
    let (mut x, mut y, mut row_h) = (gap, gap, 0.0f32);
    let max_w = (width - 2.0 * gap).max(8.0);
    for size in sizes {
        let size = vec2(size.x.min(max_w), size.y);
        if x > gap && x + size.x + gap > width {
            x = gap;
            y += row_h + gap;
            row_h = 0.0;
        }
        rects.push(Rect::from_min_size(pos2(x, y), size));
        x += size.x + gap;
        row_h = row_h.max(size.y);
    }
    let height = if sizes.is_empty() { 0.0 } else { y + row_h + gap };
    (rects, height)
}

pub fn zone_layout(list: &TierList, zone: Zone, width: f32) -> (Vec<String>, Vec<Rect>, f32) {
    let ids: Vec<String> = list.zone_items(zone).to_vec();
    let sizes: Vec<Vec2> = ids
        .iter()
        .map(|id| item_size(&list.settings, list.items.get(id)))
        .collect();
    let (rects, height) = flow(width, &sizes, list.settings.gap);
    (ids, rects, height)
}

pub struct Insertion {
    /// Index in the zone's item list (dragged item still included) to insert before.
    pub index: usize,
    /// Where to draw the insertion marker, relative to the zone's top-left.
    pub marker: Rect,
}

/// Figures out where a dragged item would land for pointer `p` (relative to the zone).
pub fn insertion(rects: &[Rect], p: Pos2, gap: f32, item_h: f32) -> Insertion {
    const BAR: f32 = 4.0;
    if rects.is_empty() {
        return Insertion {
            index: 0,
            marker: Rect::from_min_size(pos2(gap.max(2.0), gap), vec2(BAR, item_h)),
        };
    }
    // Group rects into rows: (first index, end index, top, bottom).
    let mut rows = Vec::new();
    let mut start = 0;
    for i in 1..=rects.len() {
        if i == rects.len() || rects[i].min.y != rects[start].min.y {
            let bottom = rects[start..i].iter().map(|r| r.max.y).fold(f32::MIN, f32::max);
            rows.push((start, i, rects[start].min.y, bottom));
            start = i;
        }
    }
    let &(s, e, top, bottom) = rows
        .iter()
        .find(|row| p.y < row.3 + gap * 0.5)
        .unwrap_or(rows.last().expect("rows is non-empty"));

    for i in s..e {
        if p.x < rects[i].center().x {
            let x = rects[i].min.x - gap * 0.5 - BAR * 0.5;
            return Insertion {
                index: i,
                marker: Rect::from_min_max(pos2(x, top), pos2(x + BAR, bottom)),
            };
        }
    }
    let x = rects[e - 1].max.x + gap * 0.5 - BAR * 0.5;
    Insertion {
        index: e,
        marker: Rect::from_min_max(pos2(x, top), pos2(x + BAR, bottom)),
    }
}

/// UV rect that crops a texture of `src_aspect` to fill a box of `dst_aspect` (center crop).
pub fn crop_uv(src_aspect: f32, dst_aspect: f32) -> Rect {
    if !(src_aspect > 0.0 && dst_aspect > 0.0) {
        return Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    }
    if src_aspect > dst_aspect {
        let w = dst_aspect / src_aspect;
        Rect::from_min_max(pos2(0.5 - w / 2.0, 0.0), pos2(0.5 + w / 2.0, 1.0))
    } else {
        let h = src_aspect / dst_aspect;
        Rect::from_min_max(pos2(0.0, 0.5 - h / 2.0), pos2(1.0, 0.5 + h / 2.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_wraps_rows() {
        let sizes = vec![vec2(10.0, 10.0); 5];
        let (rects, h) = flow(36.0, &sizes, 2.0);
        // 3 items fit per row: 2 + 10 + 2 + 10 + 2 + 10 + 2 = 38 > 36, so only 2 fit.
        assert_eq!(rects[2].min, pos2(2.0, 14.0));
        assert_eq!(h, 2.0 + 10.0 + 2.0 + 10.0 + 2.0 + 10.0 + 2.0);
    }

    #[test]
    fn insertion_picks_row_and_side() {
        let sizes = vec![vec2(10.0, 10.0); 4];
        let (rects, _) = flow(26.0, &sizes, 2.0); // two per row
        assert_eq!(insertion(&rects, pos2(1.0, 5.0), 2.0, 10.0).index, 0);
        assert_eq!(insertion(&rects, pos2(10.0, 5.0), 2.0, 10.0).index, 1);
        assert_eq!(insertion(&rects, pos2(25.0, 5.0), 2.0, 10.0).index, 2);
        assert_eq!(insertion(&rects, pos2(1.0, 18.0), 2.0, 10.0).index, 2);
        assert_eq!(insertion(&rects, pos2(25.0, 90.0), 2.0, 10.0).index, 4);
    }
}
