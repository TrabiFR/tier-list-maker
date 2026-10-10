use crate::app::{App, Pending};
use crate::dialogs::Dialog;
use crate::icons::Icon;
use crate::layout::{crop_uv, insertion, zone_layout};
use crate::model::{ImageFit, Item, Settings, TierList, Zone};
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, pos2, text::LayoutJob, vec2, Align, Align2, Color32, CursorIcon, FontId, Id, LayerId,
    Layout, Margin, Order, Painter, Pos2, Rect, RichText, ScrollArea, Sense, Shape,
    StrokeKind, TextEdit, TextureHandle, Ui,
};
use std::collections::{HashMap, HashSet};

const BORDER: f32 = 2.0;
const CONTROLS_W: f32 = 34.0;
const MIN_ROW_H: f32 = 56.0;
/// Edits closer together than this are merged into one undo step (typing, slider drags).
const UNDO_GROUP_SECS: f64 = 1.0;

pub struct Editor {
    pub list: TierList,
    pub undo: Vec<TierList>,
    pub redo: Vec<TierList>,
    pub dirty: bool,
    pub changed_at: f64,
    group_at: f64,
    /// Set by one-shot actions (moves, deletes) so they always get their own undo step.
    pub discrete: bool,
    pub drag: Option<DragState>,
    pub show_settings: bool,
    pub hovered_item: Option<String>,
    /// Width of the tier table (without the row controls), used for WYSIWYG PNG exports.
    pub export_width: f32,
}

#[derive(Clone)]
pub struct DragState {
    pub item: String,
    pub grab: egui::Vec2,
    pub size: egui::Vec2,
}

pub enum Action {
    Back,
    AddImages,
    ExportPng,
    ExportPackage,
    ToggleSettings,
    EditTier(usize),
    AddTier(usize),
    MoveTier { from: usize, to: usize },
    RenameItem(String),
    DeleteItem(String),
    MoveItem { item: String, to: Zone, index: usize },
    ResetSettings,
    UnrankAll,
    SavePreset,
}

#[derive(Default)]
pub struct FrameOut {
    actions: Vec<Action>,
    drag_start: Option<DragState>,
    drop: Option<(Zone, usize)>,
    need_thumbs: Vec<String>,
    hovered: Option<String>,
}

impl Editor {
    pub fn new(list: TierList) -> Self {
        Self {
            list,
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
            changed_at: 0.0,
            group_at: f64::NEG_INFINITY,
            discrete: false,
            drag: None,
            show_settings: false,
            hovered_item: None,
            export_width: 1100.0,
        }
    }

    pub fn track_changes(&mut self, before: TierList, now: f64) {
        if self.list == before {
            self.discrete = false;
            return;
        }
        if self.discrete || now - self.group_at > UNDO_GROUP_SECS {
            self.undo.push(before);
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        self.group_at = if self.discrete { f64::NEG_INFINITY } else { now };
        self.discrete = false;
        self.dirty = true;
        self.changed_at = now;
    }

    pub fn undo(&mut self, now: f64) {
        if let Some(prev) = self.undo.pop() {
            let current = std::mem::replace(&mut self.list, prev);
            self.redo.push(current);
            self.after_history_jump(now);
        }
    }

    pub fn redo(&mut self, now: f64) {
        if let Some(next) = self.redo.pop() {
            let current = std::mem::replace(&mut self.list, next);
            self.undo.push(current);
            self.after_history_jump(now);
        }
    }

    fn after_history_jump(&mut self, now: f64) {
        self.drag = None;
        self.dirty = true;
        self.changed_at = now;
        self.group_at = f64::NEG_INFINITY;
    }
}

impl App {
    pub fn editor_ui(&mut self, ctx: &egui::Context) {
        let mut out = FrameOut::default();
        self.toolbar(ctx, &mut out);
        if self.editor.as_ref().is_some_and(|e| e.show_settings) {
            self.settings_panel(ctx, &mut out);
        }
        self.unranked_panel(ctx, &mut out);
        self.tiers_panel(ctx, &mut out);
        self.finish_drag(ctx, &mut out);

        for id in std::mem::take(&mut out.need_thumbs) {
            self.request_thumb(&id);
        }
        if let Some(ed) = self.editor.as_mut() {
            ed.hovered_item = out.hovered.take();
        }
        for action in out.actions {
            self.apply_action(action);
        }
    }

