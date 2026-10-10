use crate::app::App;
use crate::icons::Icon;
use crate::model::{auto_text_color, now_secs, Rgb, StartWith, Template, PALETTE};
use crate::storage;
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, pos2, text::LayoutJob, vec2, Align, CursorIcon, FontId, Id, Key, Layout, Modal,
    Rect, RichText, Sense, StrokeKind, TextEdit, Ui,
};
use std::path::PathBuf;

pub enum Dialog {
    NewList { name: String, start: StartWith, images: Vec<PathBuf>, focus: bool },
    SavePreset { name: String, include_settings: bool, focus: bool },
    RenameList { id: String, name: String, focus: bool },
    DeleteList { id: String, name: String },
    EditTier { tier: usize, focus: bool },
    RenameItem { item: String, name: String, focus: bool },
}

impl Dialog {
    pub fn new_list(images: Vec<PathBuf>, start: StartWith) -> Self {
        Dialog::NewList { name: String::new(), start, images, focus: true }
    }

    pub fn save_preset(list_name: &str) -> Self {
        Dialog::SavePreset { name: list_name.trim().to_owned(), include_settings: true, focus: true }
    }
}

fn enter_pressed(ui: &Ui) -> bool {
    ui.input(|i| i.key_pressed(Key::Enter))
}

/// Right-aligned confirm/cancel row. Returns (confirmed, cancelled).
fn buttons_row(ui: &mut Ui, confirm: &str, icon: Icon, danger: bool) -> (bool, bool) {
    let mut result = (false, false);
    ui.add_space(14.0);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let confirm_resp = if danger {
            danger_button(ui, icon, confirm)
        } else {
            primary_button(ui, icon, confirm)
        };
        result.0 = confirm_resp.clicked();
        result.1 = plain_button(ui, "Cancel").clicked();
    });
    result
}

impl App {
    pub fn dialogs_ui(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialog.take() else { return };
        let keep = match &mut dialog {
            Dialog::NewList { name, start, images, focus } => self.new_list_dialog(ctx, name, start, images, focus),
            Dialog::SavePreset { name, include_settings, focus } => self.save_preset_dialog(ctx, name, include_settings, focus),
            Dialog::RenameList { id, name, focus } => self.rename_list_dialog(ctx, id, name, focus),
            Dialog::DeleteList { id, name } => self.delete_list_dialog(ctx, id, name),
            Dialog::EditTier { tier, focus } => self.edit_tier_dialog(ctx, *tier, focus),
            Dialog::RenameItem { item, name, focus } => self.rename_item_dialog(ctx, item, name, focus),
        };
        // A dialog may have opened another one (e.g. "add tier below" edits the new tier).
        if keep && self.dialog.is_none() {
            self.dialog = Some(dialog);
        }
    }

