//! Persistent state: notes and settings in one JSON file, written atomically.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const COLORS: [&str; 7] = ["butter", "peach", "mint", "sky", "lilac", "rose", "ivory"];
pub const PATTERNS: [&str; 5] = ["blank", "ruled", "grid", "cross", "dots"];
pub const SHAPES: [&str; 5] = ["square", "circle", "cloud", "heart", "bubble"];
pub const DEFAULT_NOTE_W: f64 = 340.0;
pub const DEFAULT_NOTE_H: f64 = 330.0;

/// Content rectangle of a note window in logical pixels (shadow margin excluded).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub color: String,
    /// One of `SHAPES`; files written before shapes existed load as "square".
    #[serde(default = "default_shape")]
    pub shape: String,
    #[serde(default = "default_pattern")]
    pub pattern: String,
    pub content: String,
    pub pinned: bool,
    pub open: bool,
    pub window: Option<WindowRect>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Vertical centre of the tab as a fraction of the work-area height.
    pub tab_y: f64,
    pub tab_hidden: bool,
    /// Launch with Windows. On by default; the registry entry is refreshed on
    /// every release-build start so it always points at the current exe.
    pub autostart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            tab_y: 0.36,
            tab_hidden: false,
            autostart: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Data {
    pub version: u32,
    pub settings: Settings,
    pub notes: Vec<Note>,
}

impl Default for Data {
    fn default() -> Self {
        Data {
            version: 1,
            settings: Settings::default(),
            notes: Vec::new(),
        }
    }
}

pub struct Store {
    path: PathBuf,
    data: Data,
    dirty: bool,
}

impl Store {
    /// Reads the file if it exists. Unreadable JSON is moved aside, never overwritten.
    pub fn load(path: PathBuf) -> Store {
        let data = match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Data>(&text) {
                Ok(data) => data,
                Err(err) => {
                    log::error!("{} is not valid; moving it aside: {err}", path.display());
                    quarantine(&path);
                    fs::read_to_string(path.with_extension("json.bak"))
                        .ok()
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default()
                }
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => Data::default(),
            Err(err) => {
                log::error!("could not read {}: {err}", path.display());
                Data::default()
            }
        };
        Store {
            path,
            data,
            dirty: false,
        }
    }

    pub fn data(&self) -> &Data {
        &self.data
    }

    pub fn snapshot(&self) -> Data {
        self.data.clone()
    }

    pub fn notes(&self) -> &[Note] {
        &self.data.notes
    }

    pub fn note(&self, id: &str) -> Option<&Note> {
        self.data.notes.iter().find(|n| n.id == id)
    }

    pub fn note_mut(&mut self, id: &str) -> Option<&mut Note> {
        self.dirty = true;
        self.data.notes.iter_mut().find(|n| n.id == id)
    }

    pub fn settings_mut(&mut self) -> &mut Settings {
        self.dirty = true;
        &mut self.data.settings
    }

    pub fn create_note(&mut self) -> Note {
        let now = now_ms();
        let note = Note {
            id: new_id(),
            color: next_color(&self.data.notes).to_string(),
            shape: "square".to_string(),
            pattern: "blank".to_string(),
            content: String::new(),
            pinned: true,
            open: true,
            window: None,
            created_at: now,
            updated_at: now,
        };
        self.data.notes.push(note.clone());
        self.dirty = true;
        note
    }

    pub fn delete_note(&mut self, id: &str) -> bool {
        let before = self.data.notes.len();
        self.data.notes.retain(|n| n.id != id);
        let removed = self.data.notes.len() != before;
        if removed {
            self.dirty = true;
        }
        removed
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Writes to a temp file, then renames over the real one.
    pub fn save(&mut self) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(&self.data).map_err(io::Error::other)?;
        let tmp = self.path.with_extension("json.tmp");
        let mut file = fs::File::create(&tmp)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        drop(file);
        if self.path.exists() {
            fs::copy(&self.path, self.path.with_extension("json.bak"))?;
        }
        fs::rename(&tmp, &self.path)?;
        self.dirty = false;
        Ok(())
    }

    pub fn save_if_dirty(&mut self) -> io::Result<()> {
        if self.dirty {
            self.save()
        } else {
            Ok(())
        }
    }
}

/// The colour after the last note's colour, so new notes walk the palette.
pub fn next_color(notes: &[Note]) -> &'static str {
    let last = notes
        .last()
        .and_then(|n| COLORS.iter().position(|c| *c == n.color));
    match last {
        Some(i) => COLORS[(i + 1) % COLORS.len()],
        None => COLORS[0],
    }
}

fn default_shape() -> String {
    "square".to_string()
}

fn default_pattern() -> String {
    "blank".to_string()
}

pub fn valid_color(color: &str) -> bool {
    COLORS.contains(&color)
        || (color.len() == 7 && color.starts_with('#') && color[1..].bytes().all(|b| b.is_ascii_hexdigit()))
}

