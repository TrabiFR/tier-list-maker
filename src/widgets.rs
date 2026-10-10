use crate::icons::Icon;
use crate::theme::*;
use eframe::egui::{
    self, Color32, CursorIcon, Id, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2,
};

fn paint_icon_button(ui: &Ui, rect: Rect, resp: &Response, icon: Icon, icon_size: f32, enabled: bool, active: bool, danger: bool) {
    let hovered = enabled && resp.hovered();
    let pressed = enabled && resp.is_pointer_button_down_on();
    let bg = if pressed {
        Color32::from_rgb(64, 64, 72)
    } else if active {
        Color32::from_rgb(58, 58, 66)
    } else if hovered {
        Color32::from_rgb(48, 48, 55)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 7.0, bg);
    let color = if !enabled {
        TEXT_FAINT.gamma_multiply(0.55)
    } else if danger && hovered {
        DANGER
    } else if hovered || active {
        TEXT
    } else {
        TEXT_DIM
    };
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(icon_size));
    icon.image(icon_size, color).paint_at(ui, icon_rect);
}

pub fn icon_button(ui: &mut Ui, icon: Icon, tooltip: &str, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0), sense);
    paint_icon_button(ui, rect, &resp, icon, 18.0, enabled, false, false);
    with_cursor(resp, enabled).on_hover_text(tooltip)
}

pub fn toggle_icon_button(ui: &mut Ui, icon: Icon, tooltip: &str, active: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
    paint_icon_button(ui, rect, &resp, icon, 18.0, true, active, false);
    with_cursor(resp, true).on_hover_text(tooltip)
}

/// Icon button placed at an explicit rect (used inside custom-painted rows and cards).
#[allow(clippy::too_many_arguments)]
pub fn icon_button_at(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    icon: Icon,
    icon_size: f32,
    tooltip: &str,
    enabled: bool,
    danger: bool,
) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let resp = ui.interact(rect, id, sense);
    paint_icon_button(ui, rect, &resp, icon, icon_size, enabled, false, danger);
    with_cursor(resp, enabled).on_hover_text(tooltip)
}

fn with_cursor(resp: Response, enabled: bool) -> Response {
    if enabled {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    }
}

pub fn primary_button(ui: &mut Ui, icon: Icon, text: &str) -> Response {
    let button = egui::Button::image_and_text(
        icon.image(16.0, ACCENT_TEXT),
        RichText::new(text).color(ACCENT_TEXT).strong(),
    )
    .fill(ACCENT)
    .stroke(Stroke::NONE)
    .corner_radius(8.0)
    .min_size(egui::vec2(0.0, 32.0));
    ui.add(button).on_hover_cursor(CursorIcon::PointingHand)
}

pub fn secondary_button(ui: &mut Ui, icon: Icon, text: &str) -> Response {
    let button = egui::Button::image_and_text(icon.image(16.0, TEXT), RichText::new(text).color(TEXT))
        .corner_radius(8.0)
        .min_size(egui::vec2(0.0, 32.0));
    ui.add(button).on_hover_cursor(CursorIcon::PointingHand)
}

pub fn danger_button(ui: &mut Ui, icon: Icon, text: &str) -> Response {
    let button = egui::Button::image_and_text(
        icon.image(16.0, Color32::WHITE),
        RichText::new(text).color(Color32::WHITE).strong(),
    )
    .fill(Color32::from_rgb(190, 56, 56))
    .stroke(Stroke::NONE)
    .corner_radius(8.0)
    .min_size(egui::vec2(0.0, 32.0));
    ui.add(button).on_hover_cursor(CursorIcon::PointingHand)
}

pub fn plain_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Button::new(text).corner_radius(8.0).min_size(egui::vec2(0.0, 32.0)))
        .on_hover_cursor(CursorIcon::PointingHand)
}

pub fn menu_item(ui: &mut Ui, icon: Icon, text: &str, color: Color32) -> Response {
    ui.add(egui::Button::image_and_text(icon.image(15.0, color), RichText::new(text).color(color)))
}

pub fn dialog_title(ui: &mut Ui, icon: Icon, title: &str) {
    ui.horizontal(|ui| {
        ui.add(icon.image(20.0, ACCENT));
        ui.label(RichText::new(title).size(19.0).strong().color(TEXT));
    });
    ui.add_space(10.0);
}

pub const CREDIT: &str = "Made by MacFlunder";

/// The small "Made by MacFlunder" credit line.
pub fn credit(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(CREDIT).size(12.0).color(TEXT_FAINT));
    });
}

pub fn section_label(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text.to_uppercase()).size(11.0).strong().color(TEXT_FAINT));
    ui.add_space(2.0);
}

/// A clickable color swatch; `selected` draws a ring around it.
pub fn swatch(ui: &mut Ui, color: Color32, selected: bool, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_stroke(rect, 8.0, stroke(2.0, TEXT), StrokeKind::Inside);
        painter.rect_filled(rect.shrink(4.0), 5.0, color);
    } else {
        let r = if resp.hovered() { rect.shrink(1.0) } else { rect.shrink(2.0) };
        painter.rect_filled(r, 6.0, color);
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}
