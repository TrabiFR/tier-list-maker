//! Everything is stored per tier list in its own folder:
//!
//! ```text
//! <data dir>/Tier List Maker/lists/<id>/list.json
//!                                      /images/<item>.<ext>   (original uploads)
//!                                      /thumbs/<item>.png     (fast-loading previews)
//! ```

use crate::model::{new_id, now_secs, Preset, TierList};
use anyhow::{anyhow, Context as _, Result};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

pub const PACKAGE_EXT: &str = "tierlist";

pub fn root() -> PathBuf {
    // Lets you keep a separate set of lists (handy for testing or a portable install).
    if let Some(dir) = std::env::var_os("TIERLIST_DATA_DIR") {
        return PathBuf::from(dir);
    }
    dirs::data_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Tier List Maker")
}

pub fn lists_dir() -> PathBuf {
    root().join("lists")
}

pub fn list_dir(id: &str) -> PathBuf {
    lists_dir().join(id)
}

pub fn images_dir(id: &str) -> PathBuf {
    list_dir(id).join("images")
}

pub fn thumbs_dir(id: &str) -> PathBuf {
    list_dir(id).join("thumbs")
}

fn presets_path() -> PathBuf {
    root().join("presets.json")
}

pub fn load_presets() -> Vec<Preset> {
    fs::read(presets_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save_presets(presets: &[Preset]) -> Result<()> {
    fs::create_dir_all(root())?;
    let tmp = root().join("presets.json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(presets)?)?;
    fs::rename(tmp, presets_path())?;
    Ok(())
}

pub fn save_list(list: &TierList) -> Result<()> {
    let dir = list_dir(&list.id);
    fs::create_dir_all(&dir)?;
    let json = serde_json::to_vec_pretty(list)?;
    let tmp = dir.join("list.json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, dir.join("list.json"))?;
    Ok(())
}

pub fn load_list(dir: &Path) -> Result<TierList> {
    let bytes = fs::read(dir.join("list.json"))?;
    let mut list: TierList = serde_json::from_slice(&bytes)?;
    // The folder name is the source of truth for the id.
    if let Some(name) = dir.file_name().and_then(|n| n.to_str()) {
        list.id = name.to_owned();
    }
    list.repair();
    Ok(list)
}

/// All saved lists, most recently edited first.
pub fn load_all() -> Vec<TierList> {
    let mut lists: Vec<TierList> = fs::read_dir(lists_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| load_list(&e.path()).ok())
        .collect();
    lists.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.name.cmp(&b.name)));
    lists
}

pub fn delete_list(id: &str) -> Result<()> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(anyhow!("invalid list id"));
    }
    let dir = list_dir(id);
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    Ok(())
}