fn quarantine(path: &Path) {
    let bad = path.with_extension(format!("json.bad-{}", now_ms()));
    if let Err(err) = fs::rename(path, &bad) {
        log::error!("could not move {} aside: {err}", path.display());
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn new_id() -> String {
    use rand::Rng;
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..10)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_path() -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("cute-sticky-test-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).unwrap();
        dir.join("notes.json")
    }

    #[test]
    fn missing_file_starts_empty() {
        let store = Store::load(temp_path());
        assert_eq!(store.data(), &Data::default());
        assert!(!store.is_dirty());
    }

    #[test]
    fn round_trip_saves_and_loads() {
        let path = temp_path();
        let mut store = Store::load(path.clone());
        let note = store.create_note();
        store.note_mut(&note.id).unwrap().content = "Groceries\n- [ ] eggs".into();
        store.settings_mut().tab_y = 0.5;
        store.save().unwrap();
        let again = Store::load(path);
        assert_eq!(again.data(), store.data());
        assert_eq!(again.note(&note.id).unwrap().content, "Groceries\n- [ ] eggs");
        assert_eq!(again.data().settings.tab_y, 0.5);
    }

    #[test]
    fn save_leaves_no_temp_file() {
        let path = temp_path();
        let mut store = Store::load(path.clone());
        store.create_note();
        store.save().unwrap();
        assert!(path.exists());
        assert!(!path.with_extension("json.tmp").exists());
        assert!(!store.is_dirty());
    }

    #[test]
    fn corrupt_file_is_quarantined() {
        let path = temp_path();
        fs::write(&path, "{ not json").unwrap();
        let store = Store::load(path.clone());
        assert_eq!(store.data(), &Data::default());
        assert!(!path.exists());
        let quarantined = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with("notes.json.bad-"));
        assert!(quarantined);
    }

    #[test]
    fn new_notes_cycle_colours() {
        let mut store = Store::load(temp_path());
        let colours: Vec<String> = (0..8).map(|_| store.create_note().color).collect();
        assert_eq!(colours[0], "butter");
        assert_eq!(colours[5], "rose");
        assert_eq!(colours[6], "ivory");
        assert_eq!(colours[7], "butter");
    }

    #[test]
    fn next_colour_follows_the_last_note() {
        let mut store = Store::load(temp_path());
        store.create_note();
        store.create_note();
        let first = store.notes()[0].id.clone();
        store.delete_note(&first);
        assert_eq!(store.create_note().color, "mint");
    }

    #[test]
    fn delete_reports_whether_it_removed() {
        let mut store = Store::load(temp_path());
        let note = store.create_note();
        assert!(store.delete_note(&note.id));
        assert!(!store.delete_note(&note.id));
        assert!(store.notes().is_empty());
    }

    #[test]
    fn ids_are_ten_lowercase_alphanumerics() {
        let id = new_id();
        assert_eq!(id.len(), 10);
        assert!(id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
        assert_ne!(id, new_id());
    }

    #[test]
    fn settings_missing_from_older_files_get_defaults() {
        let path = temp_path();
        fs::write(&path, r#"{"version":1,"settings":{"tabY":0.5},"notes":[]}"#).unwrap();
        let store = Store::load(path);
        assert_eq!(store.data().settings.tab_y, 0.5);
        assert!(!store.data().settings.autostart);
        assert!(!store.data().settings.tab_hidden);
    }

    #[test]
    fn notes_without_a_shape_load_as_square() {
        let path = temp_path();
        fs::write(
            &path,
            r#"{"version":1,"settings":{},"notes":[{"id":"a","color":"mint","content":"x","pinned":true,"open":false,"window":null,"createdAt":1,"updatedAt":1}]}"#,
        )
        .unwrap();
        let mut store = Store::load(path);
        assert_eq!(store.notes()[0].shape, "square");
        assert_eq!(store.create_note().shape, "square");
    }

    #[test]
    fn json_uses_camel_case() {
        let mut store = Store::load(temp_path());
        store.create_note();
        let json = serde_json::to_string(store.data()).unwrap();
        assert!(json.contains("\"tabY\""));
        assert!(json.contains("\"createdAt\""));
        assert!(!json.contains("created_at"));
    }

    #[test]
    fn chinese_and_each_paper_pattern_survive_restart() {
        let path = temp_path();
        let mut store = Store::load(path.clone());
        for pattern in PATTERNS {
            let note = store.create_note();
            let n = store.note_mut(&note.id).unwrap();
            n.content = "下午三点，记得带伞。\n- [ ] 回复邮件".into();
            n.color = "#405a32".into();
            n.pattern = pattern.into();
            n.window = Some(WindowRect {
                x: 150.0,
                y: 210.0,
                w: 340.0,
                h: 330.0,
            });
        }
        store.save().unwrap();
        let again = Store::load(path);
        assert_eq!(again.data(), store.data());
    }

    #[test]
    fn corrupt_primary_recovers_last_backup() {
        let path = temp_path();
        let mut store = Store::load(path.clone());
        let note = store.create_note();
        store.note_mut(&note.id).unwrap().content = "上一份内容".into();
        store.save().unwrap();
        store.note_mut(&note.id).unwrap().content = "新的内容".into();
        store.save().unwrap();
        fs::write(&path, "broken").unwrap();
        let restored = Store::load(path);
        assert_eq!(restored.note(&note.id).unwrap().content, "上一份内容");
    }

    #[test]
    fn accepts_only_presets_and_six_digit_custom_colors() {
        assert!(valid_color("ivory"));
        assert!(valid_color("#ab12EF"));
        for bad in ["red", "#123", "#12345z", "url(x)", "#000000;display:none"] {
            assert!(!valid_color(bad));
        }
    }
}
