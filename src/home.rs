use crate::app::App;
use crate::dialogs::Dialog;
use crate::icons::Icon;
use crate::model::TierList;
use crate::storage;
use crate::theme::*;
use crate::util::{plural, time_ago};
use crate::widgets::*;
use eframe::egui::{
    self, pos2, text::LayoutJob, vec2, Align, Align2, Color32, CursorIcon, FontId, Id, Layout,
    Margin, Painter, Rect, RichText, ScrollArea, Sense, StrokeKind, Ui,
};

const CARD_W: f32 = 284.0;
const CARD_H: f32 = 246.0;

enum HomeAction {
    New,
    Import,
    Open(String),
    Rename(String),
    Duplicate(String),
    ExportPng(String),
    ExportPackage(String),
    Delete(String),
}

impl App {
    pub fn home_ui(&mut self, ctx: &egui::Context) {
        let mut action = None;
        let busy = self.busy_imports + self.busy_exports;
        egui::TopBottomPanel::bottom("home_footer")
            .frame(egui::Frame::new().fill(BG).inner_margin(Margin::symmetric(0, 10)))
            .show_separator_line(false)
            .show(ctx, credit);
        let frame = egui::Frame::new().fill(BG).inner_margin(Margin::symmetric(32, 26));
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add(Icon::Crown.image(30.0, ACCENT));
                ui.add_space(2.0);
                ui.label(RichText::new("Tier List Maker").size(26.0).strong().color(TEXT));
                if busy > 0 {
                    ui.add(egui::Spinner::new().size(16.0).color(ACCENT));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if primary_button(ui, Icon::Plus, "New tier list").clicked() {
                        action = Some(HomeAction::New);
                    }
                    if secondary_button(ui, Icon::FileUp, "Import")
                        .on_hover_text("Open a .tierlist file someone shared with you (or your own backup)")
                        .clicked()
                    {
                        action = Some(HomeAction::Import);
                    }
                });
            });
            ui.label(
                RichText::new("Your tier lists are saved automatically on this computer, so you can pick up where you left off.")
                    .color(TEXT_DIM),
            );
            ui.add_space(18.0);

            if self.lists.is_empty() {
                empty_state(ui, &mut action);
                return;
            }

            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let spacing = 16.0;
                let per_row = ((ui.available_width() + spacing) / (CARD_W + spacing)).floor().max(1.0) as usize;
                for row in self.lists.chunks(per_row) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = spacing;
                        for list in row {
                            if let Some(a) = list_card(ui, list) {
                                action = Some(a);
                            }
                        }
                    });
                    ui.add_space(spacing);
                }
            });
        });

        let Some(action) = action else { return };
        let find = |id: &str| self.lists.iter().find(|l| l.id == id).cloned();
        match action {
            HomeAction::New => self.open_new_list_dialog(Vec::new()),
            HomeAction::Import => self.pick_packages(),
            HomeAction::Open(id) => self.open_list(&id),
            HomeAction::Rename(id) => {
                if let Some(list) = find(&id) {
                    self.dialog = Some(Dialog::RenameList { id, name: list.name, focus: true });
                }
            }
            HomeAction::Duplicate(id) => {
                if let Some(list) = find(&id) {
                    match storage::duplicate_list(&list) {
                        Ok(copy) => {
                            self.toasts.success(format!("Created “{}”", copy.name));
                            self.reload_lists();
                        }
                        Err(err) => self.toasts.error(format!("Could not duplicate: {err}")),
                    }
                }
            }
            HomeAction::ExportPng(id) => self.export_png_by_id(&id),
            HomeAction::ExportPackage(id) => {
                if let Some(list) = find(&id) {
                    self.export_package(list);
                }
            }
            HomeAction::Delete(id) => {
                if let Some(list) = find(&id) {
                    self.dialog = Some(Dialog::DeleteList { id, name: list.name });
                }
            }
        }
    }
}

fn empty_state(ui: &mut Ui, action: &mut Option<HomeAction>) {
    ui.add_space(70.0);
    ui.vertical_centered(|ui| {
        ui.add(Icon::Crown.image(56.0, TEXT_FAINT));
        ui.add_space(12.0);
        ui.label(RichText::new("No tier lists yet").size(20.0).strong());
        ui.add_space(4.0);
        ui.label(RichText::new("Create one, add your images and drag them into tiers.").color(TEXT_DIM));
        ui.label(RichText::new("Tip: you can also drop images onto this window.").color(TEXT_FAINT));
        ui.add_space(16.0);
        if primary_button(ui, Icon::Plus, "Create your first tier list").clicked() {
            *action = Some(HomeAction::New);
        }
    });
}