pub fn duplicate_list(list: &TierList) -> Result<TierList> {
    let mut copy = list.clone();
    copy.id = new_id();
    copy.name = format!("{} (copy)", list.name);
    copy.created = now_secs();
    copy.modified = now_secs();
    copy_dir(&list_dir(&list.id), &list_dir(&copy.id))?;
    save_list(&copy)?;
    Ok(copy)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Removes image files that no item references anymore (e.g. deleted images).
pub fn remove_unused_files(list: &TierList) {
    let originals: HashSet<&str> = list.items.values().map(|i| i.file.as_str()).collect();
    let thumbs: HashSet<&str> = list.items.values().map(|i| i.thumb.as_str()).collect();
    for (dir, keep) in [(images_dir(&list.id), originals), (thumbs_dir(&list.id), thumbs)] {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let name = entry.file_name();
            let referenced = name.to_str().is_some_and(|n| keep.contains(n));
            if !referenced && entry.path().is_file() {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// Writes a `.tierlist` file: a zip with `list.json`, `images/` and `thumbs/`.
pub fn export_package(list: &TierList, dest: &Path) -> Result<()> {
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    let file = fs::File::create(dest).with_context(|| format!("creating {}", dest.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    // Images are already compressed, deflating them again only costs time.
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    zip.start_file("list.json", deflated)?;
    zip.write_all(&serde_json::to_vec_pretty(list)?)?;

    for item in list.items.values() {
        for (folder, name, dir) in [
            ("images", &item.file, images_dir(&list.id)),
            ("thumbs", &item.thumb, thumbs_dir(&list.id)),
        ] {
            if let Ok(bytes) = fs::read(dir.join(name)) {
                zip.start_file(format!("{folder}/{name}"), stored)?;
                zip.write_all(&bytes)?;
            }
        }
    }
    zip.finish()?;
    Ok(())
}

/// Imports a `.tierlist` file as a brand new list.
pub fn import_package(src: &Path) -> Result<TierList> {
    let file = fs::File::open(src).with_context(|| format!("opening {}", src.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("not a valid .tierlist file")?;

    let mut list: TierList = {
        let mut entry = archive
            .by_name("list.json")
            .context("the file does not contain a tier list")?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        serde_json::from_slice(&bytes).context("the tier list data is damaged")?
    };

    let id = new_id();
    let dir = list_dir(&id);
    let result = (|| -> Result<()> {
        fs::create_dir_all(dir.join("images"))?;
        fs::create_dir_all(dir.join("thumbs"))?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if entry.is_dir() {
                continue;
            }
            let Some(path) = entry.enclosed_name() else { continue };
            let parts: Vec<&str> = path
                .components()
                .filter_map(|c| match c {
                    Component::Normal(s) => s.to_str(),
                    _ => None,
                })
                .collect();
            let [folder, name] = parts.as_slice() else { continue };
            if *folder != "images" && *folder != "thumbs" {
                continue;
            }
            let mut out = fs::File::create(dir.join(folder).join(name))?;
            std::io::copy(&mut entry, &mut out)?;
        }
        list.id = id.clone();
        list.created = now_secs();
        list.modified = now_secs();
        list.repair();
        save_list(&list)
    })();

    if let Err(err) = result {
        let _ = fs::remove_dir_all(&dir);
        return Err(err);
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Item, Template, Zone};

    #[test]
    fn save_export_import_duplicate_delete() {
        let root = std::env::temp_dir().join(format!("tierlist-storage-test-{}", new_id()));
        std::env::set_var("TIERLIST_DATA_DIR", &root);

        let mut list = TierList::new("Snacks", Template::Classic);
        fs::create_dir_all(images_dir(&list.id)).unwrap();
        fs::create_dir_all(thumbs_dir(&list.id)).unwrap();
        fs::write(images_dir(&list.id).join("a.png"), b"original").unwrap();
        fs::write(thumbs_dir(&list.id).join("a.png"), b"thumb").unwrap();
        fs::write(images_dir(&list.id).join("stale.png"), b"unused").unwrap();
        list.add_item(Item { id: "a".into(), name: "A".into(), file: "a.png".into(), thumb: "a.png".into(), aspect: 1.0 });
        list.move_item("a", Zone::Tier(2), 0);
        save_list(&list).unwrap();

        remove_unused_files(&list);
        assert!(!images_dir(&list.id).join("stale.png").exists());
        assert_eq!(load_all().len(), 1);

        let package = root.join("snacks.tierlist");
        export_package(&list, &package).unwrap();
        let imported = import_package(&package).unwrap();
        assert_ne!(imported.id, list.id);
        assert_eq!(imported.tiers[2].items, vec!["a"]);
        assert_eq!(fs::read(images_dir(&imported.id).join("a.png")).unwrap(), b"original");

        let copy = duplicate_list(&list).unwrap();
        assert_eq!(copy.name, "Snacks (copy)");
        assert_eq!(load_all().len(), 3);

        delete_list(&copy.id).unwrap();
        assert!(!list_dir(&copy.id).exists());
        assert!(delete_list("../evil").is_err());

        let presets = vec![Preset::from_list("Mine", &list, false)];
        save_presets(&presets).unwrap();
        assert_eq!(load_presets(), presets);

        let _ = fs::remove_dir_all(&root);
    }
}
