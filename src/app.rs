use crate::dialogs::Dialog;
use crate::editor::Editor;
use crate::icons::Icon;
use crate::jobs::{self, JobMsg, Jobs, ThumbRequest};
use crate::model::{now_secs, Preset, StartWith, TierList};
use crate::theme::{self, SystemFont};
use crate::toasts::Toasts;
use crate::util::{plural, safe_file_name, with_extension};
use crate::storage;
use eframe::egui::{
    self, Align2, Color32, FontId, Id, Key, KeyboardShortcut, LayerId, Modifiers, Order, Rect,
    StrokeKind, TextureHandle, TextureOptions, Vec2,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

/// Width used for PNG exports started from the home screen (no editor to measure).
const DEFAULT_EXPORT_WIDTH: f32 = 1100.0;

#[derive(Clone, Copy)]
pub enum Pending {
    Undo,
    Redo,
}

pub struct App {
    pub(crate) lists: Vec<TierList>,
    pub(crate) presets: Vec<Preset>,
    /// Template/preset picked last time, preselected in the "New tier list" dialog.
    pub(crate) last_start: StartWith,
    pub(crate) editor: Option<Editor>,
    pub(crate) dialog: Option<Dialog>,
    pub(crate) textures: HashMap<String, TextureHandle>,
    pub(crate) thumbs_requested: HashSet<String>,
    pub(crate) thumbs_failed: HashSet<String>,
    pub(crate) jobs: Jobs,
    jobs_rx: Receiver<JobMsg>,
    pub(crate) toasts: Toasts,
    pub(crate) export_font: ab_glyph::FontArc,
    pub(crate) busy_imports: usize,
    pub(crate) busy_exports: usize,
    pub(crate) pending: Option<Pending>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let system_font: Option<SystemFont> = theme::load_system_font();
        theme::apply(&cc.egui_ctx, system_font.as_ref());
        let (jobs, jobs_rx) = Jobs::new(cc.egui_ctx.clone());
        let mut app = Self {
            lists: Vec::new(),
            presets: storage::load_presets(),
            last_start: StartWith::Template(crate::model::Template::Classic),
            editor: None,
            dialog: None,
            textures: HashMap::new(),
            thumbs_requested: HashSet::new(),
            thumbs_failed: HashSet::new(),
            jobs,
            jobs_rx,
            toasts: Toasts::default(),
            export_font: theme::export_font(system_font.as_ref()),
            busy_imports: 0,
            busy_exports: 0,
            pending: None,
        };
        app.reload_lists();
        app
    }

    pub fn reload_lists(&mut self) {
        self.lists = storage::load_all();
    }

    // ---------- opening & closing lists ----------

    pub fn open_list(&mut self, id: &str) {
        let list = match storage::load_list(&storage::list_dir(id)) {
            Ok(list) => list,
            Err(err) => {
                self.toasts.error(format!("Could not open the tier list: {err}"));
                return;
            }
        };
        storage::remove_unused_files(&list);
        self.open_loaded(list);
    }

    pub fn open_loaded(&mut self, list: TierList) {
        self.save_editor();
        self.clear_textures();
        self.editor = Some(Editor::new(list));
    }

    pub fn create_list(&mut self, name: &str, start: &StartWith, images: Vec<PathBuf>) {
        let name = if name.trim().is_empty() { "Untitled tier list" } else { name.trim() };
        let list = match start {
            StartWith::Template(t) => TierList::new(name, *t),
            StartWith::Preset(id) => match self.presets.iter().find(|p| &p.id == id) {
                Some(preset) => TierList::from_preset(name, preset),
                None => TierList::new(name, crate::model::Template::Classic),
            },
        };
        if let Err(err) = storage::save_list(&list) {
            self.toasts.error(format!("Could not create the tier list: {err}"));
            return;
        }
        let id = list.id.clone();
        self.open_loaded(list);
        if !images.is_empty() {
            self.import_images(&id, images);
        }
    }

    // ---------- presets ----------

    pub fn open_new_list_dialog(&mut self, images: Vec<PathBuf>) {
        let start = match &self.last_start {
            StartWith::Preset(id) if !self.presets.iter().any(|p| &p.id == id) => {
                StartWith::Template(crate::model::Template::Classic)
            }
            other => other.clone(),
        };
        self.dialog = Some(Dialog::new_list(images, start));
    }

    /// Saves the open list's tiers as a preset; a preset with the same name is replaced.
    pub fn save_preset(&mut self, name: &str, include_settings: bool) {
        let Some(ed) = &self.editor else { return };
        let name = if name.trim().is_empty() { ed.list.name.trim() } else { name.trim() };
        let mut preset = Preset::from_list(name, &ed.list, include_settings);
        let existing = self.presets.iter().position(|p| p.name.eq_ignore_ascii_case(name));
        match existing {
            Some(i) => {
                preset.id = self.presets[i].id.clone();
                self.presets[i] = preset;
            }
            None => self.presets.push(preset),
        }
        match storage::save_presets(&self.presets) {
            Ok(()) => self.toasts.success(format!(
                "{} preset “{name}”. Pick it when you create a new tier list.",
                if existing.is_some() { "Updated" } else { "Saved" }
            )),
            Err(err) => self.toasts.error(format!("Could not save the preset: {err}")),
        }
    }

    pub fn delete_preset(&mut self, id: &str) {
        let Some(i) = self.presets.iter().position(|p| p.id == id) else { return };
        let preset = self.presets.remove(i);
        match storage::save_presets(&self.presets) {
            Ok(()) => self.toasts.success(format!("Deleted preset “{}”", preset.name)),
            Err(err) => self.toasts.error(format!("Could not delete the preset: {err}")),
        }
    }

    pub fn close_editor(&mut self) {
        self.save_editor();
        if let Some(ed) = self.editor.take() {
            storage::remove_unused_files(&ed.list);
        }
        self.clear_textures();
        self.reload_lists();
    }

    fn clear_textures(&mut self) {
        self.textures.clear();
        self.thumbs_requested.clear();
        self.thumbs_failed.clear();
    }

    pub fn save_editor(&mut self) {
        let Some(ed) = self.editor.as_mut() else { return };
        ed.list.modified = now_secs();
        match storage::save_list(&ed.list) {
            Ok(()) => ed.dirty = false,
            Err(err) => self.toasts.error(format!("Saving failed: {err}")),
        }
    }

    // ---------- images ----------

    pub fn import_images(&mut self, list_id: &str, files: Vec<PathBuf>) {
        let files = jobs::collect_images(files);
        if files.is_empty() {
            return;
        }
        self.busy_imports += files.len();
        self.jobs.import_images(list_id.to_owned(), files);
    }

    pub fn pick_images(&mut self) {
        let Some(id) = self.editor.as_ref().map(|e| e.list.id.clone()) else { return };
        let files = rfd::FileDialog::new()
            .set_title("Add images")
            .add_filter("Images", jobs::IMAGE_EXTENSIONS)
            .pick_files();
        if let Some(files) = files {
            self.import_images(&id, files);
        }
    }

    pub fn request_thumb(&mut self, item_id: &str) {
        let Some(ed) = &self.editor else { return };
        if self.thumbs_requested.contains(item_id) {
            return;
        }
        let Some(item) = ed.list.items.get(item_id) else { return };
        self.thumbs_requested.insert(item_id.to_owned());
        self.jobs.request_thumb(ThumbRequest {
            item_id: item_id.to_owned(),
            thumb: storage::thumbs_dir(&ed.list.id).join(&item.thumb),
            original: storage::images_dir(&ed.list.id).join(&item.file),
        });
    }

    // ---------- export / import ----------

    pub fn export_png(&mut self, list: TierList, width: f32) {
        let name = format!("{}.png", safe_file_name(&list.name, "tier list"));
        let path = rfd::FileDialog::new()
            .set_title("Export tier list as PNG")
            .set_file_name(name)
            .add_filter("PNG image", &["png"])
            .save_file();
        if let Some(path) = path {
            self.busy_exports += 1;
            self.jobs.export_png(list, with_extension(path, "png"), width, self.export_font.clone());
        }
    }

    pub fn export_package(&mut self, list: TierList) {
        let name = format!("{}.{}", safe_file_name(&list.name, "tier list"), storage::PACKAGE_EXT);
        let path = rfd::FileDialog::new()
            .set_title("Export tier list file")
            .set_file_name(name)
            .add_filter("Tier list", &[storage::PACKAGE_EXT])
            .save_file();
        if let Some(path) = path {
            self.busy_exports += 1;
            self.jobs.export_package(list, with_extension(path, storage::PACKAGE_EXT));
        }
    }

    pub fn export_current_png(&mut self) {
        if let Some(ed) = &self.editor {
            let (list, width) = (ed.list.clone(), ed.export_width);
            self.export_png(list, width);
        }
    }

    pub fn export_png_by_id(&mut self, id: &str) {
        if let Some(list) = self.lists.iter().find(|l| l.id == id).cloned() {
            self.export_png(list, DEFAULT_EXPORT_WIDTH);
        }
    }

    pub fn pick_packages(&mut self) {
        let files = rfd::FileDialog::new()
            .set_title("Import tier list")
            .add_filter("Tier list", &[storage::PACKAGE_EXT, "zip"])
            .pick_files();
        for file in files.into_iter().flatten() {
            self.busy_imports += 1;
            self.jobs.import_package(file);
        }
    }

    // ---------- frame plumbing ----------

    fn poll_jobs(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.jobs_rx.try_recv() {
            match msg {
                JobMsg::Imported { list_id, item, image } => {
                    self.busy_imports = self.busy_imports.saturating_sub(1);
                    match self.editor.as_mut().filter(|e| e.list.id == list_id) {
                        Some(ed) => {
                            let texture = ctx.load_texture(
                                format!("item-{}", item.id),
                                image,
                                texture_options(),
                            );
                            self.textures.insert(item.id.clone(), texture);
                            self.thumbs_requested.insert(item.id.clone());
                            ed.list.add_item(item);
                        }
                        None => {
                            // The list was closed while importing; add the image on disk.
                            if let Ok(mut list) = storage::load_list(&storage::list_dir(&list_id)) {
                                list.add_item(item);
                                list.modified = now_secs();
                                let _ = storage::save_list(&list);
                            }
                        }
                    }
                }
                JobMsg::ImportFailed => {
                    self.busy_imports = self.busy_imports.saturating_sub(1);
                }
                JobMsg::ImportBatchDone { list_id, ok, failed } => {
                    if !failed.is_empty() {
                        let mut names = failed.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
                        if failed.len() > 3 {
                            names.push_str(", …");
                        }
                        self.toasts.error(format!(
                            "Couldn't read {} ({names}). Supported: {}.",
                            plural(failed.len(), "file", "files"),
                            supported_formats()
                        ));
                    } else if ok > 0 {
                        self.toasts.success(format!("Added {}", plural(ok, "image", "images")));
                    }
                    if self.editor.as_ref().is_none_or(|e| e.list.id != list_id) {
                        self.reload_lists();
                    }
                }
                JobMsg::Thumb { item_id, image } => {
                    match image {
                        Some(image) => {
                            let texture = ctx.load_texture(format!("item-{item_id}"), image, texture_options());
                            self.textures.insert(item_id, texture);
                        }
                        None => {
                            self.thumbs_failed.insert(item_id);
                        }
                    }
                }
                JobMsg::Exported { path, result } => {
                    self.busy_exports = self.busy_exports.saturating_sub(1);
                    match result {
                        Ok(()) => {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            self.toasts.success_with_reveal(format!("Exported {name}"), path);
                        }
                        Err(err) => self.toasts.error(format!("Export failed: {err}")),
                    }
                }
                JobMsg::PackageImported { result } => {
                    self.busy_imports = self.busy_imports.saturating_sub(1);
                    match result {
                        Ok(list) => {
                            self.toasts.success(format!("Imported “{}”", list.name));
                            if self.editor.is_none() {
                                self.reload_lists();
                            }
                        }
                        Err(err) => self.toasts.error(format!("Import failed: {err}")),
                    }
                }
            }
        }
    }

    fn process_drops(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if dropped.is_empty() {
            return;
        }
        let (packages, others): (Vec<PathBuf>, Vec<PathBuf>) = dropped.into_iter().partition(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(storage::PACKAGE_EXT))
        });
        for package in packages {
            self.busy_imports += 1;
            self.jobs.import_package(package);
        }
        let images = jobs::collect_images(others);
        if images.is_empty() {
            return;
        }
        if let Some(id) = self.editor.as_ref().map(|e| e.list.id.clone()) {
            self.import_images(&id, images);
        } else {
            self.open_new_list_dialog(images);
        }
    }

    fn handle_undo_redo(&mut self, ctx: &egui::Context) {
        let pending = self.pending.take();
        let Some(ed) = self.editor.as_mut() else { return };
        let mut action = pending;
        if action.is_none() && self.dialog.is_none() && !ctx.wants_keyboard_input() {
            ctx.input_mut(|i| {
                let redo = i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z))
                    || i.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::Y));
                if redo {
                    action = Some(Pending::Redo);
                } else if i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Z)) {
                    action = Some(Pending::Undo);
                }
            });
        }
        let now = ctx.input(|i| i.time);
        match action {
            Some(Pending::Undo) => ed.undo(now),
            Some(Pending::Redo) => ed.redo(now),
            None => {}
        }
    }

    fn handle_delete_key(&mut self, ctx: &egui::Context) {
        let Some(ed) = self.editor.as_mut() else { return };
        if self.dialog.is_some() || ctx.wants_keyboard_input() || ed.drag.is_some() {
            return;
        }
        let pressed = ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace));
        if pressed {
            if let Some(id) = ed.hovered_item.take() {
                ed.discrete = true;
                ed.list.remove_item(&id);
            }
        }
    }

    fn autosave(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let Some(ed) = self.editor.as_ref() else { return };
        if !ed.dirty {
            return;
        }
        if now - ed.changed_at >= 0.6 {
            self.save_editor();
        } else {
            ctx.request_repaint_after(Duration::from_millis(650));
        }
    }

    fn file_hover_overlay(&self, ctx: &egui::Context) {
        if ctx.input(|i| i.raw.hovered_files.is_empty()) {
            return;
        }
        let screen = ctx.screen_rect();
        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("file_drop_overlay")));
        painter.rect_filled(screen, 0.0, Color32::from_black_alpha(190));
        let frame = screen.shrink(24.0);
        painter.rect_stroke(frame, 18.0, theme::stroke(2.0, theme::ACCENT), StrokeKind::Inside);
        let text = if self.editor.is_some() {
            "Drop images to add them to Unranked"
        } else {
            "Drop images to start a new tier list, or a .tierlist file to import it"
        };
        painter.text(
            screen.center() + Vec2::new(0.0, 34.0),
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(20.0),
            theme::TEXT,
        );
        egui::Area::new(Id::new("file_drop_icon"))
            .order(Order::Foreground)
            .fixed_pos(screen.center() - Vec2::new(24.0, 40.0))
            .interactable(false)
            .show(ctx, |ui| {
                let rect = Rect::from_min_size(ui.cursor().min, Vec2::splat(48.0));
                Icon::ImagePlus.image(48.0, theme::ACCENT).paint_at(ui, rect);
                ui.allocate_rect(rect, egui::Sense::hover());
            });
    }
}

fn supported_formats() -> &'static str {
    if cfg!(target_os = "macos") {
        "PNG, JPG, GIF, WebP, BMP, TIFF, HEIC, AVIF"
    } else {
        "PNG, JPG, GIF, WebP, BMP, TIFF"
    }
}

pub fn texture_options() -> TextureOptions {
    TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear))
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Undo/redo replace the list wholesale, so they run before change tracking starts.
        self.handle_undo_redo(ctx);
        let before = self.editor.as_ref().map(|e| (e.list.id.clone(), e.list.clone()));

        self.poll_jobs(ctx);
        self.process_drops(ctx);
        self.handle_delete_key(ctx);

        if self.editor.is_some() {
            self.editor_ui(ctx);
        } else {
            self.home_ui(ctx);
        }
        self.dialogs_ui(ctx);
        self.file_hover_overlay(ctx);
        self.toasts.show(ctx);

        if let (Some((id, before)), Some(ed)) = (before, self.editor.as_mut()) {
            if ed.list.id == id {
                ed.track_changes(before, ctx.input(|i| i.time));
            }
        }
        self.autosave(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_editor();
    }
}
