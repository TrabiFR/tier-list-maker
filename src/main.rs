// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod dialogs;
mod editor;
mod export;
mod home;
mod icons;
mod jobs;
mod layout;
mod model;
mod storage;
mod theme;
mod toasts;
mod util;
mod widgets;

use eframe::egui;

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Tier List Maker")
        .with_app_id("tier-list-maker")
        .with_inner_size([1280.0, 840.0])
        .with_min_inner_size([780.0, 540.0])
        .with_drag_and_drop(true);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/app-icon.png")) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Tier List Maker",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
