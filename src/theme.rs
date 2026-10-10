use crate::model::Rgb;
use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    TextStyle,
};
use std::sync::Arc;

pub const BG: Color32 = Color32::from_rgb(17, 17, 19);
pub const PANEL: Color32 = Color32::from_rgb(24, 24, 27);
pub const CARD: Color32 = Color32::from_rgb(30, 30, 34);
pub const CARD_HOVER: Color32 = Color32::from_rgb(37, 37, 42);
pub const STROKE: Color32 = Color32::from_rgb(50, 50, 57);
pub const TEXT: Color32 = Color32::from_rgb(236, 236, 240);
pub const TEXT_DIM: Color32 = Color32::from_rgb(160, 160, 170);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(112, 112, 122);
pub const ACCENT: Color32 = Color32::from_rgb(255, 127, 127);
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(40, 16, 16);
pub const DANGER: Color32 = Color32::from_rgb(255, 99, 99);
pub const SUCCESS: Color32 = Color32::from_rgb(134, 222, 150);

pub fn rgb(c: Rgb) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// A regular system font (closer to the classic tier list look than egui's default),
/// shared by the UI and the PNG exporter.
pub struct SystemFont {
    pub bytes: Vec<u8>,
    pub index: u32,
}

pub fn load_system_font() -> Option<SystemFont> {
    #[cfg(target_os = "macos")]
    const CANDIDATES: &[(&str, u32)] = &[
        ("/System/Library/Fonts/Helvetica.ttc", 0),
        ("/System/Library/Fonts/Supplemental/Arial.ttf", 0),
        ("/Library/Fonts/Arial.ttf", 0),
    ];
    #[cfg(target_os = "windows")]
    const CANDIDATES: &[(&str, u32)] = &[
        ("C:\\Windows\\Fonts\\segoeui.ttf", 0),
        ("C:\\Windows\\Fonts\\arial.ttf", 0),
    ];
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    const CANDIDATES: &[(&str, u32)] = &[
        ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
        ("/usr/share/fonts/TTF/DejaVuSans.ttf", 0),
    ];

    CANDIDATES.iter().find_map(|(path, index)| {
        let bytes = std::fs::read(path).ok()?;
        // Make sure the font actually parses before handing it to egui.
        ab_glyph::FontRef::try_from_slice_and_index(&bytes, *index).ok()?;
        Some(SystemFont { bytes, index: *index })
    })
}

/// Font used for PNG exports; falls back to egui's bundled font.
pub fn export_font(system: Option<&SystemFont>) -> ab_glyph::FontArc {
    if let Some(f) = system {
        if let Ok(font) = ab_glyph::FontVec::try_from_vec_and_index(f.bytes.clone(), f.index) {
            return ab_glyph::FontArc::new(font);
        }
    }
    let defaults = FontDefinitions::default();
    let data = defaults
        .font_data
        .get("Ubuntu-Light")
        .or_else(|| defaults.font_data.values().next())
        .expect("egui ships default fonts");
    let font = ab_glyph::FontVec::try_from_vec(data.font.to_vec()).expect("valid bundled font");
    ab_glyph::FontArc::new(font)
}

pub fn apply(ctx: &egui::Context, system: Option<&SystemFont>) {
    let mut fonts = FontDefinitions::default();
    if let Some(f) = system {
        let mut data = FontData::from_owned(f.bytes.clone());
        data.index = f.index;
        fonts.font_data.insert("system".to_owned(), Arc::new(data));
        if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
            family.insert(0, "system".to_owned());
        }
    }
    ctx.set_fonts(fonts);

    ctx.style_mut(|style| {
        style.visuals = egui::Visuals::dark();
        let v = &mut style.visuals;
        v.panel_fill = PANEL;
        v.window_fill = Color32::from_rgb(28, 28, 32);
        v.window_stroke = stroke(1.0, STROKE);
        v.window_corner_radius = CornerRadius::same(14);
        v.menu_corner_radius = CornerRadius::same(10);
        v.extreme_bg_color = Color32::from_rgb(14, 14, 16);
        v.faint_bg_color = CARD;
        v.selection.bg_fill = Color32::from_rgb(130, 62, 62);
        v.selection.stroke = stroke(1.5, ACCENT);
        v.hyperlink_color = ACCENT;

        let radius = CornerRadius::same(7);
        v.widgets.noninteractive.bg_stroke = stroke(1.0, STROKE);
        v.widgets.noninteractive.fg_stroke = stroke(1.0, TEXT);
        v.widgets.noninteractive.corner_radius = radius;
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(42, 42, 48);
        v.widgets.inactive.bg_fill = Color32::from_rgb(48, 48, 55);
        v.widgets.inactive.fg_stroke = stroke(1.0, TEXT);
        v.widgets.inactive.corner_radius = radius;
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(54, 54, 62);
        v.widgets.hovered.bg_fill = Color32::from_rgb(60, 60, 68);
        v.widgets.hovered.bg_stroke = stroke(1.0, Color32::from_rgb(80, 80, 90));
        v.widgets.hovered.corner_radius = radius;
        v.widgets.active.weak_bg_fill = Color32::from_rgb(64, 64, 72);
        v.widgets.active.bg_fill = Color32::from_rgb(70, 70, 80);
        v.widgets.active.corner_radius = radius;
        v.widgets.open.corner_radius = radius;

        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.menu_margin = Margin::same(6);
        style.spacing.slider_width = 150.0;
        style.text_styles = [
            (TextStyle::Heading, FontId::proportional(22.0)),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (TextStyle::Small, FontId::proportional(11.5)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
    });
}

pub fn stroke(width: f32, color: Color32) -> Stroke {
    Stroke::new(width, color)
}
