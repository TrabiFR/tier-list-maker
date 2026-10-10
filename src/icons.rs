//! Lucide icons (https://lucide.dev, ISC license), embedded as SVG and tinted at draw time.
//! The SVG files have their stroke set to white so `tint` can recolor them.

use eframe::egui::{self, Color32, Vec2};

macro_rules! icons {
    ($($variant:ident => $file:literal),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[allow(dead_code)]
        pub enum Icon { $($variant),* }

        impl Icon {
            pub fn source(self) -> egui::ImageSource<'static> {
                match self {
                    $(Icon::$variant => egui::ImageSource::Bytes {
                        uri: std::borrow::Cow::Borrowed(concat!("bytes://lucide/", $file, ".svg")),
                        bytes: egui::load::Bytes::Static(include_bytes!(concat!(
                            env!("CARGO_MANIFEST_DIR"), "/assets/icons/", $file, ".svg"
                        ))),
                    },)*
                }
            }
        }
    };
}

icons! {
    ArrowLeft => "arrow-left",
    ArrowUpToLine => "arrow-up-to-line",
    ArrowDownToLine => "arrow-down-to-line",
    Bookmark => "bookmark",
    BookmarkPlus => "bookmark-plus",
    Check => "check",
    ChevronUp => "chevron-up",
    ChevronDown => "chevron-down",
    Copy => "copy",
    Crop => "crop",
    Crown => "crown",
    Download => "download",
    Ellipsis => "ellipsis-vertical",
    Eraser => "eraser",
    FileUp => "file-up",
    FolderOpen => "folder-open",
    Image => "image",
    ImageDown => "image-down",
    ImagePlus => "image-plus",
    Inbox => "inbox",
    Loader => "loader-circle",
    Package => "package",
    Palette => "palette",
    Pencil => "pencil",
    Plus => "plus",
    Redo => "redo-2",
    RotateCcw => "rotate-ccw",
    Settings => "settings",
    Sliders => "sliders-horizontal",
    Trash => "trash",
    Alert => "triangle-alert",
    Undo => "undo-2",
    X => "x",
}

impl Icon {
    pub fn image(self, size: f32, color: Color32) -> egui::Image<'static> {
        egui::Image::new(self.source())
            .fit_to_exact_size(Vec2::splat(size))
            .tint(color)
    }
}