    fn apply_action(&mut self, action: Action) {
        match action {
            Action::Back => {
                self.close_editor();
                return;
            }
            Action::AddImages => {
                self.pick_images();
                return;
            }
            Action::ExportPng => {
                self.export_current_png();
                return;
            }
            Action::ExportPackage => {
                if let Some(list) = self.editor.as_ref().map(|e| e.list.clone()) {
                    self.export_package(list);
                }
                return;
            }
            Action::EditTier(tier) => {
                self.dialog = Some(Dialog::EditTier { tier, focus: true });
                return;
            }
            Action::SavePreset => {
                if let Some(ed) = &self.editor {
                    self.dialog = Some(Dialog::save_preset(&ed.list.name));
                }
                return;
            }
            Action::RenameItem(item) => {
                if let Some(name) = self.editor.as_ref().and_then(|e| e.list.items.get(&item)).map(|i| i.name.clone()) {
                    self.dialog = Some(Dialog::RenameItem { item, name, focus: true });
                }
                return;
            }
            _ => {}
        }

        let Some(ed) = self.editor.as_mut() else { return };
        ed.discrete = true;
        let list = &mut ed.list;
        match action {
            Action::ToggleSettings => {
                ed.discrete = false;
                ed.show_settings = !ed.show_settings;
            }
            Action::AddTier(at) => {
                let index = list.insert_tier(at);
                self.dialog = Some(Dialog::EditTier { tier: index, focus: true });
            }
            Action::MoveTier { from, to } => {
                if from < list.tiers.len() && to < list.tiers.len() {
                    list.tiers.swap(from, to);
                }
            }
            Action::DeleteItem(item) => list.remove_item(&item),
            Action::MoveItem { item, to, index } => list.move_item(&item, to, index),
            Action::ResetSettings => list.settings = Settings::default(),
            Action::UnrankAll => list.reset_all(),
            _ => {}
        }
    }

    // ---------------------------------------------------------------- toolbar