    fn new_list_dialog(
        &mut self,
        ctx: &egui::Context,
        name: &mut String,
        start: &mut StartWith,
        images: &mut Vec<PathBuf>,
        focus: &mut bool,
    ) -> bool {
        let (mut create, mut cancel) = (false, false);
        let mut delete_preset = None;
        let presets = &self.presets;
        let modal = Modal::new(Id::new("new_list_dialog")).show(ctx, |ui| {
            ui.set_width(470.0);
            dialog_title(ui, Icon::Plus, "New tier list");
            ui.label(RichText::new("Name").color(TEXT_DIM));
            let resp = ui.add(
                TextEdit::singleline(name)
                    .hint_text("e.g. Best pizza toppings")
                    .desired_width(f32::INFINITY)
                    .margin(egui::Margin::symmetric(8, 6)),
            );
            if std::mem::take(focus) {
                resp.request_focus();
            }
            if resp.lost_focus() && enter_pressed(ui) {
                create = true;
            }
            if !presets.is_empty() {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add(Icon::Bookmark.image(14.0, TEXT_DIM));
                    ui.label(RichText::new("Your presets").color(TEXT_DIM));
                });
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                    for preset in presets {
                        let colors: Vec<Rgb> = preset.tiers.iter().map(|t| t.color).collect();
                        let selected = *start == StartWith::Preset(preset.id.clone());
                        let card = start_card(ui, &preset.name, &preset.summary(), &colors, selected);
                        let trash = Rect::from_min_size(card.rect.right_bottom() - vec2(28.0, 28.0), vec2(24.0, 24.0));
                        let delete = ui.rect_contains_pointer(card.rect)
                            && icon_button_at(ui, trash, Id::new(("del_preset", &preset.id)), Icon::Trash, 14.0, "Delete preset", true, true)
                                .clicked();
                        if delete {
                            delete_preset = Some(preset.id.clone());
                        } else if card.clicked() {
                            *start = StartWith::Preset(preset.id.clone());
                        }
                    }
                });
            }
            ui.add_space(10.0);
            ui.label(RichText::new(if presets.is_empty() { "Start with" } else { "Templates" }).color(TEXT_DIM));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                for t in Template::ALL {
                    let colors: Vec<Rgb> = t.tiers().iter().map(|(_, c)| *c).collect();
                    let selected = *start == StartWith::Template(t);
                    if start_card(ui, t.name(), &t.summary(), &colors, selected).clicked() {
                        *start = StartWith::Template(t);
                    }
                }
            });
            ui.label(
                RichText::new("You can rename, recolor, add and remove tiers at any time. Save your own setup with the bookmark button in the editor.")
                    .size(12.0)
                    .color(TEXT_FAINT),
            );
            if !images.is_empty() {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add(Icon::ImagePlus.image(15.0, SUCCESS));
                    ui.label(format!("{} will be added to Unranked.", crate::util::plural(images.len(), "image", "images")));
                });
            }
            let (ok, no) = buttons_row(ui, "Create", Icon::Check, false);
            create |= ok;
            cancel |= no;
        });
        if let Some(id) = delete_preset {
            if *start == StartWith::Preset(id.clone()) {
                *start = StartWith::Template(Template::Classic);
            }
            self.delete_preset(&id);
        }
        if create {
            let images = std::mem::take(images);
            self.last_start = start.clone();
            self.create_list(name, start, images);
            return false;
        }
        !(cancel || modal.should_close())
    }

    fn save_preset_dialog(&mut self, ctx: &egui::Context, name: &mut String, include_settings: &mut bool, focus: &mut bool) -> bool {
        let Some(ed) = &self.editor else { return false };
        let (mut save, mut cancel) = (false, false);
        let replaces = self.presets.iter().any(|p| p.name.eq_ignore_ascii_case(name.trim()));
        let modal = Modal::new(Id::new("save_preset_dialog")).show(ctx, |ui| {
            ui.set_width(440.0);
            dialog_title(ui, Icon::BookmarkPlus, "Save tiers as preset");
            ui.label(
                RichText::new("Saves the tier names and colors (not the images) so you can start new tier lists with them.")
                    .color(TEXT_DIM),
            );
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                for tier in &ed.list.tiers {
                    let label = tier.label.lines().next().unwrap_or("").trim();
                    let label = if label.is_empty() { " " } else { label };
                    let text = RichText::new(label).color(rgb(tier.text_color())).size(13.0);
                    ui.add(egui::Button::new(text).fill(rgb(tier.color)).sense(Sense::hover()).corner_radius(5.0));
                }
            });
            ui.add_space(10.0);
            ui.label(RichText::new("Preset name").color(TEXT_DIM));
            let resp = ui.add(
                TextEdit::singleline(name)
                    .hint_text("e.g. My ranking tiers")
                    .desired_width(f32::INFINITY)
                    .margin(egui::Margin::symmetric(8, 6)),
            );
            if std::mem::take(focus) {
                resp.request_focus();
            }
            if resp.lost_focus() && enter_pressed(ui) {
                save = true;
            }
            if replaces {
                ui.label(RichText::new("A preset with this name exists and will be replaced.").size(12.0).color(ACCENT));
            }
            ui.add_space(4.0);
            ui.checkbox(include_settings, "Also save the layout (image size, label width, row colors…)");
            let (ok, no) = buttons_row(ui, "Save preset", Icon::BookmarkPlus, false);
            save |= ok;
            cancel |= no;
        });
        if save {
            self.save_preset(name, *include_settings);
            return false;
        }
        !(cancel || modal.should_close())
    }

    fn rename_list_dialog(&mut self, ctx: &egui::Context, id: &str, name: &mut String, focus: &mut bool) -> bool {
        let (mut save, mut cancel) = (false, false);
        let modal = Modal::new(Id::new("rename_list_dialog")).show(ctx, |ui| {
            ui.set_width(400.0);
            dialog_title(ui, Icon::Pencil, "Rename tier list");
            let resp = ui.add(TextEdit::singleline(name).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(8, 6)));
            if std::mem::take(focus) {
                resp.request_focus();
            }
            if resp.lost_focus() && enter_pressed(ui) {
                save = true;
            }
            let (ok, no) = buttons_row(ui, "Save", Icon::Check, false);
            save |= ok;
            cancel |= no;
        });
        if save {
            if let Some(list) = self.lists.iter_mut().find(|l| l.id == id) {
                list.name = if name.trim().is_empty() { "Untitled tier list".into() } else { name.trim().to_owned() };
                list.modified = now_secs();
                if let Err(err) = storage::save_list(list) {
                    self.toasts.error(format!("Could not rename: {err}"));
                }
            }
            self.reload_lists();
            return false;
        }
        !(cancel || modal.should_close())
    }

    fn delete_list_dialog(&mut self, ctx: &egui::Context, id: &str, name: &str) -> bool {
        let (mut delete, mut cancel) = (false, false);
        let modal = Modal::new(Id::new("delete_list_dialog")).show(ctx, |ui| {
            ui.set_width(420.0);
            dialog_title(ui, Icon::Trash, "Delete tier list?");
            ui.label(format!("“{name}” and its images will be removed from this computer."));
            ui.label(
                RichText::new("This can't be undone. Tip: export it as a .tierlist file first if you want a backup.")
                    .color(TEXT_DIM),
            );
            let (ok, no) = buttons_row(ui, "Delete", Icon::Trash, true);
            delete |= ok;
            cancel |= no;
        });
        if delete {
            match storage::delete_list(id) {
                Ok(()) => self.toasts.success(format!("Deleted “{name}”")),
                Err(err) => self.toasts.error(format!("Could not delete: {err}")),
            }
            self.reload_lists();
            return false;
        }
        !(cancel || modal.should_close())
    }

    fn rename_item_dialog(&mut self, ctx: &egui::Context, item: &str, name: &mut String, focus: &mut bool) -> bool {
        let (mut save, mut cancel) = (false, false);
        let modal = Modal::new(Id::new("rename_item_dialog")).show(ctx, |ui| {
            ui.set_width(380.0);
            dialog_title(ui, Icon::Pencil, "Rename image");
            let resp = ui.add(TextEdit::singleline(name).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(8, 6)));
            if std::mem::take(focus) {
                resp.request_focus();
            }
            if resp.lost_focus() && enter_pressed(ui) {
                save = true;
            }
            let (ok, no) = buttons_row(ui, "Save", Icon::Check, false);
            save |= ok;
            cancel |= no;
        });
        if save {
            if let Some(ed) = self.editor.as_mut() {
                if let Some(it) = ed.list.items.get_mut(item) {
                    ed.discrete = true;
                    it.name = name.trim().to_owned();
                }
            }
            return false;
        }
        !(cancel || modal.should_close())
    }

    fn edit_tier_dialog(&mut self, ctx: &egui::Context, index: usize, focus: &mut bool) -> bool {
        enum TierAction {
            AddAbove,
            AddBelow,
            Clear,
            Delete,
        }
        let Some(ed) = self.editor.as_mut() else { return false };
        if index >= ed.list.tiers.len() {
            return false;
        }
        let mut done = false;
        let mut action = None;
        let modal = Modal::new(Id::new("edit_tier_dialog")).show(ctx, |ui| {
            ui.set_width(440.0);
            let tier = &mut ed.list.tiers[index];
            dialog_title(ui, Icon::Settings, "Edit tier");

            ui.horizontal_top(|ui| {
                // Live preview of the label cell.
                let (rect, _) = ui.allocate_exact_size(vec2(96.0, 96.0), Sense::hover());
                ui.painter().rect_filled(rect, 8.0, rgb(tier.color));
                let text_color = rgb(tier.text_color());
                let mut job = LayoutJob::simple(tier.label.clone(), FontId::proportional(20.0), text_color, 84.0);
                job.halign = Align::Center;
                let galley = ui.fonts(|f| f.layout_job(job));
                let pos = rect.center() - galley.rect.center().to_vec2();
                ui.painter_at(rect).galley(pos, galley, text_color);

                ui.vertical(|ui| {
                    ui.label(RichText::new("Label (letters, words or a whole sentence)").color(TEXT_DIM));
                    let resp = ui.add(
                        TextEdit::multiline(&mut tier.label)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY)
                            .hint_text("e.g. S, God tier, Must try…"),
                    );
                    if std::mem::take(focus) {
                        resp.request_focus();
                        // Select everything so typing replaces the old label.
                        if let Some(mut state) = TextEdit::load_state(ui.ctx(), resp.id) {
                            let range = egui::text::CCursorRange::two(
                                egui::text::CCursor::new(0),
                                egui::text::CCursor::new(tier.label.chars().count()),
                            );
                            state.cursor.set_char_range(Some(range));
                            state.store(ui.ctx(), resp.id);
                        }
                    }
                });
            });

            ui.add_space(10.0);
            ui.label(RichText::new("Color").color(TEXT_DIM));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                for c in PALETTE {
                    if swatch(ui, rgb(c), tier.color == c, 26.0).clicked() {
                        tier.color = c;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Custom color");
                egui::color_picker::color_edit_button_srgb(ui, &mut tier.color);
                ui.add_space(16.0);
                let mut auto = tier.text_color.is_none();
                if ui.checkbox(&mut auto, "Auto text color").changed() {
                    tier.text_color = if auto { None } else { Some(auto_text_color(tier.color)) };
                }
                if let Some(c) = &mut tier.text_color {
                    egui::color_picker::color_edit_button_srgb(ui, c);
                }
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if secondary_button(ui, Icon::ArrowUpToLine, "Add above").clicked() {
                    action = Some(TierAction::AddAbove);
                }
                if secondary_button(ui, Icon::ArrowDownToLine, "Add below").clicked() {
                    action = Some(TierAction::AddBelow);
                }
                let clear = ui.add_enabled_ui(!tier.items.is_empty(), |ui| secondary_button(ui, Icon::Eraser, "Clear images"));
                if clear.inner.on_hover_text("Move this tier's images back to Unranked").clicked() {
                    action = Some(TierAction::Clear);
                }
                if danger_button(ui, Icon::Trash, "Delete tier")
                    .on_hover_text("Its images go back to Unranked")
                    .clicked()
                {
                    action = Some(TierAction::Delete);
                }
            });
            ui.add_space(12.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary_button(ui, Icon::Check, "Done").clicked() {
                    done = true;
                }
            });
        });

        match action {
            Some(TierAction::AddAbove) => {
                ed.discrete = true;
                let at = ed.list.insert_tier(index);
                self.dialog = Some(Dialog::EditTier { tier: at, focus: true });
                false
            }
            Some(TierAction::AddBelow) => {
                ed.discrete = true;
                let at = ed.list.insert_tier(index + 1);
                self.dialog = Some(Dialog::EditTier { tier: at, focus: true });
                false
            }
            Some(TierAction::Clear) => {
                ed.discrete = true;
                ed.list.clear_tier(index);
                true
            }
            Some(TierAction::Delete) => {
                ed.discrete = true;
                ed.list.delete_tier(index);
                false
            }
            None => !(done || modal.should_close()),
        }
    }
}

