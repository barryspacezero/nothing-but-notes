// Hide the console window in release builds so the widget launches cleanly.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone};
use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontData, FontDefinitions, FontFamily, FontId, Key,
    Pos2, Rect, Response, RichText, Rounding, Sense, Stroke, Vec2,
};
use serde::{Deserialize, Serialize};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, hotkey::{HotKey, Modifiers, Code}};
use arboard::Clipboard;
use crossbeam_channel::Receiver;
use std::path::PathBuf;
use std::time::{Duration, Instant};


use crate::*;
// ── Geometry & timing ───────────────────────────────────────────────────────
pub const NOTCH_H: Vec2 = Vec2::new(128.0, 34.0); // docked on top / bottom
pub const NOTCH_V: Vec2 = Vec2::new(34.0, 128.0); // docked on left / right
pub const PANEL_SIZE: Vec2 = Vec2::new(360.0, 560.0);
pub const PANEL_RADIUS: f32 = 16.0;
pub const PAD: f32 = 20.0;
pub const ANIM_TIME: f32 = 0.22;
pub const SAVE_DELAY: Duration = Duration::from_millis(800);

// ── Helpers ─────────────────────────────────────────────────────────────────

pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Zero-allocation substring search, optimized for RAM usage. Case-insensitive for ASCII.
pub fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    let h_bytes = haystack.as_bytes();
    let n_bytes = needle.as_bytes();
    if n_bytes.is_empty() { return true; }
    h_bytes.windows(n_bytes.len()).any(|w| w.eq_ignore_ascii_case(n_bytes))
}

pub fn clamp_range(v: f32, lo: f32, hi: f32) -> f32 {
    if hi < lo {
        lo
    } else {
        v.max(lo).min(hi)
    }
}

pub fn anchor_or_center(store: &Store, wa: Rect) -> f32 {
    store
        .anchor
        .unwrap_or(if store.edge.vertical() { wa.center().y } else { wa.center().x })
}

/// Top-left window position for a widget of `size` docked to `edge`, centered on `anchor`.
pub fn docked_pos(edge: Edge, anchor: f32, size: Vec2, wa: Rect) -> Pos2 {
    let x = clamp_range(anchor - size.x / 2.0, wa.left(), wa.right() - size.x);
    let y = clamp_range(anchor - size.y / 2.0, wa.top(), wa.bottom() - size.y);
    match edge {
        Edge::Top => Pos2::new(x, wa.top()),
        Edge::Bottom => Pos2::new(x, wa.bottom() - size.y),
        Edge::Left => Pos2::new(wa.left(), y),
        Edge::Right => Pos2::new(wa.right() - size.x, y),
    }
}

/// Desktop area excluding the taskbar, in logical points.
#[cfg(windows)]
pub fn work_area_points(ppp: f32) -> Option<Rect> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA};
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: SPI_GETWORKAREA writes a single RECT into the provided pointer.
    let ok = unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut RECT as *mut _, 0) };
    (ok != 0).then(|| {
        Rect::from_min_max(
            Pos2::new(r.left as f32 / ppp, r.top as f32 / ppp),
            Pos2::new(r.right as f32 / ppp, r.bottom as f32 / ppp),
        )
    })
}

#[cfg(not(windows))]
pub fn work_area_points(_ppp: f32) -> Option<Rect> {
    None
}

#[cfg(windows)]
pub fn system_scale() -> f32 {
    // SAFETY: no arguments, always safe to call.
    let dpi = unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForSystem() };
    if dpi == 0 {
        1.0
    } else {
        dpi as f32 / 96.0
    }
}

#[cfg(not(windows))]
pub fn system_scale() -> f32 {
    1.0
}

pub fn local_dt(ts: i64) -> DateTime<Local> {
    Local.timestamp_opt(ts, 0).single().unwrap_or_else(Local::now)
}

pub fn day_label(date: NaiveDate, today: NaiveDate) -> String {
    match (today - date).num_days() {
        0 => "Today".to_owned(),
        1 => "Yesterday".to_owned(),
        2..=6 => date.format("%A").to_string(),
        _ if date.year() == today.year() => date.format("%a %d %b").to_string(),
        _ => date.format("%d %b %Y").to_string(),
    }
}

pub fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        let head: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{head}…")
    }
}