    fn toolbar(&mut self, ctx: &egui::Context, out: &mut FrameOut) {
        let (busy_imports, busy_exports) = (self.busy_imports, self.busy_exports);
        let mut pending = None;
        let Some(ed) = self.editor.as_mut() else { return };
        let frame = egui::Frame::new()
            .fill(PANEL)
            .inner_margin(Margin::symmetric(12, 8))
            .stroke(stroke(1.0, Color32::from_rgb(34, 34, 38)));
        egui::TopBottomPanel::top("editor_toolbar").frame(frame).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.set_min_height(34.0);
                if icon_button(ui, Icon::ArrowLeft, "Back to all tier lists", true).clicked() {
                    out.actions.push(Action::Back);
                }
                ui.add(
                    TextEdit::singleline(&mut ed.list.name)
                        .font(FontId::proportional(18.0))
                        .hint_text("Untitled tier list")
                        .desired_width(260.0)
                        .margin(Margin::symmetric(8, 5)),
                )
                .on_hover_text("Rename this tier list");

                let (icon, text) = if ed.dirty { (Icon::Loader, "Saving…") } else { (Icon::Check, "Saved") };
                ui.add(icon.image(14.0, TEXT_FAINT));
                ui.label(RichText::new(text).size(12.0).color(TEXT_FAINT))
                    .on_hover_text("Your work is saved automatically on this computer");

                if busy_imports > 0 || busy_exports > 0 {
                    ui.add(egui::Spinner::new().size(14.0).color(ACCENT));
                    let msg = if busy_imports > 0 {
                        format!("Adding {busy_imports}…")
                    } else {
                        "Exporting…".to_owned()
                    };
                    ui.label(RichText::new(msg).size(12.0).color(TEXT_DIM));
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if primary_button(ui, Icon::ImageDown, "Export PNG")
                        .on_hover_text("Save the tier list as an image")
                        .clicked()
                    {
                        out.actions.push(Action::ExportPng);
                    }
                    if icon_button(ui, Icon::Package, "Export as .tierlist file (backup or share with images)", true).clicked() {
                        out.actions.push(Action::ExportPackage);
                    }
                    if toggle_icon_button(ui, Icon::Sliders, "Customize sizes, colors and layout", ed.show_settings).clicked() {
                        out.actions.push(Action::ToggleSettings);
                    }
                    ui.separator();
                    if icon_button(ui, Icon::BookmarkPlus, "Save these tiers as a preset for new tier lists", true).clicked() {
                        out.actions.push(Action::SavePreset);
                    }
                    if secondary_button(ui, Icon::Plus, "Add tier").clicked() {
                        out.actions.push(Action::AddTier(ed.list.tiers.len()));
                    }
                    if secondary_button(ui, Icon::ImagePlus, "Add images").clicked() {
                        out.actions.push(Action::AddImages);
                    }
                    ui.separator();
                    let redo_tip = if cfg!(target_os = "macos") { "Redo (Cmd+Shift+Z)" } else { "Redo (Ctrl+Y)" };
                    let undo_tip = if cfg!(target_os = "macos") { "Undo (Cmd+Z)" } else { "Undo (Ctrl+Z)" };
                    if icon_button(ui, Icon::Redo, redo_tip, !ed.redo.is_empty()).clicked() {
                        pending = Some(Pending::Redo);
                    }
                    if icon_button(ui, Icon::Undo, undo_tip, !ed.undo.is_empty()).clicked() {
                        pending = Some(Pending::Undo);
                    }
                });
            });
        });
        if pending.is_some() {
            self.pending = pending;
            ctx.request_repaint();
        }
    }

    // ---------------------------------------------------------------- customize panel

    fn settings_panel(&mut self, ctx: &egui::Context, out: &mut FrameOut) {
        let Some(ed) = self.editor.as_mut() else { return };
        let frame = egui::Frame::new().fill(PANEL).inner_margin(Margin::symmetric(16, 12));
        egui::SidePanel::right("customize_panel")
            .resizable(false)
            .exact_width(300.0)
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(Icon::Palette.image(18.0, ACCENT));
                    ui.label(RichText::new("Customize").size(17.0).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon_button(ui, Icon::X, "Close", true).clicked() {
                            out.actions.push(Action::ToggleSettings);
                        }
                    });
                });
                ui.add_space(4.0);
                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let s = &mut ed.list.settings;
                    section_label(ui, "Images");
                    egui::Grid::new("img_settings").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                        ui.label("Size");
                        ui.add(egui::Slider::new(&mut s.item_size, 40.0..=240.0).suffix(" px").integer());
                        ui.end_row();
                        ui.label("Spacing");
                        ui.add(egui::Slider::new(&mut s.gap, 0.0..=16.0).suffix(" px").integer());
                        ui.end_row();
                        ui.label("Shape");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut s.fit, ImageFit::Square, "Square");
                            ui.selectable_value(&mut s.fit, ImageFit::Original, "Original");
                        });
                        ui.end_row();
                    });
                    ui.checkbox(&mut s.show_names, "Show image names");

                    section_label(ui, "Tiers");
                    egui::Grid::new("tier_settings").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                        ui.label("Label width");
                        ui.add(egui::Slider::new(&mut s.label_width, 50.0..=400.0).suffix(" px").integer());
                        ui.end_row();
                        ui.label("Label text");
                        ui.add(egui::Slider::new(&mut s.label_font_size, 10.0..=72.0).suffix(" px").integer());
                        ui.end_row();
                        ui.label("Row color");
                        egui::color_picker::color_edit_button_srgb(ui, &mut s.row_color);
                        ui.end_row();
                        ui.label("Lines");
                        egui::color_picker::color_edit_button_srgb(ui, &mut s.border_color);
                        ui.end_row();
                    });
                    ui.label(
                        RichText::new("Click a tier's label to change its text and color.")
                            .size(12.0)
                            .color(TEXT_FAINT),
                    );
                    ui.add_space(4.0);
                    if secondary_button(ui, Icon::BookmarkPlus, "Save tiers as preset…")
                        .on_hover_text("Reuse these tier names and colors when you create a new tier list")
                        .clicked()
                    {
                        out.actions.push(Action::SavePreset);
                    }

                    section_label(ui, "Export");
                    ui.checkbox(&mut s.export_title, "Show title on exported image");
                    ui.horizontal(|ui| {
                        ui.label("Quality");
                        for (label, value) in [("1×", 1.0), ("2×", 2.0), ("3×", 3.0)] {
                            ui.selectable_value(&mut s.export_scale, value, label);
                        }
                    });
                    ui.label(
                        RichText::new("The PNG uses the same width and wrapping you see here.")
                            .size(12.0)
                            .color(TEXT_FAINT),
                    );

                    section_label(ui, "Reset");
                    if secondary_button(ui, Icon::Inbox, "Move all images to Unranked").clicked() {
                        out.actions.push(Action::UnrankAll);
                    }
                    if secondary_button(ui, Icon::RotateCcw, "Reset these settings").clicked() {
                        out.actions.push(Action::ResetSettings);
                    }
                });
            });
    }

    // ---------------------------------------------------------------- unranked pool

    fn unranked_panel(&mut self, ctx: &egui::Context, out: &mut FrameOut) {
        let pointer = ctx.input(|i| i.pointer.latest_pos());
        let (textures, failed) = (&self.textures, &self.thumbs_failed);
        let Some(ed) = self.editor.as_mut() else { return };
        let frame = egui::Frame::new()
            .fill(PANEL)
            .inner_margin(Margin { left: 16, right: 16, top: 10, bottom: 12 })
            .stroke(stroke(1.0, Color32::from_rgb(34, 34, 38)));
        egui::TopBottomPanel::bottom("unranked_panel")
            .resizable(true)
            .default_height(210.0)
            .height_range(120.0..=640.0)
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(Icon::Inbox.image(18.0, TEXT_DIM));
                    ui.label(RichText::new("Unranked").size(15.0).strong());
                    ui.label(RichText::new(ed.list.unranked.len().to_string()).color(TEXT_FAINT));
                    ui.label(
                        RichText::new("Drag images into a tier · right-click an image for more options")
                            .size(12.0)
                            .color(TEXT_FAINT),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if secondary_button(ui, Icon::ImagePlus, "Add images").clicked() {
                            out.actions.push(Action::AddImages);
                        }
                    });
                });
                ui.add_space(6.0);
                let zone_rect = ui.available_rect_before_wrap();
                ScrollArea::vertical()
                    .id_salt("unranked_scroll")
                    .auto_shrink([false, false])
                    .drag_to_scroll(false)
                    .show(ui, |ui| {
                        let width = ui.available_width();
                        let (ids, rects, flow_h) = zone_layout(&ed.list, Zone::Unranked, width);
                        let height = flow_h.max(zone_rect.height() - 2.0);
                        let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
                        ui.painter().rect_filled(rect, 8.0, rgb(ed.list.settings.row_color));

                        if ids.is_empty() && ed.drag.is_none() {
                            let hint = if ed.list.items.is_empty() {
                                "Click here or drop images from your computer to get started"
                            } else {
                                "All images are ranked. Drop more images here to add them."
                            };
                            let center = Rect::from_center_size(rect.center() - vec2(0.0, 14.0), vec2(28.0, 28.0));
                            Icon::ImagePlus.image(28.0, TEXT_FAINT).paint_at(ui, center);
                            ui.painter().text(
                                rect.center() + vec2(0.0, 16.0),
                                Align2::CENTER_CENTER,
                                hint,
                                FontId::proportional(13.0),
                                TEXT_FAINT,
                            );
                            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                out.actions.push(Action::AddImages);
                            }
                        }

                        draw_items(ui, ed, textures, failed, Zone::Unranked, rect.min, &ids, &rects, out);

                        if let (Some(drag), Some(p)) = (&ed.drag, pointer) {
                            if zone_rect.contains(p) {
                                let s = &ed.list.settings;
                                let ins = insertion(&rects, p - rect.min.to_vec2(), s.gap, s.item_size);
                                let target = drop_index(&ed.list, drag, Zone::Unranked, ins.index);
                                out.drop = Some((Zone::Unranked, target));
                                ui.painter().rect_stroke(rect, 8.0, stroke(2.0, ACCENT.gamma_multiply(0.6)), StrokeKind::Inside);
                                if !is_noop(&ed.list, drag, Zone::Unranked, ins.index) {
                                    ui.painter().rect_filled(ins.marker.translate(rect.min.to_vec2()), 2.0, ACCENT);
                                }
                            }
                            auto_scroll(ui, p);
                        }
                    });
            });
    }

    // ---------------------------------------------------------------- tier rows

    fn tiers_panel(&mut self, ctx: &egui::Context, out: &mut FrameOut) {
        let pointer = ctx.input(|i| i.pointer.latest_pos());
        let (textures, failed) = (&self.textures, &self.thumbs_failed);
        let Some(ed) = self.editor.as_mut() else { return };
        let frame = egui::Frame::new().fill(BG).inner_margin(Margin::same(16));
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            ScrollArea::vertical()
                .id_salt("tiers_scroll")
                .auto_shrink([false, false])
                .drag_to_scroll(false)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    let width = ui.available_width();
                    ed.export_width = (width - CONTROLS_W - BORDER).max(240.0);

                    if ed.list.tiers.is_empty() {
                        ui.add_space(40.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("This tier list has no tiers yet.").color(TEXT_DIM));
                            ui.add_space(8.0);
                            if primary_button(ui, Icon::Plus, "Add a tier").clicked() {
                                out.actions.push(Action::AddTier(0));
                            }
                        });
                        return;
                    }

                    let left = ui.cursor().min.x;
                    let top = ui.cursor().min.y;
                    let background = ui.painter().add(Shape::Noop);
                    ui.allocate_exact_size(vec2(width, BORDER), Sense::hover());
                    for ti in 0..ed.list.tiers.len() {
                        tier_row(ui, ed, textures, failed, ti, width, pointer, out);
                        ui.allocate_exact_size(vec2(width, BORDER), Sense::hover());
                    }
                    let bottom = ui.cursor().min.y;
                    let table = Rect::from_min_max(pos2(left, top), pos2(left + width, bottom));
                    ui.painter().set(background, Shape::rect_filled(table, 0.0, rgb(ed.list.settings.border_color)));

                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.add_space((width - 130.0).max(0.0) / 2.0);
                        if secondary_button(ui, Icon::Plus, "Add tier").clicked() {
                            out.actions.push(Action::AddTier(ed.list.tiers.len()));
                        }
                    });
                    ui.add_space(18.0);
                    credit(ui);
                    ui.add_space(12.0);

                    if let (Some(_), Some(p)) = (&ed.drag, pointer) {
                        auto_scroll(ui, p);
                    }
                });
        });
    }

    // ---------------------------------------------------------------- drag & drop

    fn finish_drag(&mut self, ctx: &egui::Context, out: &mut FrameOut) {
        let Some(ed) = self.editor.as_mut() else { return };
        if let Some(start) = out.drag_start.take() {
            if ed.drag.is_none() {
                ed.drag = Some(start);
            }
        }
        let Some(drag) = ed.drag.clone() else { return };
        let (released, pointer, escape) = ctx.input(|i| {
            (!i.pointer.primary_down(), i.pointer.latest_pos(), i.key_pressed(egui::Key::Escape))
        });
        let item = ed.list.items.get(&drag.item).cloned();
        if escape || item.is_none() {
            ed.drag = None;
            return;
        }
        if released {
            ed.drag = None;
            if let Some((to, index)) = out.drop.take() {
                out.actions.push(Action::MoveItem { item: drag.item, to, index });
            }
            return;
        }

        ctx.set_cursor_icon(CursorIcon::Grabbing);
        if let (Some(p), Some(item)) = (pointer, item) {
            let rect = Rect::from_min_size(p - drag.grab, drag.size);
            let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("drag_ghost")));
            painter.rect_filled(rect.translate(vec2(0.0, 8.0)).expand(3.0), 8.0, Color32::from_black_alpha(110));
            paint_item(
                &painter,
                rect,
                &item,
                self.textures.get(&drag.item),
                self.thumbs_failed.contains(&drag.item),
                ed.list.settings.show_names,
                1.0,
            );
            painter.rect_stroke(rect, 0.0, stroke(2.0, ACCENT), StrokeKind::Outside);
        }
        ctx.request_repaint();
    }
}

