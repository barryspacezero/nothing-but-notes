
use chrono::Local;
use eframe::egui::{
    Rounding, Vec2,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::*;
// ── Data model ──────────────────────────────────────────────────────────────

/// Which screen edge the widget is docked to.
#[derive(Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Edge {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    pub fn vertical(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }

    pub fn notch_size(self) -> Vec2 {
        if self.vertical() {
            NOTCH_V
        } else {
            NOTCH_H
        }
    }

    /// Flat on the docked side, rounded on the side facing the screen.
    pub fn rounding(self, r: f32) -> Rounding {
        match self {
            Edge::Top => Rounding { nw: 0.0, ne: 0.0, sw: r, se: r },
            Edge::Bottom => Rounding { nw: r, ne: r, sw: 0.0, se: 0.0 },
            Edge::Left => Rounding { nw: 0.0, sw: 0.0, ne: r, se: r },
            Edge::Right => Rounding { nw: r, sw: r, ne: 0.0, se: 0.0 },
        }
    }
}

use std::sync::Arc;

#[derive(Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: u64,
    /// Unix seconds.
    pub created: i64,
    pub updated: i64,
    pub text: String,
    #[serde(default)]
    pub is_clipboard: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub title: String,
    #[serde(skip)]
    pub preview: String,
    #[serde(skip)]
    pub display_title: String,
    #[serde(skip)]
    pub is_todo_cached: bool,
}

impl Note {
    pub fn new(id: u64, text: String, is_clipboard: bool) -> Self {
        let now = Local::now().timestamp();
        let mut note = Self {
            id,
            created: now,
            updated: now,
            text,
            is_clipboard,
            pinned: false,
            title: String::new(),
            preview: String::new(),
            display_title: String::new(),
            is_todo_cached: false,
        };
        note.refresh_cache();
        note
    }

    /// Precompute and cache display_title, preview, and is_todo so 60+ FPS UI frames do zero string parsing.
    pub fn refresh_cache(&mut self) {
        self.is_todo_cached = is_todo(&self.text);
        let preview_str = self.content_lines().nth(1).unwrap_or("").to_string();
        let title_candidate = self.content_lines().next().unwrap_or("Untitled").to_string();
        let t = self.title.trim();
        self.display_title = if !t.is_empty() {
            t.to_string()
        } else if self.is_todo_cached {
            "TO-DO".to_string()
        } else {
            title_candidate
        };
        self.preview = preview_str;
    }

    fn content_lines(&self) -> impl Iterator<Item = &str> {
        self.text.lines().map(|l| parse_line(l).1.trim()).filter(|l| !l.is_empty())
    }

    #[inline]
    pub fn title(&self) -> &str {
        if !self.display_title.is_empty() {
            &self.display_title
        } else {
            "Untitled"
        }
    }

    #[inline]
    pub fn preview(&self) -> &str {
        &self.preview
    }

    #[inline]
    pub fn is_todo(&self) -> bool {
        self.is_todo_cached
    }

    pub fn is_blank(&self) -> bool {
        self.content_lines().next().is_none()
    }
}

/// Everything persisted to `%APPDATA%\Nothing But Notes\notes.json`.
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Store {
    #[serde(default)]
    pub notes: Vec<Arc<Note>>,
    #[serde(default)]
    pub edge: Edge,
    /// Center of the widget along its docked edge (points). `None` = centered.
    #[serde(default)]
    pub anchor: Option<f32>,
    #[serde(default)]
    pub next_id: u64,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub dark_mode: bool,
}

pub struct BackgroundSaver {
    tx: crossbeam_channel::Sender<Store>,
}

impl Default for BackgroundSaver {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundSaver {
    pub fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded::<Store>();
        std::thread::spawn(move || {
            while let Ok(store) = rx.recv() {
                // Debounce quiet period: wait 400ms to coalesce rapid keystrokes/saves
                std::thread::sleep(std::time::Duration::from_millis(400));
                let mut latest = store;
                while let Ok(newer) = rx.try_recv() {
                    latest = newer;
                }
                let _ = save_store(&latest);
            }
        });
        Self { tx }
    }

    pub fn schedule_save(&self, store: Store) {
        let _ = self.tx.send(store);
    }
}

pub fn data_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let new_path = base.join("Nothing But Notes").join("notes.json");
    let old_path = base.join("NothingNotes").join("notes.json");
    if !new_path.exists() && old_path.exists() {
        if let Some(parent) = new_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::copy(&old_path, &new_path);
    }
    new_path
}

pub const DEFAULT_GUIDE_TITLE: &str = "Nothing But Notes — Quick Guide";

pub const DEFAULT_GUIDE_TEXT: &str = "\
# NOTHING (R) BUT NOTES
A minimal, glyph-inspired desktop companion docked directly to your screen edge.

● DOCK & SUMMON
- Drag the notch along any screen edge (top, bottom, left, right).
- Click the notch or press Ctrl+Space to expand.
- Esc or clicking outside collapses it back into your screen edge.

● DUAL TABS: NOTES & CLIPBOARD
- [NOTES]: Your personal notes & interactive to-do lists.
- [CLIPBOARD]: Automatically captures copied text in the background without cluttering your notes. Use CLEAR to wipe clips anytime.

● TO-DO LISTS (Ctrl+T)
- Create interactive checklists with live progress counters.
- Enter creates a new task; Ctrl+Enter toggles completion.
- Alt + Up/Down reorders tasks; Backspace on an empty item deletes it.

● MINIMIZE TO TRAY (Ctrl+M)
- Click [ – ] in the header or right-click the notch to minimize into the system tray & taskbar.
- Left-click or double-click the tray icon to restore.

● SHORTCUTS
- Ctrl + N : New note
- Ctrl + T : New to-do
- Ctrl + M : Minimize to tray
- Ctrl + Space : Global summon
- Esc : Collapse / Back";

pub fn create_guide_note(next_id: &mut u64) -> Note {
    *next_id += 1;
    let mut note = Note::new(*next_id, DEFAULT_GUIDE_TEXT.to_string(), false);
    note.title = DEFAULT_GUIDE_TITLE.to_string();
    note.pinned = true;
    note
}

pub fn load_store() -> Store {
    let mut store: Store = std::fs::read_to_string(data_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    for note in &mut store.notes {
        Arc::make_mut(note).refresh_cache();
    }

    if store.notes.is_empty() {
        store.notes.push(Arc::new(create_guide_note(&mut store.next_id)));
        let _ = save_store(&store);
    }
    store
}

pub fn save_store(store: &Store) -> std::io::Result<()> {
    let path = data_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(store).map_err(std::io::Error::other)?;
    // Write-then-rename so a crash mid-save never corrupts the notes file.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(tmp, path)
}

