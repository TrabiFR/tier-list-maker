use crate::icons::Icon;
use crate::theme::*;
use eframe::egui::{self, Align2, Id, Margin, Order, RichText};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Success,
    Error,
}

struct Toast {
    kind: Kind,
    text: String,
    reveal: Option<PathBuf>,
    shown_at: Option<f64>,
}

#[derive(Default)]
pub struct Toasts {
    items: Vec<Toast>,
}

impl Toasts {
    pub fn success(&mut self, text: impl Into<String>) {
        self.push(Kind::Success, text.into(), None);
    }

    pub fn success_with_reveal(&mut self, text: impl Into<String>, path: PathBuf) {
        self.push(Kind::Success, text.into(), Some(path));
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.push(Kind::Error, text.into(), None);
    }

    fn push(&mut self, kind: Kind, text: String, reveal: Option<PathBuf>) {
        self.items.push(Toast { kind, text, reveal, shown_at: None });
        if self.items.len() > 4 {
            self.items.remove(0);
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        self.items.retain(|t| {
            let ttl = if t.kind == Kind::Error { 8.0 } else { 4.5 };
            t.shown_at.is_none_or(|s| now - s < ttl)
        });
        if self.items.is_empty() {
            return;
        }
        let mut dismiss = None;
        egui::Area::new(Id::new("toasts"))
            .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-18.0, -18.0))
            .order(Order::Foreground)
            .show(ctx, |ui| {
                for (i, toast) in self.items.iter_mut().enumerate() {
                    toast.shown_at.get_or_insert(now);
                    let (icon, color) = match toast.kind {
                        Kind::Success => (Icon::Check, SUCCESS),
                        Kind::Error => (Icon::Alert, DANGER),
                    };
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgb(34, 34, 39))
                        .stroke(stroke(1.0, STROKE))
                        .corner_radius(10.0)
                        .inner_margin(Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.set_max_width(380.0);
                            ui.horizontal(|ui| {
                                ui.add(icon.image(18.0, color));
                                ui.label(RichText::new(&toast.text).color(TEXT));
                                if let Some(path) = &toast.reveal {
                                    if ui.link(reveal_label()).clicked() {
                                        crate::util::reveal(path);
                                        dismiss = Some(i);
                                    }
                                }
                            });
                        });
                    ui.add_space(6.0);
                }
            });
        if let Some(i) = dismiss {
            self.items.remove(i);
        }
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

fn reveal_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Show in Finder"
    } else if cfg!(target_os = "windows") {
        "Show in Explorer"
    } else {
        "Show file"
    }
}