fn start_card(ui: &mut Ui, title: &str, summary: &str, colors: &[Rgb], selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(146.0, 82.0), Sense::click());
    let painter = ui.painter();
    let hovered = resp.hovered();
    painter.rect(
        rect,
        10.0,
        if hovered || selected { CARD_HOVER } else { CARD },
        stroke(if selected { 2.0 } else { 1.0 }, if selected { ACCENT } else { STROKE }),
        StrokeKind::Inside,
    );
    let shown = colors.len().clamp(1, 7);
    let stripe_area = Rect::from_min_size(rect.min + vec2(10.0, 10.0), vec2(30.0, 62.0));
    let h = (stripe_area.height() - (shown as f32 - 1.0)) / shown as f32;
    for (i, color) in colors.iter().take(shown).enumerate() {
        let r = Rect::from_min_size(pos2(stripe_area.min.x, stripe_area.min.y + i as f32 * (h + 1.0)), vec2(30.0, h));
        painter.rect_filled(r, 2.0, rgb(*color));
    }
    let x = stripe_area.max.x + 10.0;
    let mut title_job = LayoutJob::simple_singleline(title.to_owned(), FontId::proportional(14.5), TEXT);
    title_job.wrap = egui::text::TextWrapping::truncate_at_width(rect.max.x - x - 26.0);
    painter.galley(pos2(x, rect.min.y + 14.0), painter.layout_job(title_job), TEXT);
    let mut job = LayoutJob::simple(summary.to_owned(), FontId::proportional(11.5), TEXT_DIM, rect.max.x - x - 28.0);
    job.wrap.max_rows = 2;
    let galley = painter.layout_job(job);
    painter.galley(pos2(x, rect.min.y + 36.0), galley, TEXT_DIM);
    if selected {
        let r = Rect::from_min_size(pos2(rect.max.x - 22.0, rect.min.y + 8.0), vec2(14.0, 14.0));
        Icon::Check.image(14.0, ACCENT).paint_at(ui, r);
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}