/// Index to insert at once the dragged item has been removed from its current spot.
fn drop_index(list: &TierList, drag: &DragState, zone: Zone, index: usize) -> usize {
    match list.locate(&drag.item) {
        Some((from, i)) if from == zone && index > i => index - 1,
        _ => index,
    }
}

fn is_noop(list: &TierList, drag: &DragState, zone: Zone, index: usize) -> bool {
    matches!(list.locate(&drag.item), Some((from, i)) if from == zone && (index == i || index == i + 1))
}

fn auto_scroll(ui: &mut Ui, p: Pos2) {
    let clip = ui.clip_rect();
    if !clip.x_range().contains(p.x) {
        return;
    }
    const EDGE: f32 = 48.0;
    if p.y > clip.top() - 4.0 && p.y < clip.top() + EDGE {
        ui.scroll_with_delta(vec2(0.0, 12.0));
    } else if p.y < clip.bottom() + 4.0 && p.y > clip.bottom() - EDGE {
        ui.scroll_with_delta(vec2(0.0, -12.0));
    }
}

#[allow(clippy::too_many_arguments)]
fn tier_row(
    ui: &mut Ui,
    ed: &Editor,
    textures: &HashMap<String, TextureHandle>,
    failed: &HashSet<String>,
    ti: usize,
    width: f32,
    pointer: Option<Pos2>,
    out: &mut FrameOut,
) {
    let s = &ed.list.settings;
    let tier = &ed.list.tiers[ti];
    let zone = Zone::Tier(ti);
    let label_w = s.label_width.min(width * 0.45).max(40.0);
    let content_w = (width - label_w - CONTROLS_W - 3.0 * BORDER).max(40.0);
    let (ids, rects, flow_h) = zone_layout(&ed.list, zone, content_w);

    let text_color = rgb(tier.text_color());
    let mut job = LayoutJob::simple(
        tier.label.clone(),
        FontId::proportional(s.label_font_size),
        text_color,
        (label_w - 12.0).max(10.0),
    );
    job.halign = Align::Center;
    let galley = ui.fonts(|f| f.layout_job(job));

    let row_h = flow_h
        .max(s.item_size + 2.0 * s.gap)
        .max(galley.size().y + 16.0)
        .max(MIN_ROW_H);
    let (row_rect, _) = ui.allocate_exact_size(vec2(width, row_h), Sense::hover());
    let label_rect = Rect::from_min_size(row_rect.min + vec2(BORDER, 0.0), vec2(label_w, row_h));
    let content_rect = Rect::from_min_size(pos2(label_rect.max.x + BORDER, row_rect.min.y), vec2(content_w, row_h));
    let controls_rect = Rect::from_min_max(pos2(content_rect.max.x + BORDER, row_rect.min.y), row_rect.max);

    if !ui.is_rect_visible(row_rect) && ed.drag.is_none() {
        return;
    }

    // Label cell.
    let painter = ui.painter();
    painter.rect_filled(label_rect, 0.0, rgb(tier.color));
    let label_resp = ui.interact(label_rect, Id::new(("tier_label", &tier.id)), Sense::click());
    if label_resp.hovered() && ed.drag.is_none() {
        painter.rect_filled(label_rect, 0.0, Color32::from_black_alpha(22));
        let pencil = Rect::from_min_size(label_rect.right_top() + vec2(-18.0, 5.0), vec2(13.0, 13.0));
        Icon::Pencil.image(13.0, text_color.gamma_multiply(0.7)).paint_at(ui, pencil);
    }
    let text_pos = label_rect.center() - galley.rect.center().to_vec2();
    ui.painter_at(label_rect).galley(text_pos, galley, text_color);
    if label_resp
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text("Edit tier text and color")
        .clicked()
    {
        out.actions.push(Action::EditTier(ti));
    }

    // Items.
    ui.painter().rect_filled(content_rect, 0.0, rgb(s.row_color));
    draw_items(ui, ed, textures, failed, zone, content_rect.min, &ids, &rects, out);

    if let (Some(drag), Some(p)) = (&ed.drag, pointer) {
        let within = row_rect.contains(p) && ui.clip_rect().contains(p);
        if within && !controls_rect.contains(p) {
            let (index, marker) = if label_rect.contains(p) {
                (ids.len(), None)
            } else {
                let ins = insertion(&rects, p - content_rect.min.to_vec2(), s.gap, s.item_size);
                (ins.index, Some(ins.marker.translate(content_rect.min.to_vec2())))
            };
            out.drop = Some((zone, drop_index(&ed.list, drag, zone, index)));
            ui.painter().rect_stroke(content_rect, 0.0, stroke(2.0, ACCENT.gamma_multiply(0.6)), StrokeKind::Inside);
            if let Some(marker) = marker {
                if !is_noop(&ed.list, drag, zone, index) {
                    ui.painter().rect_filled(marker, 2.0, ACCENT);
                }
            }
        }
    }

    // Row controls: settings, move up, move down.
    ui.painter().rect_filled(controls_rect, 0.0, Color32::from_rgb(10, 10, 11));
    let b = ((row_h - 8.0) / 3.0).clamp(16.0, 26.0);
    let x = controls_rect.center().x - b / 2.0;
    let y = controls_rect.center().y - 1.5 * b;
    let icon_size = (b * 0.66).round();
    let n = ed.list.tiers.len();
    let buttons = [
        (Icon::Settings, "Edit tier", true, Action::EditTier(ti)),
        (Icon::ChevronUp, "Move tier up", ti > 0, Action::MoveTier { from: ti, to: ti.saturating_sub(1) }),
        (Icon::ChevronDown, "Move tier down", ti + 1 < n, Action::MoveTier { from: ti, to: ti + 1 }),
    ];
    for (i, (icon, tip, enabled, action)) in buttons.into_iter().enumerate() {
        let rect = Rect::from_min_size(pos2(x, y + i as f32 * b), vec2(b, b));
        let id = Id::new(("tier_ctl", &tier.id, i));
        if icon_button_at(ui, rect, id, icon, icon_size, tip, enabled, false).clicked() {
            out.actions.push(action);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_items(
    ui: &mut Ui,
    ed: &Editor,
    textures: &HashMap<String, TextureHandle>,
    failed: &HashSet<String>,
    zone: Zone,
    origin: Pos2,
    ids: &[String],
    rects: &[Rect],
    out: &mut FrameOut,
) {
    let s = &ed.list.settings;
    let dragging = ed.drag.as_ref().map(|d| d.item.as_str());
    for (id, local) in ids.iter().zip(rects) {
        let rect = local.translate(origin.to_vec2());
        let Some(item) = ed.list.items.get(id) else { continue };
        if !ui.is_rect_visible(rect) {
            continue;
        }
        let resp = ui.interact(rect, Id::new(("item", id)), Sense::click_and_drag());
        let texture = textures.get(id);
        if texture.is_none() && !failed.contains(id) {
            out.need_thumbs.push(id.clone());
        }
        let is_dragged = dragging == Some(id.as_str());
        paint_item(ui.painter(), rect, item, texture, failed.contains(id), s.show_names, if is_dragged { 0.25 } else { 1.0 });

        if dragging.is_none() && resp.hovered() {
            ui.painter().rect_stroke(rect, 0.0, stroke(2.0, Color32::from_white_alpha(210)), StrokeKind::Inside);
            out.hovered = Some(id.clone());
        }
        if resp.drag_started() {
            let press = ui.input(|i| i.pointer.press_origin()).unwrap_or(rect.center());
            out.drag_start = Some(DragState {
                item: id.clone(),
                grab: press - rect.min,
                size: rect.size(),
            });
        }
        let resp = if dragging.is_none() && !s.show_names {
            resp.on_hover_text(&item.name).on_hover_cursor(CursorIcon::Grab)
        } else {
            resp
        };
        resp.context_menu(|ui| item_menu(ui, ed, id, zone, out));
    }
}

fn item_menu(ui: &mut Ui, ed: &Editor, id: &str, zone: Zone, out: &mut FrameOut) {
    ui.set_min_width(190.0);
    if menu_item(ui, Icon::Pencil, "Rename…", TEXT).clicked() {
        out.actions.push(Action::RenameItem(id.to_owned()));
        ui.close_menu();
    }
    ui.menu_button("Move to", |ui| {
        ui.set_min_width(160.0);
        for (ti, tier) in ed.list.tiers.iter().enumerate() {
            let mut label = tier.label.lines().next().unwrap_or("").trim().to_owned();
            if label.is_empty() {
                label = format!("Tier {}", ti + 1);
            }
            if label.chars().count() > 24 {
                label = label.chars().take(23).collect::<String>() + "…";
            }
            let button = egui::Button::image_and_text(Icon::Swatch.image(13.0, rgb(tier.color)), label);
            if ui.add_enabled(zone != Zone::Tier(ti), button).clicked() {
                out.actions.push(Action::MoveItem { item: id.to_owned(), to: Zone::Tier(ti), index: usize::MAX });
                ui.close_menu();
            }
        }
        ui.separator();
        if ui.add_enabled(zone != Zone::Unranked, egui::Button::new("Unranked")).clicked() {
            out.actions.push(Action::MoveItem { item: id.to_owned(), to: Zone::Unranked, index: usize::MAX });
            ui.close_menu();
        }
    });
    ui.separator();
    if menu_item(ui, Icon::Trash, "Delete image", DANGER).clicked() {
        out.actions.push(Action::DeleteItem(id.to_owned()));
        ui.close_menu();
    }
}

pub fn paint_item(
    painter: &Painter,
    rect: Rect,
    item: &Item,
    texture: Option<&TextureHandle>,
    failed: bool,
    show_name: bool,
    opacity: f32,
) {
    match texture {
        Some(tex) => {
            let [w, h] = tex.size();
            let uv = crop_uv(w as f32 / h.max(1) as f32, rect.width() / rect.height().max(1.0));
            painter.image(tex.id(), rect, uv, Color32::WHITE.gamma_multiply(opacity));
        }
        None => {
            painter.rect_filled(rect, 0.0, Color32::from_gray(40).gamma_multiply(opacity));
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                if failed { "!" } else { "…" },
                FontId::proportional(18.0),
                TEXT_FAINT.gamma_multiply(opacity),
            );
        }
    }
    if show_name && rect.height() >= 30.0 {
        let strip = Rect::from_min_max(pos2(rect.min.x, rect.max.y - 18.0), rect.max);
        painter.rect_filled(strip, 0.0, Color32::from_black_alpha((170.0 * opacity) as u8));
        let mut job = LayoutJob::simple_singleline(item.name.clone(), FontId::proportional(11.0), Color32::WHITE.gamma_multiply(opacity));
        job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 6.0);
        let galley = painter.layout_job(job);
        let pos = strip.center() - galley.rect.center().to_vec2();
        painter.with_clip_rect(rect).galley(pos, galley, Color32::WHITE);
    }
}