fn list_card(ui: &mut Ui, list: &TierList) -> Option<HomeAction> {
    let mut action = None;
    let (rect, resp) = ui.allocate_exact_size(vec2(CARD_W, CARD_H), Sense::click());
    let hovered = ui.rect_contains_pointer(rect);
    let painter = ui.painter_at(rect.expand(2.0));
    painter.rect(
        rect,
        14.0,
        if hovered { CARD_HOVER } else { CARD },
        stroke(1.0, if hovered { Color32::from_rgb(72, 72, 82) } else { STROKE }),
        StrokeKind::Inside,
    );

    let inner = rect.shrink(12.0);
    let preview = Rect::from_min_size(inner.min, vec2(inner.width(), 142.0));
    draw_preview(&painter, preview, list);

    let mut job = LayoutJob::simple_singleline(list.name.clone(), FontId::proportional(16.0), TEXT);
    job.wrap = egui::text::TextWrapping::truncate_at_width(inner.width());
    let galley = painter.layout_job(job);
    painter.galley(pos2(inner.min.x, preview.max.y + 10.0), galley, TEXT);

    let meta = format!(
        "{} · {} · {}",
        plural(list.items.len(), "image", "images"),
        plural(list.tiers.len(), "tier", "tiers"),
        time_ago(list.modified)
    );
    painter.text(
        pos2(inner.min.x, preview.max.y + 34.0),
        Align2::LEFT_TOP,
        meta,
        FontId::proportional(12.0),
        TEXT_DIM,
    );

    // Progress: how many images are already placed.
    if !list.items.is_empty() {
        let done = list.ranked_count() as f32 / list.items.len() as f32;
        let bar = Rect::from_min_size(pos2(inner.min.x, inner.max.y - 41.0), vec2(inner.width(), 3.0));
        painter.rect_filled(bar, 2.0, Color32::from_rgb(50, 50, 56));
        painter.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * done, 3.0)), 2.0, ACCENT);
    }

    let b = 30.0;
    let y = inner.max.y - b;
    let buttons = [
        (Icon::Trash, "Delete", HomeAction::Delete(list.id.clone()), true),
        (Icon::Package, "Export .tierlist file", HomeAction::ExportPackage(list.id.clone()), false),
        (Icon::ImageDown, "Export PNG", HomeAction::ExportPng(list.id.clone()), false),
        (Icon::Copy, "Duplicate", HomeAction::Duplicate(list.id.clone()), false),
        (Icon::Pencil, "Rename", HomeAction::Rename(list.id.clone()), false),
    ];
    for (i, (icon, tip, a, danger)) in buttons.into_iter().enumerate() {
        let r = Rect::from_min_size(pos2(inner.max.x - b * (i as f32 + 1.0) - 2.0 * i as f32, y), vec2(b, b));
        if icon_button_at(ui, r, Id::new(("card_btn", &list.id, i)), icon, 16.0, tip, true, danger).clicked() {
            action = Some(a);
        }
    }
    let open_rect = Rect::from_min_size(pos2(inner.min.x, y), vec2(70.0, b));
    let open = ui.interact(open_rect, Id::new(("card_open", &list.id)), Sense::click());
    let open_color = if open.hovered() { ACCENT } else { TEXT };
    ui.painter().text(open_rect.left_center(), Align2::LEFT_CENTER, "Open →", FontId::proportional(14.0), open_color);
    if open.on_hover_cursor(CursorIcon::PointingHand).clicked() {
        action = Some(HomeAction::Open(list.id.clone()));
    }

    if action.is_none() && resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
        action = Some(HomeAction::Open(list.id.clone()));
    }
    action
}

/// Miniature of the tier list: colored labels and one square per placed image.
fn draw_preview(painter: &Painter, rect: Rect, list: &TierList) {
    painter.rect_filled(rect, 8.0, rgb(list.settings.border_color));
    let shown = list.tiers.len().min(6);
    if shown == 0 {
        painter.text(rect.center(), Align2::CENTER_CENTER, "No tiers", FontId::proportional(13.0), TEXT_FAINT);
        return;
    }
    let gap = 2.0;
    let row_h = (rect.height() - gap * (shown as f32 + 1.0)) / shown as f32;
    let label_w = 42.0;
    for (i, tier) in list.tiers.iter().take(shown).enumerate() {
        let y = rect.min.y + gap + i as f32 * (row_h + gap);
        let label = Rect::from_min_size(pos2(rect.min.x + gap, y), vec2(label_w, row_h));
        let content = Rect::from_min_max(pos2(label.max.x + gap, y), pos2(rect.max.x - gap, y + row_h));
        let (first, last) = (i == 0, i + 1 == shown);
        let radius_l = egui::CornerRadius {
            nw: if first { 6 } else { 0 },
            sw: if last { 6 } else { 0 },
            ..Default::default()
        };
        let radius_r = egui::CornerRadius {
            ne: if first { 6 } else { 0 },
            se: if last { 6 } else { 0 },
            ..Default::default()
        };
        painter.rect_filled(label, radius_l, rgb(tier.color));
        painter.rect_filled(content, radius_r, rgb(list.settings.row_color));

        let text: String = tier.label.lines().next().unwrap_or("").chars().take(3).collect();
        painter.text(
            label.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::proportional((row_h * 0.5).clamp(8.0, 14.0)),
            rgb(tier.text_color()),
        );

        let sq = (row_h - 6.0).max(3.0);
        let fit = ((content.width() - 3.0) / (sq + 3.0)).floor().max(0.0) as usize;
        for j in 0..tier.items.len().min(fit) {
            let r = Rect::from_min_size(pos2(content.min.x + 3.0 + j as f32 * (sq + 3.0), y + 3.0), vec2(sq, sq));
            painter.rect_filled(r, 2.0, Color32::from_white_alpha(38));
        }
    }
}
