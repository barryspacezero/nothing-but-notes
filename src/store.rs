
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
}

impl Note {
    pub fn new(id: u64, text: String, is_clipboard: bool) -> Self {
        let now = Local::now().timestamp();
        Self { id, created: now, updated: now, text, is_clipboard, pinned: false, title: String::new() }
    }

    fn content_lines(&self) -> impl Iterator<Item = &str> {
        self.text.lines().map(|l| parse_line(l).1.trim()).filter(|l| !l.is_empty())
    }

    pub fn title(&self) -> &str {
        let t = self.title.trim();
        if !t.is_empty() {
            return t;
        }
        if self.is_todo() {
            return "TO-DO";
        }
        self.content_lines().next().unwrap_or("Untitled")
    }

    pub fn preview(&self) -> &str {
        self.content_lines().nth(1).unwrap_or("")
    }

    pub fn is_todo(&self) -> bool {
        is_todo(&self.text)
    }

    pub fn is_blank(&self) -> bool {
        self.content_lines().next().is_none()
    }
}

/// Everything persisted to `%APPDATA%\Nothing But Notes\notes.json`.
#[derive(Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub notes: Vec<Note>,
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

pub fn load_store() -> Store {
    std::fs::read_to_string(data_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
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

