//! Slow work (decoding images, exporting) runs on background threads so the UI never freezes.
//! Results come back to the UI thread as [`JobMsg`]s.

use crate::model::{new_id, Item, TierList};
use crate::{export, storage};
use anyhow::{Context as _, Result};
use eframe::egui;
use image::{DynamicImage, ImageDecoder, ImageReader, RgbaImage};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

#[cfg(target_os = "macos")]
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "jfif", "gif", "webp", "bmp", "tif", "tiff", "ico", "heic", "heif", "avif",
];
#[cfg(not(target_os = "macos"))]
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "jfif", "gif", "webp", "bmp", "tif", "tiff", "ico",
];
const THUMB_SIZE: u32 = 384;

pub enum JobMsg {
    Imported {
        list_id: String,
        item: Item,
        image: egui::ColorImage,
    },
    ImportFailed,
    ImportBatchDone {
        list_id: String,
        ok: usize,
        failed: Vec<String>,
    },
    Thumb {
        item_id: String,
        image: Option<egui::ColorImage>,
    },
    Exported {
        path: PathBuf,
        result: Result<(), String>,
    },
    PackageImported {
        result: Result<TierList, String>,
    },
}

pub struct ThumbRequest {
    pub item_id: String,
    pub thumb: PathBuf,
    pub original: PathBuf,
}

#[derive(Clone)]
pub struct Jobs {
    ctx: egui::Context,
    tx: Sender<JobMsg>,
    thumb_tx: Sender<ThumbRequest>,
}

impl Jobs {
    pub fn new(ctx: egui::Context) -> (Self, Receiver<JobMsg>) {
        let (tx, rx) = channel();
        let (thumb_tx, thumb_rx) = channel::<ThumbRequest>();
        let thumb_rx = Arc::new(Mutex::new(thumb_rx));
        for _ in 0..3 {
            let rx = thumb_rx.clone();
            let tx = tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || loop {
                let request = match rx.lock() {
                    Ok(guard) => guard.recv(),
                    Err(_) => return,
                };
                let Ok(request) = request else { return };
                let image = load_thumb(&request).ok().map(|img| to_color_image(&img));
                if tx.send(JobMsg::Thumb { item_id: request.item_id, image }).is_err() {
                    return;
                }
                ctx.request_repaint();
            });
        }
        (Self { ctx, tx, thumb_tx }, rx)
    }

    pub fn request_thumb(&self, request: ThumbRequest) {
        let _ = self.thumb_tx.send(request);
    }

    pub fn import_images(&self, list_id: String, files: Vec<PathBuf>) {
        let (ctx, tx) = (self.ctx.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let threads = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .clamp(1, 6);
            let queue = Mutex::new(files.into_iter());
            let ok = std::sync::atomic::AtomicUsize::new(0);
            let failed = Mutex::new(Vec::new());
            std::thread::scope(|scope| {
                for _ in 0..threads {
                    scope.spawn(|| loop {
                        let Some(path) = queue.lock().ok().and_then(|mut q| q.next()) else {
                            return;
                        };
                        let msg = match import_one(&list_id, &path) {
                            Ok((item, image)) => {
                                ok.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                JobMsg::Imported { list_id: list_id.clone(), item, image }
                            }
                            Err(_) => {
                                let file = file_label(&path);
                                if let Ok(mut f) = failed.lock() {
                                    f.push(file);
                                }
                                JobMsg::ImportFailed
                            }
                        };
                        let _ = tx.send(msg);
                        ctx.request_repaint();
                    });
                }
            });
            let _ = tx.send(JobMsg::ImportBatchDone {
                list_id,
                ok: ok.into_inner(),
                failed: failed.into_inner().unwrap_or_default(),
            });
            ctx.request_repaint();
        });
    }

    pub fn export_png(&self, list: TierList, path: PathBuf, width: f32, font: ab_glyph::FontArc) {
        let (ctx, tx) = (self.ctx.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let result = export::export_png(&list, &path, width, &font).map_err(|e| format!("{e:#}"));
            let _ = tx.send(JobMsg::Exported { path, result });
            ctx.request_repaint();
        });
    }

    pub fn export_package(&self, list: TierList, path: PathBuf) {
        let (ctx, tx) = (self.ctx.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let result = storage::export_package(&list, &path).map_err(|e| format!("{e:#}"));
            let _ = tx.send(JobMsg::Exported { path, result });
            ctx.request_repaint();
        });
    }

    pub fn import_package(&self, path: PathBuf) {
        let (ctx, tx) = (self.ctx.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let result = storage::import_package(&path).map_err(|e| format!("{e:#}"));
            let _ = tx.send(JobMsg::PackageImported { result });
            ctx.request_repaint();
        });
    }
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Expands dropped folders into the images they contain (one level deep).
pub fn collect_images(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        if path.is_dir() {
            let mut inner: Vec<PathBuf> = std::fs::read_dir(&path)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && is_image_path(p))
                .collect();
            inner.sort();
            out.extend(inner);
        } else {
            out.push(path);
        }
    }
    out
}

