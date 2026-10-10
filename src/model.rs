use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub type Rgb = [u8; 3];

/// The classic tier colors (S, A, B, C, D, ...) followed by a few extra choices.
pub const PALETTE: [Rgb; 15] = [
    [0xFF, 0x7F, 0x7F],
    [0xFF, 0xBF, 0x7F],
    [0xFF, 0xDF, 0x7F],
    [0xFF, 0xFF, 0x7F],
    [0xBF, 0xFF, 0x7F],
    [0x7F, 0xFF, 0x7F],
    [0x7F, 0xFF, 0xFF],
    [0x7F, 0xBF, 0xFF],
    [0x7F, 0x7F, 0xFF],
    [0xFF, 0x7F, 0xFF],
    [0xBF, 0x7F, 0xBF],
    [0x3B, 0x3B, 0x3B],
    [0x85, 0x85, 0x85],
    [0xCF, 0xCF, 0xCF],
    [0xF7, 0xF7, 0xF7],
];

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Unique, filesystem-safe id (hex characters only).
pub fn new_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}{:04x}", nanos, n & 0xffff)
}

/// Readable text color (dark or light) for a given background.
pub fn auto_text_color(bg: Rgb) -> Rgb {
    let [r, g, b] = bg.map(|c| c as f32 / 255.0);
    let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    if luminance > 0.5 {
        [26, 26, 26]
    } else {
        [245, 245, 245]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TierList {
    pub id: String,
    pub name: String,
    pub created: u64,
    pub modified: u64,
    pub tiers: Vec<Tier>,
    /// Item ids that have not been placed in a tier yet.
    pub unranked: Vec<String>,
    pub items: HashMap<String, Item>,
    #[serde(default)]
    pub settings: Settings,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tier {
    pub id: String,
    pub label: String,
    pub color: Rgb,
    /// `None` picks black or white automatically.
    #[serde(default)]
    pub text_color: Option<Rgb>,
    pub items: Vec<String>,
}

impl Tier {
    pub fn new(label: &str, color: Rgb) -> Self {
        Self {
            id: new_id(),
            label: label.to_owned(),
            color,
            text_color: None,
            items: Vec::new(),
        }
    }

    pub fn text_color(&self) -> Rgb {
        self.text_color.unwrap_or_else(|| auto_text_color(self.color))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    /// Original image file name inside the list's `images/` folder.
    pub file: String,
    /// Thumbnail file name inside the list's `thumbs/` folder.
    pub thumb: String,
    /// Width / height of the original image.
    pub aspect: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ImageFit {
    #[default]
    Square,
    Original,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub item_size: f32,
    pub gap: f32,
    pub fit: ImageFit,
    pub show_names: bool,
    pub label_width: f32,
    pub label_font_size: f32,
    pub row_color: Rgb,
    pub border_color: Rgb,
    pub export_title: bool,
    pub export_scale: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            item_size: 88.0,
            gap: 2.0,
            fit: ImageFit::Square,
            show_names: false,
            label_width: 110.0,
            label_font_size: 22.0,
            row_color: [0x1B, 0x1B, 0x17],
            border_color: [0, 0, 0],
            export_title: true,
            export_scale: 2.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Tier(usize),
    Unranked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    Classic,
    Extended,
    Numbers,
    Words,
    Blank,
}

impl Template {
    pub const ALL: [Template; 5] = [
        Template::Classic,
        Template::Extended,
        Template::Numbers,
        Template::Words,
        Template::Blank,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Template::Classic => "Classic",
            Template::Extended => "Extended",
            Template::Numbers => "Numbers",
            Template::Words => "Words",
            Template::Blank => "Blank",
        }
    }

    pub fn tiers(self) -> Vec<(&'static str, Rgb)> {
        let labels: &[&str] = match self {
            Template::Classic => &["S", "A", "B", "C", "D"],
            Template::Extended => &["S", "A", "B", "C", "D", "E", "F"],
            Template::Numbers => &["1", "2", "3", "4", "5"],
            Template::Words => &["Love it", "Like it", "It's okay", "Nope"],
            Template::Blank => &["New tier"],
        };
        labels
            .iter()
            .enumerate()
            .map(|(i, l)| (*l, PALETTE[i % PALETTE.len()]))
            .collect()
    }

    pub fn summary(self) -> String {
        let tiers = self.tiers();
        if tiers.len() > 5 {
            format!("{} … {}", tiers[0].0, tiers[tiers.len() - 1].0)
        } else {
            tiers.iter().map(|t| t.0).collect::<Vec<_>>().join(" · ")
        }
    }
}

/// A saved tier setup (labels + colors, optionally layout settings) to start new lists from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub tiers: Vec<PresetTier>,
    #[serde(default)]
    pub settings: Option<Settings>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresetTier {
    pub label: String,
    pub color: Rgb,
    #[serde(default)]
    pub text_color: Option<Rgb>,
}

impl Preset {
    pub fn from_list(name: &str, list: &TierList, include_settings: bool) -> Self {
        Self {
            id: new_id(),
            name: name.to_owned(),
            tiers: list
                .tiers
                .iter()
                .map(|t| PresetTier {
                    label: t.label.clone(),
                    color: t.color,
                    text_color: t.text_color,
                })
                .collect(),
            settings: include_settings.then(|| list.settings.clone()),
        }
    }

    pub fn summary(&self) -> String {
        let labels: Vec<String> = self
            .tiers
            .iter()
            .map(|t| t.label.lines().next().unwrap_or("").trim().to_owned())
            .collect();
        match labels.len() {
            0 => "No tiers".to_owned(),
            1..=4 => labels.join(" · "),
            n => format!("{} … {} ({n} tiers)", labels[0], labels[n - 1]),
        }
    }
}

/// What a new list starts from: a built-in template or one of the user's presets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartWith {
    Template(Template),
    Preset(String),
}

impl TierList {
    pub fn from_preset(name: &str, preset: &Preset) -> Self {
        let mut list = TierList::new(name, Template::Blank);
        list.tiers = preset
            .tiers
            .iter()
            .map(|t| Tier {
                text_color: t.text_color,
                ..Tier::new(&t.label, t.color)
            })
            .collect();
        if let Some(settings) = &preset.settings {
            list.settings = settings.clone();
        }
        list
    }

    pub fn new(name: &str, template: Template) -> Self {
        let now = now_secs();
        Self {
            id: new_id(),
            name: name.to_owned(),
            created: now,
            modified: now,
            tiers: template
                .tiers()
                .into_iter()
                .map(|(label, color)| Tier::new(label, color))
                .collect(),
            unranked: Vec::new(),
            items: HashMap::new(),
            settings: Settings::default(),
        }
    }

    pub fn zone_items(&self, zone: Zone) -> &[String] {
        match zone {
            Zone::Tier(i) => self.tiers.get(i).map(|t| t.items.as_slice()).unwrap_or(&[]),
            Zone::Unranked => &self.unranked,
        }
    }

    fn zone_items_mut(&mut self, zone: Zone) -> &mut Vec<String> {
        match zone {
            Zone::Tier(i) if i < self.tiers.len() => &mut self.tiers[i].items,
            _ => &mut self.unranked,
        }
    }

    pub fn locate(&self, item: &str) -> Option<(Zone, usize)> {
        for (ti, tier) in self.tiers.iter().enumerate() {
            if let Some(i) = tier.items.iter().position(|id| id == item) {
                return Some((Zone::Tier(ti), i));
            }
        }
        self.unranked
            .iter()
            .position(|id| id == item)
            .map(|i| (Zone::Unranked, i))
    }

    /// Moves an item to `to`. `index` refers to the target list with the item already removed;
    /// out-of-range indices append.
    pub fn move_item(&mut self, item: &str, to: Zone, index: usize) {
        if !self.items.contains_key(item) {
            return;
        }
        if let Some((zone, i)) = self.locate(item) {
            self.zone_items_mut(zone).remove(i);
        }
        let target = self.zone_items_mut(to);
        let index = index.min(target.len());
        target.insert(index, item.to_owned());
    }

    pub fn add_item(&mut self, item: Item) {
        self.unranked.push(item.id.clone());
        self.items.insert(item.id.clone(), item);
    }

    pub fn remove_item(&mut self, item: &str) {
        if let Some((zone, i)) = self.locate(item) {
            self.zone_items_mut(zone).remove(i);
        }
        self.items.remove(item);
    }

    pub fn insert_tier(&mut self, at: usize) -> usize {
        let used: HashSet<Rgb> = self.tiers.iter().map(|t| t.color).collect();
        let color = PALETTE
            .iter()
            .copied()
            .find(|c| !used.contains(c))
            .unwrap_or(PALETTE[self.tiers.len() % PALETTE.len()]);
        let at = at.min(self.tiers.len());
        self.tiers.insert(at, Tier::new("New", color));
        at
    }

    pub fn delete_tier(&mut self, index: usize) {
        if index < self.tiers.len() {
            let tier = self.tiers.remove(index);
            self.unranked.extend(tier.items);
        }
    }

    pub fn clear_tier(&mut self, index: usize) {
        if let Some(tier) = self.tiers.get_mut(index) {
            let items = std::mem::take(&mut tier.items);
            self.unranked.extend(items);
        }
    }

    pub fn reset_all(&mut self) {
        for i in 0..self.tiers.len() {
            self.clear_tier(i);
        }
    }

    pub fn ranked_count(&self) -> usize {
        self.tiers.iter().map(|t| t.items.len()).sum()
    }

    /// Makes the list self-consistent after loading data from disk or an imported file:
    /// every item lives in exactly one place and file names cannot escape the list folder.
    pub fn repair(&mut self) {
        for item in self.items.values_mut() {
            item.file = sanitize_file_name(&item.file);
            item.thumb = sanitize_file_name(&item.thumb);
            if !(item.aspect.is_finite() && item.aspect > 0.0) {
                item.aspect = 1.0;
            }
        }
        let items = &self.items;
        let mut seen = HashSet::new();
        for tier in &mut self.tiers {
            tier.items
                .retain(|id| items.contains_key(id) && seen.insert(id.clone()));
        }
        self.unranked
            .retain(|id| items.contains_key(id) && seen.insert(id.clone()));
        let mut orphans: Vec<String> = items
            .keys()
            .filter(|id| !seen.contains(*id))
            .cloned()
            .collect();
        orphans.sort();
        self.unranked.extend(orphans);

        let s = &mut self.settings;
        s.item_size = s.item_size.clamp(32.0, 320.0);
        s.gap = s.gap.clamp(0.0, 24.0);
        s.label_width = s.label_width.clamp(40.0, 600.0);
        s.label_font_size = s.label_font_size.clamp(8.0, 96.0);
        s.export_scale = s.export_scale.clamp(1.0, 4.0);
    }
}

/// Keeps only the last path component so stored names can never point outside their folder.
pub fn sanitize_file_name(name: &str) -> String {
    std::path::Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| *n != "." && *n != "..")
        .unwrap_or("missing")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> Item {
        Item {
            id: id.into(),
            name: id.into(),
            file: format!("{id}.png"),
            thumb: format!("{id}.png"),
            aspect: 1.0,
        }
    }

    #[test]
    fn move_within_and_across_zones() {
        let mut list = TierList::new("t", Template::Classic);
        for id in ["a", "b", "c"] {
            list.add_item(item(id));
        }
        list.move_item("a", Zone::Tier(0), 0);
        list.move_item("b", Zone::Tier(0), 0);
        assert_eq!(list.tiers[0].items, vec!["b", "a"]);
        list.move_item("b", Zone::Tier(0), 1);
        assert_eq!(list.tiers[0].items, vec!["a", "b"]);
        list.move_item("c", Zone::Tier(0), usize::MAX);
        assert_eq!(list.tiers[0].items, vec!["a", "b", "c"]);
        assert!(list.unranked.is_empty());
        list.delete_tier(0);
        assert_eq!(list.unranked, vec!["a", "b", "c"]);
    }

    #[test]
    fn preset_round_trip_keeps_tiers_but_not_images() {
        let mut list = TierList::new("t", Template::Words);
        list.add_item(item("a"));
        list.move_item("a", Zone::Tier(0), 0);
        list.tiers[1].text_color = Some([1, 2, 3]);
        list.settings.item_size = 120.0;
        let preset = Preset::from_list("Mine", &list, true);
        let fresh = TierList::from_preset("New", &preset);
        assert_eq!(fresh.tiers.len(), 4);
        assert_eq!(fresh.tiers[0].label, "Love it");
        assert_eq!(fresh.tiers[1].text_color, Some([1, 2, 3]));
        assert!(fresh.tiers.iter().all(|t| t.items.is_empty()));
        assert_eq!(fresh.settings.item_size, 120.0);
        let without = TierList::from_preset("New", &Preset::from_list("Mine", &list, false));
        assert_eq!(without.settings, Settings::default());
    }

    #[test]
    fn repair_fixes_duplicates_orphans_and_paths() {
        let mut list = TierList::new("t", Template::Classic);
        let mut evil = item("x");
        evil.file = "../../etc/passwd".into();
        list.items.insert("x".into(), evil);
        list.items.insert("y".into(), item("y"));
        list.tiers[0].items = vec!["x".into(), "x".into(), "ghost".into()];
        list.repair();
        assert_eq!(list.tiers[0].items, vec!["x"]);
        assert_eq!(list.unranked, vec!["y"]);
        assert_eq!(list.items["x"].file, "passwd");
    }
}