/// Decodes an image and applies its EXIF rotation (phone photos).
pub fn decode_bytes(bytes: &[u8]) -> Result<DynamicImage> {
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder)?;
    if let Some(orientation) = orientation {
        img.apply_orientation(orientation);
    }
    Ok(img)
}

pub fn decode_file(path: &Path) -> Result<DynamicImage> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    decode_bytes(&bytes)
}

fn make_thumb(img: &DynamicImage) -> RgbaImage {
    if img.width() <= THUMB_SIZE && img.height() <= THUMB_SIZE {
        img.to_rgba8()
    } else {
        img.thumbnail(THUMB_SIZE, THUMB_SIZE).to_rgba8()
    }
}

pub fn to_color_image(img: &RgbaImage) -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied(
        [img.width() as usize, img.height() as usize],
        img.as_raw(),
    )
}

fn import_one(list_id: &str, path: &Path) -> Result<(Item, egui::ColorImage)> {
    let mut bytes = std::fs::read(path)?;
    let mut ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| IMAGE_EXTENSIONS.contains(&e.as_str()))
        .unwrap_or_else(|| "img".to_owned());
    let img = match decode_bytes(&bytes) {
        Ok(img) => img,
        Err(err) => {
            // HEIC/AVIF (e.g. iPhone photos): let macOS convert them to JPEG once, at import.
            let converted = convert_with_system(path).ok_or(err)?;
            let img = decode_bytes(&converted)?;
            bytes = converted;
            ext = "jpg".to_owned();
            img
        }
    };
    let id = new_id();

    let file = format!("{id}.{ext}");
    let images = storage::images_dir(list_id);
    std::fs::create_dir_all(&images)?;
    std::fs::write(images.join(&file), &bytes)?;

    let thumb = make_thumb(&img);
    let thumb_name = format!("{id}.png");
    let thumbs = storage::thumbs_dir(list_id);
    std::fs::create_dir_all(&thumbs)?;
    thumb.save(thumbs.join(&thumb_name))?;

    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Image".to_owned());
    let item = Item {
        id,
        name,
        file,
        thumb: thumb_name,
        aspect: img.width() as f32 / img.height().max(1) as f32,
    };
    Ok((item, to_color_image(&thumb)))
}

#[cfg(target_os = "macos")]
fn convert_with_system(path: &Path) -> Option<Vec<u8>> {
    let out = std::env::temp_dir().join(format!("tierlist-convert-{}.jpg", new_id()));
    let status = std::process::Command::new("/usr/bin/sips")
        .args(["-s", "format", "jpeg", "-s", "formatOptions", "92"])
        .arg(path)
        .arg("--out")
        .arg(&out)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    let bytes = status.success().then(|| std::fs::read(&out).ok()).flatten();
    let _ = std::fs::remove_file(&out);
    bytes
}

#[cfg(not(target_os = "macos"))]
fn convert_with_system(_path: &Path) -> Option<Vec<u8>> {
    None
}

fn load_thumb(request: &ThumbRequest) -> Result<RgbaImage> {
    if let Ok(img) = image::open(&request.thumb) {
        return Ok(img.to_rgba8());
    }
    // Missing or broken thumbnail: rebuild it from the original.
    let thumb = make_thumb(&decode_file(&request.original)?);
    if let Some(parent) = request.thumb.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = thumb.save(&request.thumb);
    Ok(thumb)
}
